const MIN_MATCH: u32 = 3;
const MAX_MATCH: u32 = 65535;
pub const WINDOW: usize = 65536;
const MAX_CANDIDATES: usize = 256;
const NICE_MATCH: u32 = 128;
const HASH_MASK: usize = 0xFFFF;

#[derive(Debug, Clone)]
pub enum Token {
    Lit(u8),
    Match { off: u32, ln: u32 },
}

fn hash_4(data: &[u8], i: usize) -> usize {
    let v = u32::from_ne_bytes(data[i..i + 4].try_into().unwrap());
    ((v ^ (v >> 12) ^ (v >> 24)) as usize) & HASH_MASK
}

fn hash_3(data: &[u8], i: usize) -> usize {
    let v = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], 0]);
    ((v ^ (v >> 12) ^ (v >> 24)) as usize) & HASH_MASK
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HashType {
    Hash4,
    Hash3,
}

pub struct HashTables {
    pub ht: HashType,
    entries: Vec<u32>,
    offsets: Vec<u32>,
}

pub fn build_hash(data: &[u8], ht: HashType) -> HashTables {
    match ht {
        HashType::Hash4 => {
            let n = data.len();
            if n < 4 {
                return HashTables {
                    ht: HashType::Hash4,
                    entries: Vec::new(),
                    offsets: vec![0; HASH_MASK + 2],
                };
            }
            let mut counts = vec![0u32; HASH_MASK + 1];
            for i in 0..n - 3 {
                counts[hash_4(data, i)] += 1;
            }
            let mut offsets = vec![0u32; HASH_MASK + 2];
            let mut sum = 0u32;
            for (slot, count) in counts.iter().enumerate() {
                offsets[slot] = sum;
                sum += count;
            }
            offsets[HASH_MASK + 1] = sum;
            let mut entries = vec![0u32; sum as usize];
            let mut cursors = offsets[..HASH_MASK + 1].to_vec();
            for i in 0..n - 3 {
                let key = hash_4(data, i);
                let slot = &mut cursors[key];
                entries[*slot as usize] = i as u32;
                *slot += 1;
            }
            HashTables { ht: HashType::Hash4, entries, offsets }
        }
        HashType::Hash3 => {
            let n = data.len();
            if n < 3 {
                return HashTables {
                    ht: HashType::Hash3,
                    entries: Vec::new(),
                    offsets: vec![0; HASH_MASK + 2],
                };
            }
            let mut counts = vec![0u32; HASH_MASK + 1];
            for i in 0..n - 2 {
                counts[hash_3(data, i)] += 1;
            }
            let mut offsets = vec![0u32; HASH_MASK + 2];
            let mut sum = 0u32;
            for (slot, count) in counts.iter().enumerate() {
                offsets[slot] = sum;
                sum += count;
            }
            offsets[HASH_MASK + 1] = sum;
            let mut entries = vec![0u32; sum as usize];
            let mut cursors = offsets[..HASH_MASK + 1].to_vec();
            for i in 0..n - 2 {
                let key = hash_3(data, i);
                let slot = &mut cursors[key];
                entries[*slot as usize] = i as u32;
                *slot += 1;
            }
            HashTables { ht: HashType::Hash3, entries, offsets }
        }
    }
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

fn find_in_slice(data: &[u8], pos: usize, slice: &[u32], max_len: usize, nice: u32,
                 best_off: &mut u32, best_ln: &mut u32, best_sav: &mut i64, lit_cost: i64,
                 min_match: u32) {
    let idx = slice.partition_point(|&c| (c as usize) < pos);
    let iter_start = idx.saturating_sub(MAX_CANDIDATES);

    for &cu in slice[iter_start..idx].iter().rev() {
        let cu = cu as usize;
        let diff = pos - cu;
        if diff > WINDOW { break; }

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

        if ln >= min_match as usize {
            let sav = ln as i64 * lit_cost - match_cost(diff as u32, ln as u32);
            if sav > *best_sav {
                *best_sav = sav;
                *best_off = diff as u32;
                *best_ln = ln as u32;
            }
            if (ln as u32) >= nice { break; }
        }
    }
}

pub fn find_match(data: &[u8], pos: usize, ht: &HashTables, lit_cost: i64) -> Option<Token> {
    match ht.ht {
        HashType::Hash4 => {
            if pos + 4 > data.len() { return None; }
            let min_match = MIN_MATCH;
            let hash = hash_4(data, pos);
            let start = ht.offsets[hash] as usize;
            let end = ht.offsets[hash + 1] as usize;
            if start >= end { return None; }
            let nice = if lit_cost <= 3 { MAX_MATCH } else { NICE_MATCH };
            let max_len = (data.len() - pos).min(MAX_MATCH as usize);

            let mut best_off = 0u32;
            let mut best_ln = 0u32;
            let mut best_sav = -1_000_000_000i64;

            find_in_slice(data, pos, &ht.entries[start..end], max_len, nice,
                          &mut best_off, &mut best_ln, &mut best_sav, lit_cost, min_match);

            if best_ln >= MIN_MATCH {
                assert!(best_off > 0, "find_match off=0 at pos={}", pos);
                Some(Token::Match { off: best_off, ln: best_ln })
            } else {
                None
            }
        }
        HashType::Hash3 => {
            if pos + 3 > data.len() { return None; }
            let min_match = MIN_MATCH;
            let hash = hash_3(data, pos);
            let start = ht.offsets[hash] as usize;
            let end = ht.offsets[hash + 1] as usize;
            if start >= end { return None; }
            let nice = if lit_cost <= 3 { MAX_MATCH } else { NICE_MATCH };
            let max_len = (data.len() - pos).min(MAX_MATCH as usize);

            let mut best_off = 0u32;
            let mut best_ln = 0u32;
            let mut best_sav = -1_000_000_000i64;

            find_in_slice(data, pos, &ht.entries[start..end], max_len, nice,
                          &mut best_off, &mut best_ln, &mut best_sav, lit_cost, min_match);

            if best_ln >= MIN_MATCH {
                assert!(best_off > 0, "find_match off=0 at pos={}", pos);
                Some(Token::Match { off: best_off, ln: best_ln })
            } else {
                None
            }
        }
    }
}
