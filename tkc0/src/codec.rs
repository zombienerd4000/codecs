use std::collections::HashMap;
use crate::bit::{BitWriter, BitReader};
use crate::rans::{self, RansEncoder, RansDecoder};
use crate::lz::{Token, Transform, find_match};

const MAGIC_RANS: u32 = 0x544D;  // "TM"
const MAGIC_LZ: u32   = 0x544C;  // "TL" - no literals, LZ only
const MAGIC_RAW: u32  = 0x5450;  // "TP" - passthrough (incompressible)

pub fn compress(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let (ht_xor, ht_add) = build_hash_ffi(data);

    let mut tokens = Vec::new();
    let mut lits = Vec::new();
    let n = data.len();
    let mut pos = 0usize;
    while pos < n {
        if let Some(tok) = find_match(data, pos, &ht_xor, &ht_add) {
            if let Token::Match { ln, .. } = tok {
                tokens.push(tok);
                pos += ln as usize;
            } else {
                unreachable!();
            }
        } else {
            tokens.push(Token::Lit(data[pos]));
            lits.push(data[pos]);
            pos += 1;
        }
    }

    let rans_data = if lits.is_empty() {
        Vec::new()
    } else {
        let mut counts = HashMap::new();
        for &b in &lits {
            *counts.entry(b).or_insert(0u32) += 1;
        }
        let (table, _total) = rans::freq_to_cumul(&counts);
        let enc = RansEncoder::new();
        enc.encode_all(&lits, &table)
    };

    let mut w = BitWriter::new();
    for tok in &tokens {
        match *tok {
            Token::Lit(_) => w.write_bit(0),
            Token::Match { off, ln, t, param } => {
                w.write_bit(1);
                w.write_bits(t as u32, 2);
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
        h.write_bits(if lits.is_empty() { MAGIC_LZ } else { MAGIC_RANS }, 16);
        h.write_vlq(n as u32);
        if lits.is_empty() {
            h.write_vlq(0);
        } else {
            let mut counts = HashMap::new();
            for &b in &lits {
                *counts.entry(b).or_insert(0u32) += 1;
            }
            h.write_vlq(counts.len() as u32);
            let mut sorted: Vec<_> = counts.iter().collect();
            sorted.sort_by_key(|(&k, _)| k);
            for (&sym, &cnt) in &sorted {
                h.write_byte(sym);
                h.write_vlq(cnt);
            }
            h.write_vlq(rans_data.len() as u32);
        }
        h.flush();
        let header = h.into_bytes();
        header.into_iter().chain(rans_data).chain(bitstream).collect()
    };

    // passthrough if expansion
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
    if compressed.len() < 4 {
        return Vec::new();
    }

    let mut r = BitReader::new(compressed);
    let magic = r.read_bits(16);

    match magic {
        MAGIC_RAW => {
            // Passthrough: raw data stored as-is
            let n = r.read_vlq() as usize;
            let remaining = &compressed[r.byte_pos()..];
            if remaining.len() >= n {
                remaining[..n].to_vec()
            } else {
                Vec::new()
            }
        }
        MAGIC_LZ | MAGIC_RANS => {
            let n = r.read_vlq() as usize;
            let num_syms = r.read_vlq();

            let (table, mut rd_state, rans_len) = if magic == MAGIC_LZ || num_syms == 0 {
                // No literals: skip to bitstream
                (vec![], None, 0usize)
            } else {
                let mut raw_counts = HashMap::new();
                for _ in 0..num_syms {
                    let sym = r.read_bits(8) as u8;
                    let cnt = r.read_vlq();
                    raw_counts.insert(sym, cnt);
                }
                let rans_len = r.read_vlq() as usize;
                let rans_bytes = &compressed[r.byte_pos()..r.byte_pos() + rans_len];
                let (table, _) = rans::freq_to_cumul(&raw_counts);
                let dec = RansDecoder::new(rans_bytes);
                (table, Some(dec), rans_len)
            };

            let bit_start = r.byte_pos() + rans_len;
            let mut r2 = BitReader::new(&compressed[bit_start..]);

            let mut out = Vec::with_capacity(n);

            while out.len() < n {
                let is_match = r2.read_bit();
                if is_match == 0 {
                    if let Some(ref mut dec) = rd_state {
                        let slot = dec.state() & ((1 << 12) - 1);
                        let idx = table.iter()
                            .position(|s| slot >= s.start && slot < s.start + s.freq)
                            .unwrap();
                        let s = &table[idx];
                        dec.decode(s.freq, s.start);
                        out.push(s.sym);
                    } else {
                        out.push(r2.read_bits(8) as u8);
                    }
                } else {
                    let t_code = r2.read_bits(2);
                    let off = read_offset(&mut r2);
                    let ln = read_length(&mut r2);
                    let p = if t_code != 0 { r2.read_bits(8) as u8 } else { 0 };

                    let src = out.len() - off as usize;
                    for i in 0..ln as usize {
                        let b = out[src + i];
                        out.push(match t_code {
                            0 => b,
                            1 => b ^ p,
                            2 => b.wrapping_add(p),
                            _ => b.wrapping_sub(p),
                        });
                    }
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

fn build_hash_ffi(data: &[u8]) -> (HashMap<u64, Vec<u32>>, HashMap<u64, Vec<u32>>) {
    crate::lz::build_hash(data)
}

fn write_offset(w: &mut BitWriter, off: u32) {
    if off <= 8 {
        w.write_bit(0); w.write_bits(off - 1, 3);
    } else if off <= 40 {
        w.write_bits(2, 2); w.write_bits(off - 9, 5);
    } else if off <= 168 {
        w.write_bits(6, 3); w.write_bits(off - 41, 7);
    } else if off <= 1192 {
        w.write_bits(14, 4); w.write_bits(off - 169, 10);
    } else {
        w.write_bits(15, 4); w.write_vlq(off);
    }
}

fn read_offset(r: &mut BitReader) -> u32 {
    if r.read_bit() == 0 { return 1 + r.read_bits(3); }
    if r.read_bit() == 0 { return 9 + r.read_bits(5); }
    if r.read_bit() == 0 { return 41 + r.read_bits(7); }
    if r.read_bit() == 0 { return 169 + r.read_bits(10); }
    r.read_vlq()
}

fn write_length(w: &mut BitWriter, ln: u32) {
    if ln <= 10 {
        w.write_bit(0); w.write_bits(ln - 3, 3);
    } else if ln <= 26 {
        w.write_bits(2, 2); w.write_bits(ln - 11, 4);
    } else if ln <= 90 {
        w.write_bits(6, 3); w.write_bits(ln - 27, 6);
    } else {
        w.write_bits(7, 3); w.write_vlq(ln);
    }
}

fn read_length(r: &mut BitReader) -> u32 {
    if r.read_bit() == 0 { return 3 + r.read_bits(3); }
    if r.read_bit() == 0 { return 11 + r.read_bits(4); }
    if r.read_bit() == 0 { return 27 + r.read_bits(6); }
    r.read_vlq()
}
