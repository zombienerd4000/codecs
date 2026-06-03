use std::collections::HashMap;

const MIN_MATCH: u32 = 3;
const MAX_MATCH: u32 = 65535;
pub const WINDOW: usize = 262144;
const MAX_XOR: usize = 4096;
const MAX_ADD: usize = 1024;

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

fn hash_keys_3(data: &[u8], i: usize) -> (u64, u64) {
    let x = ((data[i] as u64) << 16)
        | ((data[i + 1] as u64) << 8)
        | (data[i + 2] as u64);
    let d = ((data[i + 1].wrapping_sub(data[i]) as u64) & 0xff) << 16
        | ((data[i + 2].wrapping_sub(data[i + 1]) as u64) & 0xff) << 8
        | ((data[i + 3].wrapping_sub(data[i + 2]) as u64) & 0xff);
    (x, d)
}

fn hash_keys_4(data: &[u8], i: usize) -> (u64, u64) {
    let x = ((data[i] as u64) << 24)
        | ((data[i + 1] as u64) << 16)
        | ((data[i + 2] as u64) << 8)
        | (data[i + 3] as u64);
    let d = ((data[i + 1].wrapping_sub(data[i]) as u64) & 0xff) << 16
        | ((data[i + 2].wrapping_sub(data[i + 1]) as u64) & 0xff) << 8
        | ((data[i + 3].wrapping_sub(data[i + 2]) as u64) & 0xff);
    (x, d)
}

pub struct HashTables {
    pub xor_ht: HashMap<u64, Vec<u32>>,
    pub add_ht: HashMap<u64, Vec<u32>>,
}

pub fn build_hash(data: &[u8], use_3byte_xor: bool) -> HashTables {
    let n = data.len();
    let mut xor_ht: HashMap<u64, Vec<u32>> = HashMap::new();
    let mut add_ht: HashMap<u64, Vec<u32>> = HashMap::new();

    if use_3byte_xor {
        if n >= 4 {
            for i in 0..n - 3 {
                let (x, d) = hash_keys_3(data, i);
                xor_ht.entry(x).or_default().push(i as u32);
                add_ht.entry(d).or_default().push(i as u32);
            }
        }
    } else {
        if n >= 4 {
            for i in 0..n - 3 {
                let (x, d) = hash_keys_4(data, i);
                xor_ht.entry(x).or_default().push(i as u32);
                add_ht.entry(d).or_default().push(i as u32);
            }
        }
    }

    HashTables { xor_ht, add_ht }
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
    (offset_bits(off) + length_bits(ln) + if t == Transform::Exact { 0 } else { 8 }) as i64
}

fn eval_candidate(data: &[u8], pos: usize, off: usize, max_len: usize, lit_cost: i64, best_sav: &mut i64, best: &mut Option<(u32, u32, Transform, u8)>) {
    let mut ln = 0usize;
    while ln < max_len && data[pos + ln] == data[off + ln] { ln += 1; }
    if ln >= MIN_MATCH as usize {
        let sav = ln as i64 * lit_cost - match_cost((pos - off) as u32, ln as u32, Transform::Exact);
        if sav > *best_sav {
            *best_sav = sav;
            *best = Some(((pos - off) as u32, ln as u32, Transform::Exact, 0));
        }
        if ln == max_len { return; }
    }

    let p = data[pos] ^ data[off];
    ln = 1usize;
    while ln < max_len && data[pos + ln] ^ p == data[off + ln] { ln += 1; }
    if ln >= MIN_MATCH as usize {
        let sav = ln as i64 * lit_cost - match_cost((pos - off) as u32, ln as u32, Transform::Xor);
        if sav > *best_sav {
            *best_sav = sav;
            *best = Some(((pos - off) as u32, ln as u32, Transform::Xor, p));
        }
        if ln == max_len { return; }
    }

    let p = data[pos].wrapping_sub(data[off]);
    ln = 1usize;
    while ln < max_len && data[pos + ln].wrapping_sub(p) == data[off + ln] { ln += 1; }
    if ln >= MIN_MATCH as usize {
        let sav = ln as i64 * lit_cost - match_cost((pos - off) as u32, ln as u32, Transform::Add);
        if sav > *best_sav {
            *best_sav = sav;
            *best = Some(((pos - off) as u32, ln as u32, Transform::Add, p));
        }
        if ln == max_len { return; }
    }

    let p = data[off].wrapping_sub(data[pos]);
    ln = 1usize;
    while ln < max_len && data[pos + ln].wrapping_add(p) == data[off + ln] { ln += 1; }
    if ln >= MIN_MATCH as usize {
        let sav = ln as i64 * lit_cost - match_cost((pos - off) as u32, ln as u32, Transform::Sub);
        if sav > *best_sav {
            *best_sav = sav;
            *best = Some(((pos - off) as u32, ln as u32, Transform::Sub, p));
        }
    }
}

pub fn find_match(data: &[u8], pos: usize, ht: &HashTables, lit_cost: i64, use_3byte_xor: bool) -> Option<Token> {
    if pos + 4 > data.len() { return None; }
    let (x, d) = if use_3byte_xor { hash_keys_3(data, pos) } else { hash_keys_4(data, pos) };
    let lo = 0;
    let max_len = (data.len() - pos).min(MAX_MATCH as usize);

    let mut best: Option<(u32, u32, Transform, u8)> = None;
    let mut best_sav: i64 = -1_000_000_000;

    macro_rules! found_perfect {
        () => {
            best.as_ref().map(|(_, ln, _, _)| *ln as usize == max_len).unwrap_or(false)
        };
    }

    if let Some(positions) = ht.xor_ht.get(&x) {
        let mut checked = 0usize;
        for &c in positions.iter().rev() {
            if found_perfect!() { break; }
            let cu = c as usize;
            if cu >= lo && cu < pos {
                eval_candidate(data, pos, cu, max_len, lit_cost, &mut best_sav, &mut best);
                checked += 1;
                if checked >= MAX_XOR { break; }
            }
        }
    }

    if !found_perfect!() {
        if let Some(positions) = ht.add_ht.get(&d) {
            let mut checked = 0usize;
            for &c in positions.iter().rev() {
                if found_perfect!() { break; }
                let cu = c as usize;
                if cu >= lo && cu < pos {
                    eval_candidate(data, pos, cu, max_len, lit_cost, &mut best_sav, &mut best);
                    checked += 1;
                    if checked >= MAX_ADD { break; }
                }
            }
        }
    }

    best.map(|(o, l, t, p)| Token::Match { off: o, ln: l, t, param: p })
}
