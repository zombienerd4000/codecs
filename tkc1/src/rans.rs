const RANS_L: u32 = 1 << 23;
const SCALE_BITS: u32 = 12;
const SCALE: u32 = 1 << SCALE_BITS;
const INIT_FREQ: u16 = 16;
const RESCALE_TARGET: u32 = 2 * SCALE;

pub struct RansEncoder {
    state: u32,
    freq: [u16; 256],
    cumul: [u32; 257],
    total: u32,
}

impl RansEncoder {
    pub fn new() -> Self {
        let freq = [INIT_FREQ; 256];
        let mut cumul = [0u32; 257];
        let mut c = 0u32;
        for i in 0..256 {
            cumul[i] = c;
            c += freq[i] as u32;
        }
        cumul[256] = c;
        RansEncoder { state: RANS_L, freq, cumul, total: c }
    }

    fn rescale(&mut self) {
        for f in self.freq.iter_mut() {
            *f = (*f / 2).max(1);
        }
        let mut c = 0u32;
        for i in 0..256 {
            self.cumul[i] = c;
            c += self.freq[i] as u32;
        }
        self.cumul[256] = c;
        self.total = c;
    }

    fn update(&mut self, sym: u8) {
        let s = sym as usize;
        self.freq[s] += 1;
        self.total += 1;
        for i in (s + 1)..=256 {
            self.cumul[i] += 1;
        }
        if self.total > RESCALE_TARGET {
            self.rescale();
        }
    }

    /// Encode one symbol. Returns renorm bytes for this symbol.
    pub fn encode(&mut self, sym: u8) -> Vec<u8> {
        let s = sym as usize;
        let c = self.cumul[s];
        let f = self.freq[s] as u32;
        let x_max = ((RANS_L >> SCALE_BITS) << 8) * f;
        let mut x = self.state;
        let mut ren = Vec::new();
        if x >= x_max {
            loop {
                ren.push((x & 0xff) as u8);
                x >>= 8;
                if x < x_max { break; }
            }
        }
        x = ((x / f) << SCALE_BITS) + (x % f) + c;
        self.state = x;
        self.update(sym);
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

    /// Encode all symbols in forward order.
    /// Returns [state(4B)][ren_0][ren_1]...[ren_{n-1}]
    pub fn encode_all(mut self, symbols: &[u8]) -> Vec<u8> {
        let n = symbols.len();
        let mut blocks: Vec<Vec<u8>> = Vec::with_capacity(n);
        for &sym in symbols {
            let ren = self.encode(sym);
            blocks.push(ren);
        }
        let mut out = self.flush();
        for block in &blocks {
            out.extend(block);
        }
        out
    }
}

pub struct RansDecoder {
    state: u32,
    data: Vec<u8>,
    ptr: usize,
    freq: [u16; 256],
    cumul: [u32; 257],
    total: u32,
}

impl RansDecoder {
    pub fn new(data: &[u8]) -> Self {
        let state = (data[0] as u32)
            | ((data[1] as u32) << 8)
            | ((data[2] as u32) << 16)
            | ((data[3] as u32) << 24);
        let freq = [INIT_FREQ; 256];
        let mut cumul = [0u32; 257];
        let mut c = 0u32;
        for i in 0..256 {
            cumul[i] = c;
            c += freq[i] as u32;
        }
        cumul[256] = c;
        RansDecoder {
            state,
            data: data.to_vec(),
            ptr: 4,
            freq, cumul, total: c,
        }
    }

    fn rescale(&mut self) {
        for f in self.freq.iter_mut() {
            *f = (*f / 2).max(1);
        }
        let mut c = 0u32;
        for i in 0..256 {
            self.cumul[i] = c;
            c += self.freq[i] as u32;
        }
        self.cumul[256] = c;
        self.total = c;
    }

    fn update(&mut self, sym: u8) {
        let s = sym as usize;
        self.freq[s] += 1;
        self.total += 1;
        for i in (s + 1)..=256 {
            self.cumul[i] += 1;
        }
        if self.total > RESCALE_TARGET {
            self.rescale();
        }
    }

    /// Find symbol for a given slot value via linear scan.
    fn find_sym(&self, slot: u32) -> u8 {
        for s in 0..256u16 {
            let s = s as usize;
            if slot < self.cumul[s + 1] {
                return s as u8;
            }
        }
        255
    }

    /// Decode one symbol, update frequencies, return decoded byte.
    pub fn decode(&mut self) -> u8 {
        let slot = self.state & (SCALE - 1);
        let sym = self.find_sym(slot);
        let s = sym as usize;
        let c = self.cumul[s];
        let f = self.freq[s] as u32;
        let mut x = f.wrapping_mul(self.state >> SCALE_BITS)
            .wrapping_add(slot)
            .wrapping_sub(c);
        if x < RANS_L {
            loop {
                x = (x << 8) | self.data[self.ptr] as u32;
                self.ptr += 1;
                if x >= RANS_L { break; }
            }
        }
        self.state = x;
        self.update(sym);
        sym
    }

    /// Decode n symbols, return decoded bytes.
    pub fn decode_all(&mut self, n: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.decode());
        }
        out
    }
}
