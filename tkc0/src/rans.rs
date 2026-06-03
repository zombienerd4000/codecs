use std::collections::HashMap;

const RANS_L: u32 = 1 << 23;
const SCALE_BITS: u32 = 12;
const SCALE: u32 = 1 << SCALE_BITS;

#[derive(Clone)]
pub struct RansSym {
    pub sym: u8,
    pub freq: u32,
    pub start: u32,
}

pub fn freq_to_cumul(counts: &HashMap<u8, u32>) -> (Vec<RansSym>, u32) {
    if counts.is_empty() {
        return (vec![], 0);
    }
    let total: u32 = counts.values().sum();

    let raw: Vec<(u8, u32)> = counts.iter().map(|(&s, &c)| (s, c)).collect();
    let mut nf: Vec<(u8, u32)> = raw
        .iter()
        .map(|&(s, c)| (s, (c * SCALE / total).max(1)))
        .collect();

    let sum: u32 = nf.iter().map(|&(_, f)| f).sum();
    let mut diff = SCALE as i32 - sum as i32;

    if diff > 0 {
        nf.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for (_, f) in nf.iter_mut() {
            if diff <= 0 { break; }
            *f += 1; diff -= 1;
        }
    } else if diff < 0 {
        nf.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        for (_, f) in nf.iter_mut() {
            if diff >= 0 || *f <= 1 { break; }
            *f -= 1; diff += 1;
        }
    }

    nf.sort_by_key(|&(s, _)| s);
    let mut cumul = 0u32;
    let mut table = Vec::with_capacity(nf.len());
    for (s, freq) in &nf {
        table.push(RansSym { sym: *s, freq: *freq, start: cumul });
        cumul += freq;
    }
    (table, cumul)
}

pub struct RansEncoder {
    state: u32,
}

impl RansEncoder {
    pub fn new() -> Self {
        RansEncoder { state: RANS_L }
    }

    /// Encodes one symbol and returns the renorm bytes (which must be written
    /// to the output stream BEFORE the caller moves on to older symbols).
    /// Returns renorm bytes (empty vector if no renorm needed).
    pub fn encode(&mut self, freq: u32, start: u32) -> Vec<u8> {
        let x_max = ((RANS_L >> SCALE_BITS) << 8) * freq;
        let mut x = self.state;
        let mut ren = Vec::new();
        if x >= x_max {
            loop {
                ren.push((x & 0xff) as u8);
                x >>= 8;
                if x < x_max { break; }
            }
        }
        x = ((x / freq) << SCALE_BITS) + (x % freq) + start;
        self.state = x;
        ren
    }

    pub fn flush(self) -> Vec<u8> {
        let x = self.state;
        vec![
            (x >> 0) as u8,
            (x >> 8) as u8,
            (x >> 16) as u8,
            (x >> 24) as u8,
        ]
    }

    /// Encode all symbols. Returns [state(4B)][ren(0)][ren(1)]...[ren(N-1)]
    /// where ren(i) is the renorm bytes for the i-th symbol in decode order.
    pub fn encode_all(mut self, symbols: &[u8], table: &[RansSym]) -> Vec<u8> {
        let n = symbols.len();
        let mut blocks: Vec<Vec<u8>> = Vec::with_capacity(n);
        // Encode in reverse (last symbol first, which is the first in decode order)
        for &sym in symbols.iter().rev() {
            let idx = table.iter().position(|s| s.sym == sym).unwrap();
            let s = &table[idx];
            let ren = self.encode(s.freq, s.start);
            blocks.push(ren);
        }
        // blocks = [ren(N-1), ren(N-2), ..., ren(0)]
        // Assemble: [state(4B)] ++ ren(0) ++ ren(1) ++ ... ++ ren(N-1)
        let mut out = self.flush();
        for block in blocks.into_iter().rev() {
            out.extend(block);
        }
        out
    }
}

pub struct RansDecoder<'a> {
    state: u32,
    data: &'a [u8],
    ptr: usize,
}

impl<'a> RansDecoder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let state = (data[0] as u32)
            | ((data[1] as u32) << 8)
            | ((data[2] as u32) << 16)
            | ((data[3] as u32) << 24);
        RansDecoder { state, data, ptr: 4 }
    }

    pub fn decode(&mut self, freq: u32, start: u32) {
        let slot = self.state & (SCALE - 1);
        let mut x = freq.wrapping_mul(self.state >> SCALE_BITS).wrapping_add(slot).wrapping_sub(start);
        if x < RANS_L {
            loop {
                x = (x << 8) | self.data[self.ptr] as u32;
                self.ptr += 1;
                if x >= RANS_L { break; }
            }
        }
        self.state = x;
    }

    pub fn decode_all(&mut self, n: usize, table: &[RansSym]) -> Vec<u8> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let slot = self.state & (SCALE - 1);
            let idx = table.iter()
                .position(|s| slot >= s.start && slot < s.start + s.freq)
                .unwrap();
            let s = &table[idx];
            let mut x = s.freq.wrapping_mul(self.state >> SCALE_BITS)
                .wrapping_add(slot)
                .wrapping_sub(s.start);
            if x < RANS_L {
                loop {
                    x = (x << 8) | self.data[self.ptr] as u32;
                    self.ptr += 1;
                    if x >= RANS_L { break; }
                }
            }
            self.state = x;
            out.push(s.sym);
        }
        out
    }

    pub fn state(&self) -> u32 { self.state }
}
