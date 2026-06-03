use std::collections::HashMap;

const MIN_MATCH: u32 = 3;
const MAX_MATCH: u32 = 4096;
const WINDOW: usize = 262144;
const MAX_CANDIDATES: usize = 128;
const HASH_BITS: u32 = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    Exact = 0,
    Xor = 1,
    Add = 2,
    Sub = 3,
}

#[derive(Debug, Clone)]
pub enum Token {
    Lit(u8),
    Match { off: u32, ln: u32, t: Transform, param: u8 },
}

fn hash_3(data: &[u8], i: usize) -> usize {
    let a = data[i] as usize;
    let b = data[i + 1] as usize;
    let c = data[i + 2] as usize;
    ((a << 10) ^ (b << 5) ^ c) & (HASH_SIZE - 1)
}

fn hash_keys(data: &[u8], i: usize) -> (u64, u64) {
    let x = ((data[i] as u64) << 24)
        | ((data[i + 1] as u64) << 16)
        | ((data[i + 2] as u64) << 8)
        | (data[i + 3] as u64);
    let d = ((data[i + 1].wrapping_sub(data[i]) as u64) & 0xff) << 24
        | ((data[i + 2].wrapping_sub(data[i + 1]) as u64) & 0xff) << 16
        | ((data[i + 3].wrapping_sub(data[i + 2]) as u64) & 0xff) << 8
        | ((data[i + 4].wrapping_sub(data[i + 3]) as u64) & 0xff);
    (x, d)
}

pub struct HashTables {
    pub xor_ht: HashMap<u64, Vec<u32>>,
    pub add_ht: HashMap<u64, Vec<u32>>,
    // hash chain for exact matching
    pub chain_head: Vec<u32>,
    pub chain_prev: Vec<u32>,
}

pub fn build_hash(data: &[u8]) -> HashTables {
    let mut xor_ht: HashMap<u64, Vec<u32>> = HashMap::new();
    let mut add_ht: HashMap<u64, Vec<u32>> = HashMap::new();
    let mut chain_head = vec![u32::MAX; HASH_SIZE];
    let mut chain_prev = vec![u32::MAX; data.len()];
    if data.len() >= 5 {
        for i in 0..data.len() - 4 {
            let (x, d) = hash_keys(data, i);
            xor_ht.entry(x).or_default().push(i as u32);
            add_ht.entry(d).or_default().push(i as u32);
        }
    }
    if data.len() >= 3 {
        for i in 0..data.len() - 2 {
            let h = hash_3(data, i);
            chain_prev[i] = chain_head[h];
            chain_head[h] = i as u32;
        }
    }
    HashTables { xor_ht, add_ht, chain_head, chain_prev }
}

pub fn offset_bits(off: u32) -> u32 {
    if off <= 8 { 4 }
    else if off <= 40 { 7 }
    else if off <= 168 { 10 }
    else if off <= 1192 { 14 }
    else if off <= 11272 { 19 }
    else { 5 + vlq_bits(off) }
}

pub fn length_bits(ln: u32) -> u32 {
    if ln <= 10 { 4 }
    else if ln <= 26 { 6 }
    else if ln <= 90 { 9 }
    else { 3 + vlq_bits(ln) }
}

fn vlq_bits(v: u32) -> u32 {
    if v == 0 { return 8; }
    let mut n = 0u32;
    let mut x = v;
    loop {
        n += 8;
        x >>= 7;
        if x == 0 { return n; }
    }
}

pub fn match_cost(off: u32, ln: u32, t: Transform) -> i64 {
    let transform_bits = match t {
        Transform::Exact => 1,
        Transform::Xor => 2,
        Transform::Add | Transform::Sub => 3,
    };
    (transform_bits + offset_bits(off) + length_bits(ln) + if t == Transform::Exact { 0 } else { 8 }) as i64
}

fn eval_candidate(data: &[u8], pos: usize, off: usize, max_len: usize, best_sav: &mut i64, best: &mut Option<(u32, u32, Transform, u8)>) {
    for &t in &[Transform::Exact, Transform::Xor, Transform::Add, Transform::Sub] {
        let (ln, param) = match t {
            Transform::Exact => {
                let mut ln = 0usize;
                while ln < max_len && data[pos + ln] == data[off + ln] { ln += 1; }
                (ln, 0u8)
            }
            Transform::Xor => {
                let p = data[pos] ^ data[off];
                let mut ln = 1usize;
                while ln < max_len && data[pos + ln] ^ p == data[off + ln] { ln += 1; }
                (ln, p)
            }
            Transform::Add => {
                let p = data[pos].wrapping_sub(data[off]);
                let mut ln = 1usize;
                while ln < max_len && data[pos + ln].wrapping_sub(p) == data[off + ln] { ln += 1; }
                (ln, p)
            }
            Transform::Sub => {
                let p = data[off].wrapping_sub(data[pos]);
                let mut ln = 1usize;
                while ln < max_len && data[pos + ln].wrapping_add(p) == data[off + ln] { ln += 1; }
                (ln, p)
            }
        };
        if ln < MIN_MATCH as usize { continue; }
        let est_lit = ln as i64 * 8;
        let est_match = match_cost((pos - off) as u32, ln as u32, t);
        let sav = est_lit - est_match;
        if sav > *best_sav {
            *best_sav = sav;
            *best = Some(((pos - off) as u32, ln as u32, t, param));
        }
    }
}

pub fn find_match(
    data: &[u8],
    pos: usize,
    ht: &HashTables,
) -> Option<Token> {
    if pos + 5 > data.len() { return None; }
    let (x, d) = hash_keys(data, pos);
    let lo = pos.saturating_sub(WINDOW);
    let max_len = (data.len() - pos).min(MAX_MATCH as usize);

    let mut best: Option<(u32, u32, Transform, u8)> = None;
    let mut best_sav: i64 = -1_000_000_000;

    // Phase 1: hash chain candidates (3-byte prefix, good for exact matches)
    if pos + 3 <= data.len() {
        let h = hash_3(data, pos);
        let mut chain = ht.chain_head[h] as usize;
        let mut checked = 0usize;
        while chain < pos && chain >= lo && checked < MAX_CANDIDATES {
            eval_candidate(data, pos, chain, max_len, &mut best_sav, &mut best);
            checked += 1;
            chain = ht.chain_prev[chain] as usize;
        }
    }

    // Phase 2: XOR hash table candidates (4-byte, good for transform matching)
    if let Some(positions) = ht.xor_ht.get(&x) {
        let mut checked = 0usize;
        for &c in positions.iter().rev() {
            let cu = c as usize;
            if cu >= lo && cu < pos {
                eval_candidate(data, pos, cu, max_len, &mut best_sav, &mut best);
                checked += 1;
                if checked >= MAX_CANDIDATES { break; }
            }
        }
    }

    // Phase 3: ADD hash table candidates (delta, good for ADD/XOR transform)
    if let Some(positions) = ht.add_ht.get(&d) {
        let mut checked = 0usize;
        for &c in positions.iter().rev() {
            let cu = c as usize;
            if cu >= lo && cu < pos {
                eval_candidate(data, pos, cu, max_len, &mut best_sav, &mut best);
                checked += 1;
                if checked >= MAX_CANDIDATES { break; }
            }
        }
    }

    best.map(|(off, ln, t, param)| Token::Match { off, ln, t, param })
}
