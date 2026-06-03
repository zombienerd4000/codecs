use crate::bit::{BitWriter, BitReader};
use crate::lz::{Token, Transform, find_match, match_cost, build_hash, HashTables, WINDOW};
use crate::huff;

const MAGIC: u32 = 0x5443;
const MAGIC_RAW: u32 = 0x5451;
const LEN_CODES: usize = 29;

const LEN_TABLE: [(u16, u8); LEN_CODES] = {
    let mut t = [(0u16, 0u8); LEN_CODES];
    t[0] = (3, 0); t[1] = (4, 0); t[2] = (5, 0); t[3] = (6, 0);
    t[4] = (7, 0); t[5] = (8, 0); t[6] = (9, 0); t[7] = (10, 0);
    t[8] = (11, 1); t[9] = (13, 1); t[10] = (15, 1); t[11] = (17, 1);
    t[12] = (19, 2); t[13] = (23, 2); t[14] = (27, 2); t[15] = (31, 2);
    t[16] = (35, 3); t[17] = (43, 3); t[18] = (51, 3); t[19] = (59, 3);
    t[20] = (67, 4); t[21] = (83, 4); t[22] = (99, 4); t[23] = (115, 4);
    t[24] = (131, 5); t[25] = (163, 5); t[26] = (195, 5); t[27] = (227, 5);
    t[28] = (258, 1);
    t
};

const DIST_CODES: usize = 32;
const DIST_TABLE: [(u32, u8); DIST_CODES] = {
    let mut t = [(0u32, 0u8); DIST_CODES];
    t[0] = (1, 0); t[1] = (2, 0); t[2] = (3, 0); t[3] = (4, 0);
    t[4] = (5, 1); t[5] = (7, 1); t[6] = (9, 2); t[7] = (13, 2);
    t[8] = (17, 3); t[9] = (25, 3); t[10] = (33, 4); t[11] = (49, 4);
    t[12] = (65, 5); t[13] = (97, 5); t[14] = (129, 6); t[15] = (193, 6);
    t[16] = (257, 7); t[17] = (385, 7); t[18] = (513, 8); t[19] = (769, 8);
    t[20] = (1025, 9); t[21] = (1537, 9); t[22] = (2049, 10); t[23] = (3073, 10);
    t[24] = (4097, 11); t[25] = (6145, 11); t[26] = (8193, 12); t[27] = (12289, 12);
    t[28] = (16385, 13); t[29] = (24577, 13); t[30] = (32769, 14); t[31] = (0, 0);
    t
};

const MAIN_SYMS: usize = 256 + LEN_CODES * 4;  // 372

fn transform_to_idx(t: Transform) -> u16 {
    match t {
        Transform::Exact => 0,
        Transform::Xor => 1,
        Transform::Add => 2,
        Transform::Sub => 3,
    }
}

fn length_to_code(ln: u32) -> (u16, u32) {
    match ln {
        3 => (256, 0), 4 => (257, 0), 5 => (258, 0), 6 => (259, 0),
        7 => (260, 0), 8 => (261, 0), 9 => (262, 0), 10 => (263, 0),
        11..=12 => (264, (ln - 11) as u32),
        13..=14 => (265, (ln - 13) as u32),
        15..=16 => (266, (ln - 15) as u32),
        17..=18 => (267, (ln - 17) as u32),
        19..=22 => (268, (ln - 19) as u32),
        23..=26 => (269, (ln - 23) as u32),
        27..=30 => (270, (ln - 27) as u32),
        31..=34 => (271, (ln - 31) as u32),
        35..=42 => (272, (ln - 35) as u32),
        43..=50 => (273, (ln - 43) as u32),
        51..=58 => (274, (ln - 51) as u32),
        59..=66 => (275, (ln - 59) as u32),
        67..=82 => (276, (ln - 67) as u32),
        83..=98 => (277, (ln - 83) as u32),
        99..=114 => (278, (ln - 99) as u32),
        115..=130 => (279, (ln - 115) as u32),
        131..=162 => (280, (ln - 131) as u32),
        163..=194 => (281, (ln - 163) as u32),
        195..=226 => (282, (ln - 195) as u32),
        227..=257 => (283, (ln - 227) as u32),
        _ => (284, 258),
    }
}

fn match_sym(ln: u32, t: Transform) -> u16 {
    let (code, _) = length_to_code(ln);
    256 + (code - 256) * 4 + transform_to_idx(t)
}

fn sym_to_match(sym: u16) -> (usize, u32) {
    let raw = sym - 256;
    let code_idx = (raw / 4) as usize;
    let t_code = (raw % 4) as u32;
    (code_idx, t_code)
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

    const BLOCK_SIZE: usize = 262144;
    let n = data.len();

    let mut output = Vec::new();
    let mut block_start = 0;

    while block_start < n {
        let block_end = (block_start + BLOCK_SIZE).min(n);
        let win_start = block_start.saturating_sub(WINDOW);
        let ext_data = &data[win_start..block_end];

        let ht = build_hash(ext_data);

        let mut tokens = Vec::new();
        let mut main_freq = vec![0u32; MAIN_SYMS];
        let mut dist_freq = vec![0u32; DIST_CODES];
        let mut pos = block_start - win_start;
        parse_tokens(ext_data, &ht, &mut tokens, &mut main_freq, &mut dist_freq, &mut pos);

        let main_huff = huff::build(&main_freq);
        let dist_huff = huff::build(&dist_freq);

        let mut w = BitWriter::new();
        for tok in &tokens {
            match *tok {
                Token::Lit(b) => {
                    let (code, len) = main_huff.encode(b as u16);
                    w.write_bits(code, len as u32);
                }
                Token::Match { off, ln, t, param } => {
                    let sym = match_sym(ln, t);
                    let (c, l) = main_huff.encode(sym);
                    w.write_bits(c, l as u32);

                    let (code, extra) = length_to_code(ln);
                    let len_extra = LEN_TABLE[(code - 256) as usize].1 as u32;
                    if len_extra > 0 {
                        if code == 284 && ln > 258 {
                            w.write_bits(1, 1);
                            w.write_vlq(ln - 258);
                        } else {
                            w.write_bits(extra, len_extra);
                        }
                    }

                    if t != Transform::Exact {
                        w.write_byte(param);
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
        h.write_bits(MAGIC, 16);
        h.write_vlq((block_end - block_start) as u32);
        write_huff_table(&mut h, &main_huff.len);
        write_huff_table(&mut h, &dist_huff.len);
        h.flush();
        output.extend(h.into_bytes());
        output.extend(bitstream);

        block_start = block_end;
    }

    if output.len() >= n + 8 {
        let mut raw = BitWriter::new();
        raw.write_bits(MAGIC_RAW, 16);
        raw.write_vlq(n as u32);
        raw.flush();
        return raw.into_bytes().into_iter().chain(data.iter().copied()).collect();
    }

    output
}

pub fn decompress(compressed: &[u8]) -> Vec<u8> {
    if compressed.len() < 4 { return Vec::new(); }

    let mut r = BitReader::new(compressed);
    let mut out = Vec::new();

    loop {
        if r.byte_pos() >= compressed.len() - 1 { break; }
        let magic = r.read_bits(16);

        match magic {
            MAGIC_RAW => {
                let n = r.read_vlq() as usize;
                let remaining = &compressed[r.byte_pos()..];
                if remaining.len() >= n {
                    out.extend_from_slice(&remaining[..n]);
                }
                break;
            }
            MAGIC => {
                let block_n = r.read_vlq() as usize;
                let main_huff = read_huff_table(&mut r, MAIN_SYMS);
                let dist_huff = read_huff_table(&mut r, DIST_CODES);
                r.align();

                let dst = out.len();
                out.reserve(block_n);

                while out.len() - dst < block_n {
                    let sym = main_huff.decode(|| r.read_bit());
                    if sym < 256 {
                        out.push(sym as u8);
                    } else {
                        let (code_idx, t_code) = sym_to_match(sym);
                        let (base_len, extra) = LEN_TABLE[code_idx];
                        let mut ln = base_len as u32;
                        if extra > 0 {
                            if code_idx == 28 {
                                let marker = r.read_bit();
                                if marker == 1 {
                                    ln += r.read_vlq();
                                }
                            } else {
                                ln += r.read_bits(extra as u32);
                            }
                        }

                        let p = if t_code != 0 { r.read_bits(8) as u8 } else { 0u8 };

                        let d_sym = dist_huff.decode(|| r.read_bit());
                        let (base_dist, d_extra) = DIST_TABLE[d_sym as usize];
                        let off = if d_sym == 31 {
                            r.read_vlq()
                        } else {
                            base_dist + if d_extra > 0 { r.read_bits(d_extra as u32) } else { 0 }
                        };

                        let src = out.len().wrapping_sub(off as usize);
                        for i in 0..ln as usize {
                            let val: u8 = out[src + i];
                            out.push(match t_code {
                                0 => val,
                                1 => val ^ p,
                                2 => val.wrapping_add(p),
                                _ => val.wrapping_sub(p),
                            });
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

fn parse_tokens(data: &[u8], ht: &HashTables, tokens: &mut Vec<Token>, main_freq: &mut [u32], dist_freq: &mut [u32], pos: &mut usize) {
    let n = data.len();
    while *pos < n {
        if let Some(tok) = find_match(data, *pos, ht) {
            if let Token::Match { off, ln, t, param: _ } = tok {
                let lazy = ln <= 5 && *pos + 1 + 5 <= n
                    && find_match(data, *pos + 1, ht)
                        .map(|next| {
                            if let Token::Match { off: o2, ln: l2, t: t2, .. } = next {
                                let skip_cost = 6 + match_cost(o2, l2, t2);
                                let take_cost = match_cost(off, ln, t);
                                skip_cost < take_cost
                            } else { false }
                        })
                        .unwrap_or(false);
                if lazy {
                    let b = data[*pos];
                    tokens.push(Token::Lit(b));
                    main_freq[b as usize] += 1;
                    *pos += 1;
                    continue;
                }
                tokens.push(tok);
                let sym = match_sym(ln, t);
                main_freq[sym as usize] += 1;
                let (d_code, _, _) = distance_to_code(off);
                dist_freq[d_code as usize] += 1;
                *pos += ln as usize;
            }
        } else {
            let b = data[*pos];
            tokens.push(Token::Lit(b));
            main_freq[b as usize] += 1;
            *pos += 1;
        }
    }
}
