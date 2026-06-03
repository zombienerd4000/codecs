use rustc_hash::FxHashMap;

const MIN_MATCH: u32 = 3;
const MAX_MATCH: u32 = 65535;
pub const WINDOW: usize = 262144;
const MAX_CANDIDATES: usize = 64;
const NICE_MATCH: u32 = 128;

#[derive(Debug, Clone)]
pub enum Token {
    Lit(u8),
    Match { off: u32, ln: u32 },
}

fn hash_key(data: &[u8], i: usize) -> u64 {
    u32::from_ne_bytes(data[i..i + 4].try_into().unwrap()) as u64
}

pub struct HashTables {
    map: FxHashMap<u64, Vec<u32>>,
}

pub fn build_hash(data: &[u8]) -> HashTables {
    let n = data.len();
    let mut map: FxHashMap<u64, Vec<u32>> = FxHashMap::default();
    if n >= 4 {
        map.reserve(n.min(65536));
        for i in 0..n - 3 {
            let key = hash_key(data, i);
            map.entry(key).or_default().push(i as u32);
        }
    }
    HashTables { map }
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

pub fn match_cost(off: u32, ln: u32) -> i64 {
    (offset_bits(off) + length_bits(ln)) as i64
}

pub fn find_match(data: &[u8], pos: usize, ht: &HashTables, lit_cost: i64) -> Option<Token> {
    if pos + 4 > data.len() { return None; }
    let key = hash_key(data, pos);
    let max_len = (data.len() - pos).min(MAX_MATCH as usize);

    let mut best_off = 0u32;
    let mut best_ln = 0u32;
    let mut best_sav = -1_000_000_000i64;

    if let Some(candidates) = ht.map.get(&key) {
        let idx = candidates.partition_point(|&c| (c as usize) < pos);
        let start = if idx > MAX_CANDIDATES { idx - MAX_CANDIDATES } else { 0 };
        for &cu in candidates[start..idx].iter().rev() {
            let cu = cu as usize;
            let diff = pos - cu;
            if diff <= WINDOW {
                let mut ln = 0usize;
                let a = &data[pos..];
                let b = &data[cu..];
                while ln + 8 <= max_len {
                    let va = u64::from_ne_bytes(a[ln..ln+8].try_into().unwrap());
                    let vb = u64::from_ne_bytes(b[ln..ln+8].try_into().unwrap());
                    if va != vb {
                        ln += (va ^ vb).trailing_zeros() as usize / 8;
                        break;
                    }
                    ln += 8;
                }
                while ln < max_len && a[ln] == b[ln] { ln += 1; }
                if ln >= MIN_MATCH as usize {
                    let sav = ln as i64 * lit_cost - match_cost(diff as u32, ln as u32);
                    if sav > best_sav {
                        best_sav = sav;
                        best_off = diff as u32;
                        best_ln = ln as u32;
                    }
                    if ln as u32 >= NICE_MATCH { break; }
                }
            }
        }
    }

    if best_ln >= MIN_MATCH {
        Some(Token::Match { off: best_off, ln: best_ln })
    } else {
        None
    }
}
