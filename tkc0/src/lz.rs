use std::collections::HashMap;

const MIN_MATCH: u32 = 3;
const MAX_MATCH: u32 = 4096;
const WINDOW: usize = 262144;
const MAX_CANDIDATES: usize = 16;

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

pub fn build_hash(data: &[u8]) -> (HashMap<u64, Vec<u32>>, HashMap<u64, Vec<u32>>) {
    let mut ht_xor: HashMap<u64, Vec<u32>> = HashMap::new();
    let mut ht_add: HashMap<u64, Vec<u32>> = HashMap::new();
    if data.len() < 5 {
        return (ht_xor, ht_add);
    }
    for i in 0..data.len() - 4 {
        let (x, d) = hash_keys(data, i);
        ht_xor.entry(x).or_default().push(i as u32);
        ht_add.entry(d).or_default().push(i as u32);
    }
    (ht_xor, ht_add)
}

fn offset_bits(off: u32) -> u32 {
    if off <= 8 { 4 }
    else if off <= 40 { 7 }
    else if off <= 168 { 10 }
    else if off <= 1192 { 14 }
    else { 4 + vlq_bits(off) }
}

fn length_bits(ln: u32) -> u32 {
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

pub fn find_match(
    data: &[u8],
    pos: usize,
    ht_xor: &HashMap<u64, Vec<u32>>,
    ht_add: &HashMap<u64, Vec<u32>>,
) -> Option<Token> {
    if pos + 5 > data.len() {
        return None;
    }
    let (x, d) = hash_keys(data, pos);

    let lo = pos.saturating_sub(WINDOW);
    let n_remaining = data.len() - pos;
    let max_len = n_remaining.min(MAX_MATCH as usize);

    let mut candidates = Vec::new();
    if let Some(positions) = ht_xor.get(&x) {
        for &c in positions.iter().rev() {
            let cu = c as usize;
            if cu >= lo && cu < pos {
                candidates.push(cu);
                if candidates.len() >= MAX_CANDIDATES { break; }
            }
        }
    }
    if candidates.len() < MAX_CANDIDATES {
        if let Some(positions) = ht_add.get(&d) {
            for &c in positions.iter().rev() {
                let cu = c as usize;
                if cu >= lo && (cu) < pos && !candidates.contains(&cu) {
                    candidates.push(cu);
                    if candidates.len() >= MAX_CANDIDATES { break; }
                }
            }
        }
    }

    let mut best: Option<(u32, u32, Transform, u8)> = None;
    let mut best_sav: i64 = -1_000_000_000;

    for &off in &candidates {
        for &t in &[Transform::Exact, Transform::Xor, Transform::Add, Transform::Sub] {
            let (ln, param) = match t {
                Transform::Exact => {
                    let mut ln = 0usize;
                    while ln < max_len && data[pos + ln] == data[off + ln] {
                        ln += 1;
                    }
                    (ln, 0u8)
                }
                Transform::Xor => {
                    let p = data[pos] ^ data[off];
                    let mut ln = 1usize;
                    while ln < max_len && data[pos + ln] ^ p == data[off + ln] {
                        ln += 1;
                    }
                    (ln, p)
                }
                Transform::Add => {
                    let p = data[pos].wrapping_sub(data[off]);
                    let mut ln = 1usize;
                    while ln < max_len && data[pos + ln].wrapping_sub(p) == data[off + ln] {
                        ln += 1;
                    }
                    (ln, p)
                }
                Transform::Sub => {
                    let p = data[off].wrapping_sub(data[pos]);
                    let mut ln = 1usize;
                    while ln < max_len && data[pos + ln].wrapping_add(p) == data[off + ln] {
                        ln += 1;
                    }
                    (ln, p)
                }
            };
            if ln < MIN_MATCH as usize {
                continue;
            }
            let est_lit = ln as i64 * 7;
            let est_match = 1 + 2 + offset_bits((pos - off) as u32) as i64
                + length_bits(ln as u32) as i64
                + if t == Transform::Exact { 0 } else { 8 };
            let sav = est_lit - est_match;
            if sav > best_sav {
                best_sav = sav;
                best = Some(((pos - off) as u32, ln as u32, t, param));
            }
        }
    }

    best.map(|(off, ln, t, param)| Token::Match { off, ln, t, param })
}
