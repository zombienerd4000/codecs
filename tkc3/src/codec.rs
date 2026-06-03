use crate::bit::{BitWriter, BitReader};
use crate::lz::{Token, find_match, match_cost, build_hash, HashTables, WINDOW};
use crate::huff;

const MAGIC: u32 = 0x54484F52;
const MAGIC_RAW: u32 = 0x524F4854;
const LEN_CODES: usize = 29;
const MAIN_SYMS: usize = 256 + LEN_CODES;
const DIST_CODES: u16 = 32;

const LEN_TABLE: [(u16, u8); LEN_CODES] = {
    let mut t = [(0u16, 0u8); LEN_CODES];
    t[0] = (3, 0); t[1] = (4, 0); t[2] = (5, 0); t[3] = (6, 0);
    t[4] = (7, 0); t[5] = (8, 0); t[6] = (9, 0); t[7] = (10, 0);
    t[8] = (11, 1); t[9] = (13, 1); t[10] = (15, 1); t[11] = (17, 1);
    t[12] = (19, 2); t[13] = (23, 2); t[14] = (27, 2); t[15] = (31, 2);
    t[16] = (35, 3); t[17] = (43, 3); t[18] = (51, 3); t[19] = (59, 3);
    t[20] = (67, 4); t[21] = (83, 4); t[22] = (99, 4); t[23] = (115, 4);
    t[24] = (131, 5); t[25] = (163, 5); t[26] = (195, 5); t[27] = (227, 5);
    t[28] = (258, 0);
    t
};

const DIST_TABLE: [(u32, u8); DIST_CODES as usize] = {
    let mut t = [(0u32, 0u8); 32];
    t[0] = (1, 0); t[1] = (2, 0); t[2] = (3, 0); t[3] = (4, 0);
    t[4] = (5, 1); t[5] = (7, 1);
    t[6] = (9, 2); t[7] = (13, 2);
    t[8] = (17, 3); t[9] = (25, 3);
    t[10] = (33, 4); t[11] = (49, 4);
    t[12] = (65, 5); t[13] = (97, 5);
    t[14] = (129, 6); t[15] = (193, 6);
    t[16] = (257, 7); t[17] = (385, 7);
    t[18] = (513, 8); t[19] = (769, 8);
    t[20] = (1025, 9); t[21] = (1537, 9);
    t[22] = (2049, 10); t[23] = (3073, 10);
    t[24] = (4097, 11); t[25] = (6145, 11);
    t[26] = (8193, 12); t[27] = (12289, 12);
    t[28] = (16385, 13); t[29] = (24577, 13);
    t[30] = (32769, 14); t[31] = (49152, 14);
    t
};

fn length_to_code(ln: u32) -> (u16, u32) {
    match ln {
        3 => (256, 0), 4 => (257, 0), 5 => (258, 0), 6 => (259, 0),
        7 => (260, 0), 8 => (261, 0), 9 => (262, 0), 10 => (263, 0),
        11..=12 => (264, ln - 11),
        13..=14 => (265, ln - 13),
        15..=16 => (266, ln - 15),
        17..=18 => (267, ln - 17),
        19..=22 => (268, ln - 19),
        23..=26 => (269, ln - 23),
        27..=30 => (270, ln - 27),
        31..=34 => (271, ln - 31),
        35..=42 => (272, ln - 35),
        43..=50 => (273, ln - 43),
        51..=58 => (274, ln - 51),
        59..=66 => (275, ln - 59),
        67..=82 => (276, ln - 67),
        83..=98 => (277, ln - 83),
        99..=114 => (278, ln - 99),
        115..=130 => (279, ln - 115),
        131..=162 => (280, ln - 131),
        163..=194 => (281, ln - 163),
        195..=226 => (282, ln - 195),
        227..=257 => (283, ln - 227),
        _ => (284, 258),
    }
}

fn match_sym(ln: u32) -> u16 {
    let (code, _) = length_to_code(ln);
    code
}

fn sym_to_match(sym: u16) -> usize {
    (sym - 256) as usize
}

fn distance_to_code(dist: u32) -> (u16, u32, bool) {
    match dist {
        1 => (0, 0, false), 2 => (1, 0, false), 3 => (2, 0, false), 4 => (3, 0, false),
        5..=6 => (4, dist - 5, false),
        7..=8 => (5, dist - 7, false),
        9..=12 => (6, dist - 9, false),
        13..=16 => (7, dist - 13, false),
        17..=24 => (8, dist - 17, false),
        25..=32 => (9, dist - 25, false),
        33..=48 => (10, dist - 33, false),
        49..=64 => (11, dist - 49, false),
        65..=96 => (12, dist - 65, false),
        97..=128 => (13, dist - 97, false),
        129..=192 => (14, dist - 129, false),
        193..=256 => (15, dist - 193, false),
        257..=384 => (16, dist - 257, false),
        385..=512 => (17, dist - 385, false),
        513..=768 => (18, dist - 513, false),
        769..=1024 => (19, dist - 769, false),
        1025..=1536 => (20, dist - 1025, false),
        1537..=2048 => (21, dist - 1537, false),
        2049..=3072 => (22, dist - 2049, false),
        3073..=4096 => (23, dist - 3073, false),
        4097..=6144 => (24, dist - 4097, false),
        6145..=8192 => (25, dist - 6145, false),
        8193..=12288 => (26, dist - 8193, false),
        12289..=16384 => (27, dist - 12289, false),
        16385..=24576 => (28, dist - 16385, false),
        24577..=32768 => (29, dist - 24577, false),
        32769..=49152 => (30, dist - 32769, false),
        _ => (31, dist, true),
    }
}

pub fn compress(data: &[u8]) -> Vec<u8> {
    if data.is_empty() { return Vec::new(); }

    const BLOCK_SIZE_64K: usize = 65536;
    const BLOCK_SIZE_256K: usize = 262144;
    const MIN_LAST_BLOCK: usize = 32768;
    let n = data.len();

    let mut freq = [0u32; 256];
    let mut unique = 0usize;
    for &b in &data[..n.min(65536)] {
        if freq[b as usize] == 0 { unique += 1; }
        freq[b as usize] += 1;
    }

    let block_size = if unique <= 10 {
        BLOCK_SIZE_64K
    } else {
        let max_freq = *freq.iter().max().unwrap_or(&0);
        if max_freq as usize > n.min(65536) / 2 {
            BLOCK_SIZE_256K
        } else if n > 131072 {
            let mut freq2 = [0u32; 256];
            for &b in &data[65536..131072.min(n)] {
                freq2[b as usize] += 1;
            }
            let mut l1 = 0.0f64;
            let sz = 65536.0f64;
            for i in 0..256 {
                l1 += (freq[i] as f64 / sz - freq2[i] as f64 / sz).abs();
            }
            if l1 < 0.15 { BLOCK_SIZE_256K } else { BLOCK_SIZE_64K }
        } else {
            BLOCK_SIZE_64K
        }
    };

    let mut output = Vec::new();
    let mut block_start = 0;

    while block_start < n {
        let mut block_end = (block_start + block_size).min(n);
        let remaining_after = n - block_end;
        if remaining_after > 0 && remaining_after < MIN_LAST_BLOCK {
            block_end = n;
        }
        let win_start = block_start.saturating_sub(WINDOW);
        let ext_data = &data[win_start..block_end];

        let mut block_freq = [0u32; 256];
        let mut unique = 0usize;
        for &b in &data[block_start..block_end] {
            if block_freq[b as usize] == 0 { unique += 1; }
            block_freq[b as usize] += 1;
        }
        let block_len = block_end - block_start;
        let max_freq = *block_freq.iter().max().unwrap_or(&0);
        let low_entropy = unique <= 32 || max_freq * 2 > block_len as u32;
        let mut lit_cost = 8i64;
        if low_entropy {
            let total = block_len as f64;
            let mut entropy = 0.0f64;
            for &count in block_freq.iter() {
                if count > 0 {
                    let p = count as f64 / total;
                    entropy -= p * p.log2();
                }
            }
            lit_cost = (entropy.round() as i64).clamp(2, 8);
        }

        let mut tokens = Vec::new();
        let mut main_freq = vec![0u32; MAIN_SYMS];
        let mut dist_freq = vec![0u32; DIST_CODES as usize];
        let mut pos = block_start - win_start;

        if unique <= 10 {
            while pos < ext_data.len() {
                let b = ext_data[pos];
                tokens.push(Token::Lit(b));
                main_freq[b as usize] += 1;
                pos += 1;
            }
        } else {
            let ht = build_hash(ext_data);
            parse_tokens(ext_data, &ht, &mut tokens, &mut main_freq, &mut dist_freq, &mut pos, low_entropy, lit_cost);
        }

        let main_huff = huff::build(&main_freq);
        let dist_huff = huff::build(&dist_freq);

        let mut w = BitWriter::new();
        for tok in &tokens {
            match *tok {
                Token::Lit(b) => {
                    let (code, len) = main_huff.encode(b as u16);
                    w.write_bits(code, len as u32);
                }
                Token::Match { off, ln } => {
                    let sym = match_sym(ln);
                    let (c, l) = main_huff.encode(sym);
                    w.write_bits(c, l as u32);

                    let (code, extra) = length_to_code(ln);
                    let len_extra = LEN_TABLE[(code - 256) as usize].1 as u32;
                    if code == 284 {
                        if ln > 258 {
                            w.write_bits(1, 1);
                            w.write_vlq(ln - 258);
                        } else {
                            w.write_bits(0, 1);
                        }
                    } else if len_extra > 0 {
                        w.write_bits(extra, len_extra);
                    }

                    let (d_code, d_extra, d_overflow) = distance_to_code(off);
                    let (dc, dl) = dist_huff.encode(d_code);
                    w.write_bits(dc, dl as u32);
                    if d_overflow {
                        w.write_vlq(d_extra);
                    } else {
                        let extra_bits = DIST_TABLE[d_code as usize].1 as u32;
                        if extra_bits > 0 {
                            w.write_bits(d_extra, extra_bits);
                        }
                    }
                }
            }
        }
        let bitstream = w.into_bytes();

        let mut h = BitWriter::new();
        h.write_bits(MAGIC, 32);
        h.write_vlq((block_end - block_start) as u32);
        write_huff_table(&mut h, &main_huff.len);
        write_huff_table(&mut h, &dist_huff.len);
        h.flush();
        let header_bytes = h.into_bytes();

        output.extend_from_slice(&header_bytes);
        output.extend_from_slice(&bitstream);

        block_start = block_end;
    }

    output
}

pub fn decompress(compressed: &[u8]) -> Vec<u8> {
    if compressed.len() < 4 { return Vec::new(); }

    let mut r = BitReader::new(compressed);
    let mut out = Vec::new();

    loop {
        if r.byte_pos() >= compressed.len() - 1 { break; }
        let magic = r.read_bits(32);

        match magic {
            MAGIC_RAW => {
                let n = r.read_vlq() as usize;
                let byte_pos = r.byte_pos();
                if byte_pos + n <= compressed.len() {
                    out.extend_from_slice(&compressed[byte_pos..byte_pos + n]);
                    r.advance_bytes(n);
                }
            }
            MAGIC => {
                let block_n = r.read_vlq() as usize;
                let main_huff = read_huff_table(&mut r, MAIN_SYMS);
                let dist_huff = read_huff_table(&mut r, DIST_CODES as usize);
                r.align();

                let dst = out.len();
                out.reserve(block_n);

                while out.len() - dst < block_n {
                    let sym = main_huff.decode(|| r.read_bit());
                    if sym < 256 {
                        out.push(sym as u8);
                    } else {
                        let code_idx = sym_to_match(sym);
                        let (base_len, extra) = LEN_TABLE[code_idx];
                        let mut ln = base_len as u32;
                        if code_idx == 28 {
                            let marker = r.read_bit();
                            if marker == 1 {
                                ln += r.read_vlq();
                            }
                        } else if extra > 0 {
                            ln += r.read_bits(extra as u32);
                        }

                        let d_sym = dist_huff.decode(|| r.read_bit());
                        let (base_dist, d_extra) = DIST_TABLE[d_sym as usize];
                        let off = if d_sym == 31 {
                            r.read_vlq()
                        } else {
                            base_dist + if d_extra > 0 { r.read_bits(d_extra as u32) } else { 0 }
                        };

                        let src = out.len().wrapping_sub(off as usize);
                        if src + ln as usize <= out.len() {
                            out.extend_from_within(src..src + ln as usize);
                        } else {
                            for i in 0..ln as usize {
                                out.push(out[src + i]);
                            }
                        }
                    }
                }
                r.align();
            }
            _ => break,
        }
    }
    out
}

fn write_huff_table(w: &mut BitWriter, lens: &[u8]) {
    let n = lens.len();
    let mut nonzeros: Vec<(u16, u8)> = Vec::new();
    for i in 0..n as u16 {
        if lens[i as usize] > 0 {
            nonzeros.push((i, lens[i as usize]));
        }
    }
    if nonzeros.len() <= 32 {
        w.write_bit(0);
        w.write_vlq(nonzeros.len() as u32);
        for &(sym, len) in &nonzeros {
            w.write_bits(sym as u32, 16);
            w.write_byte(len);
        }
    } else {
        w.write_bit(1);
        for i in 0..n / 2 {
            let lo = lens[i * 2];
            let hi = lens[i * 2 + 1];
            w.write_byte((hi << 4) | lo);
        }
        if n % 2 == 1 {
            w.write_byte(lens[n - 1] << 4);
        }
    }
}

fn read_huff_table(r: &mut BitReader, n: usize) -> huff::Huffman {
    let mut lens = vec![0u8; n];
    if r.read_bit() == 0 {
        let count = r.read_vlq() as usize;
        for _ in 0..count {
            let sym = r.read_bits(16) as u16;
            let len = r.read_bits(8) as u8;
            lens[sym as usize] = len;
        }
    } else {
        for i in 0..n / 2 {
            let byte = r.read_bits(8) as u8;
            let lo = byte & 0x0f;
            let hi = byte >> 4;
            lens[i * 2] = lo;
            lens[i * 2 + 1] = hi;
        }
        if n % 2 == 1 {
            let last = r.read_bits(8) as u8;
            lens[n - 1] = last >> 4;
        }
    }
    let mut huff = huff::Huffman::new(n);
    huff.len = lens;
    huff.build_tables();
    huff
}

fn lazy_skip(data: &[u8], pos: usize, ln: u32, off: u32, ht: &HashTables, lit_cost: i64) -> bool {
    ln <= 5 && pos + 1 + 5 <= data.len()
        && find_match(data, pos + 1, ht, lit_cost)
            .map(|next| {
                if let Token::Match { off: o2, ln: l2 } = next {
                    let cur_sav = ln as i64 * 8 - match_cost(off, ln);
                    let nxt_sav = (l2 + 1) as i64 * 8 - (6 + match_cost(o2, l2));
                    nxt_sav > cur_sav + 8
                } else { false }
            })
            .unwrap_or(false)
}

fn lazy_skip_cost(data: &[u8], pos: usize, ln: u32, off: u32, ht: &HashTables, lit_cost: i64) -> bool {
    ln <= 5 && pos + 1 + 5 <= data.len()
        && find_match(data, pos + 1, ht, lit_cost)
            .map(|next| {
                if let Token::Match { off: o2, ln: l2 } = next {
                    let skip_cost = lit_cost + match_cost(o2, l2);
                    let take_cost = match_cost(off, ln);
                    skip_cost < take_cost
                } else { false }
            })
            .unwrap_or(false)
}

fn parse_tokens(data: &[u8], ht: &HashTables,
                tokens: &mut Vec<Token>, main_freq: &mut [u32], dist_freq: &mut [u32], pos: &mut usize, low_entropy: bool, lit_cost: i64) {
    let n = data.len();
    while *pos < n {
        if let Some(tok) = find_match(data, *pos, ht, lit_cost) {
            if let Token::Match { off, ln } = tok {
                let lazy = if low_entropy {
                    lazy_skip_cost(data, *pos, ln, off, ht, lit_cost)
                } else {
                    lazy_skip(data, *pos, ln, off, ht, lit_cost)
                };
                if lazy {
                    tokens.push(Token::Lit(data[*pos]));
                    main_freq[data[*pos] as usize] += 1;
                    *pos += 1;
                    continue;
                }
                tokens.push(tok);
                let sym = match_sym(ln);
                main_freq[sym as usize] += 1;
                let (d_code, _, _) = distance_to_code(off);
                dist_freq[d_code as usize] += 1;
                *pos += ln as usize;
            }
        } else {
            tokens.push(Token::Lit(data[*pos]));
            main_freq[data[*pos] as usize] += 1;
            *pos += 1;
        }
    }
}
