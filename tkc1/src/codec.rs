use crate::bit::{BitWriter, BitReader};
use crate::lz::{Token, Transform, find_match, match_cost, build_hash, HashTables};
use crate::huff;

const MAGIC: u32 = 0x544C;
const MAGIC_RAW: u32 = 0x5450;
const MATCH_SYM: u16 = 256;

pub fn compress(data: &[u8]) -> Vec<u8> {
    if data.is_empty() { return Vec::new(); }

    let ht = build_hash(data);
    let n = data.len();

    let mut tokens = Vec::new();
    let mut freq = [0u32; 257];
    let mut pos = 0;
    parse_tokens(data, n, &ht, &mut tokens, &mut freq, &mut pos);

    let huff = huff::build(&freq);

    let mut w = BitWriter::new();
    for tok in &tokens {
        match *tok {
            Token::Lit(b) => {
                let (code, len) = huff.encode(b as u16);
                w.write_bits(code, len as u32);
            }
            Token::Match { off, ln, t, param } => {
                let (code, len) = huff.encode(MATCH_SYM);
                w.write_bits(code, len as u32);
                match t {
                    Transform::Exact => w.write_bit(0),
                    Transform::Xor => { w.write_bit(1); w.write_bit(0); }
                    Transform::Add => { w.write_bit(1); w.write_bit(1); w.write_bit(0); }
                    Transform::Sub => { w.write_bit(1); w.write_bit(1); w.write_bit(1); }
                }
                write_offset(&mut w, off);
                write_length(&mut w, ln);
                if t != Transform::Exact {
                    w.write_byte(param);
                }
            }
        }
    }
    let bitstream = w.into_bytes();

    let compressed: Vec<u8> = {
        let mut h = BitWriter::new();
        h.write_bits(MAGIC, 16);
        h.write_vlq(n as u32);
        write_huff_table(&mut h, &huff);
        h.flush();
        let header = h.into_bytes();
        header.into_iter().chain(bitstream).collect()
    };

    if compressed.len() >= n + 8 {
        let mut raw = BitWriter::new();
        raw.write_bits(MAGIC_RAW, 16);
        raw.write_vlq(n as u32);
        raw.flush();
        let hdr = raw.into_bytes();
        return hdr.into_iter().chain(data.iter().copied()).collect();
    }

    compressed
}

pub fn decompress(compressed: &[u8]) -> Vec<u8> {
    if compressed.len() < 4 { return Vec::new(); }

    let mut r = BitReader::new(compressed);
    let magic = r.read_bits(16);

    match magic {
        MAGIC_RAW => {
            let n = r.read_vlq() as usize;
            let remaining = &compressed[r.byte_pos()..];
            if remaining.len() >= n { remaining[..n].to_vec() } else { Vec::new() }
        }
        MAGIC => {
            let n = r.read_vlq() as usize;
            let huff = read_huff_table(&mut r);
            r.align();

            let mut out: Vec<u8> = Vec::with_capacity(n);

            while out.len() < n {
                let sym = huff.decode(|| r.read_bit());
                if sym == MATCH_SYM {
                    let t_code: u32 = {
                        if r.read_bit() == 0 { 0 }
                        else if r.read_bit() == 0 { 1 }
                        else if r.read_bit() == 0 { 2 }
                        else { 3 }
                    };
                    let off = read_offset(&mut r);
                    let ln = read_length(&mut r);
                    let p = if t_code != 0 { r.read_bits(8) as u8 } else { 0u8 };

                    let src: usize = out.len().wrapping_sub(off as usize);
                    for i in 0..ln as usize {
                        let val: u8 = out[src + i];
                        out.push(match t_code {
                            0 => val,
                            1 => val ^ p,
                            2 => val.wrapping_add(p),
                            _ => val.wrapping_sub(p),
                        });
                    }
                } else {
                    out.push(sym as u8);
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

fn write_huff_table(w: &mut BitWriter, huff: &crate::huff::Huffman) {
    let mut nonzeros: Vec<(u16, u8)> = Vec::new();
    for i in 0..257u16 {
        if huff.len[i as usize] > 0 {
            nonzeros.push((i, huff.len[i as usize]));
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
        for i in 0..128 {
            let lo = huff.len[i * 2];
            let hi = huff.len[i * 2 + 1];
            w.write_byte((hi << 4) | lo);
        }
        // last symbol (256) goes in the upper nibble
        w.write_byte(huff.len[256] << 4);
    }
}

fn read_huff_table(r: &mut BitReader) -> crate::huff::Huffman {
    let mut huff = crate::huff::Huffman::new();
    if r.read_bit() == 0 {
        let count = r.read_vlq() as usize;
        for _ in 0..count {
            let sym = r.read_bits(16) as u16;
            let len = r.read_bits(8) as u8;
            huff.len[sym as usize] = len;
        }
    } else {
        for i in 0..128 {
            let byte = r.read_bits(8) as u8;
            let lo = byte & 0x0f;
            let hi = byte >> 4;
            huff.len[i * 2] = lo;
            huff.len[i * 2 + 1] = hi;
        }
        let last = r.read_bits(8) as u8;
        huff.len[256] = last >> 4;
    }
    huff.build_tables();
    huff
}

fn write_offset(w: &mut BitWriter, off: u32) {
    if off <= 8 { w.write_bit(0); w.write_bits(off - 1, 3); }
    else if off <= 40 { w.write_bits(2, 2); w.write_bits(off - 9, 5); }
    else if off <= 168 { w.write_bits(6, 3); w.write_bits(off - 41, 7); }
    else if off <= 1192 { w.write_bits(14, 4); w.write_bits(off - 169, 10); }
    else if off <= 11272 { w.write_bits(30, 5); w.write_bits(off - 1193, 14); }
    else { w.write_bits(31, 5); w.write_vlq(off); }
}

fn read_offset(r: &mut BitReader) -> u32 {
    if r.read_bit() == 0 { return 1 + r.read_bits(3); }
    if r.read_bit() == 0 { return 9 + r.read_bits(5); }
    if r.read_bit() == 0 { return 41 + r.read_bits(7); }
    if r.read_bit() == 0 { return 169 + r.read_bits(10); }
    if r.read_bit() == 0 { return 1193 + r.read_bits(14); }
    r.read_vlq()
}

fn write_length(w: &mut BitWriter, ln: u32) {
    if ln <= 10 { w.write_bit(0); w.write_bits(ln - 3, 3); }
    else if ln <= 26 { w.write_bits(2, 2); w.write_bits(ln - 11, 4); }
    else if ln <= 90 { w.write_bits(6, 3); w.write_bits(ln - 27, 6); }
    else { w.write_bits(7, 3); w.write_vlq(ln); }
}

fn read_length(r: &mut BitReader) -> u32 {
    if r.read_bit() == 0 { return 3 + r.read_bits(3); }
    if r.read_bit() == 0 { return 11 + r.read_bits(4); }
    if r.read_bit() == 0 { return 27 + r.read_bits(6); }
    r.read_vlq()
}

fn parse_tokens(data: &[u8], n: usize, ht: &HashTables, tokens: &mut Vec<Token>, freq: &mut [u32; 257], pos: &mut usize) {
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
                    tokens.push(Token::Lit(data[*pos]));
                    freq[data[*pos] as usize] += 1;
                    *pos += 1;
                    continue;
                }
                tokens.push(tok);
                freq[256] += 1;
                *pos += ln as usize;
            }
        } else {
            tokens.push(Token::Lit(data[*pos]));
            freq[data[*pos] as usize] += 1;
            *pos += 1;
        }
    }
}
