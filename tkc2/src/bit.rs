pub struct BitWriter {
    buf: Vec<u8>,
    byte: u32,
    bits: u32,
}

impl BitWriter {
    pub fn new() -> Self {
        BitWriter { buf: Vec::new(), byte: 0, bits: 0 }
    }

    pub fn write_bit(&mut self, b: u32) {
        self.byte = (self.byte << 1) | (b & 1);
        self.bits += 1;
        if self.bits == 8 {
            self.buf.push(self.byte as u8);
            self.byte = 0;
            self.bits = 0;
        }
    }

    pub fn write_bits(&mut self, val: u32, n: u32) {
        for i in (0..n).rev() {
            self.write_bit((val >> i) & 1);
        }
    }

    pub fn write_vlq(&mut self, mut v: u32) {
        loop {
            let mut byte = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 { byte |= 0x80; }
            self.write_bits(byte as u32, 8);
            if v == 0 { break; }
        }
    }

    pub fn write_byte(&mut self, b: u8) {
        self.write_bits(b as u32, 8);
    }

    pub fn flush(&mut self) {
        if self.bits > 0 {
            self.byte <<= 8 - self.bits;
            self.buf.push(self.byte as u8);
            self.byte = 0;
            self.bits = 0;
        }
    }

    pub fn into_bytes(mut self) -> Vec<u8> {
        self.flush();
        self.buf
    }
}

pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    byte: u32,
    bits: u32,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BitReader { data, pos: 0, byte: 0, bits: 0 }
    }

    fn refill(&mut self) {
        if self.bits == 0 && self.pos < self.data.len() {
            self.byte = self.data[self.pos] as u32;
            self.bits = 8;
            self.pos += 1;
        }
    }

    pub fn read_bit(&mut self) -> u32 {
        self.refill();
        if self.bits == 0 { return 0; }
        self.bits -= 1;
        (self.byte >> self.bits) & 1
    }

    pub fn read_bits(&mut self, n: u32) -> u32 {
        let mut val = 0u32;
        for _ in 0..n {
            val = (val << 1) | self.read_bit();
        }
        val
    }

    pub fn read_vlq(&mut self) -> u32 {
        let mut val = 0u32;
        let mut shift = 0u32;
        loop {
            let byte = self.read_bits(8);
            val |= (byte & 0x7f) << shift;
            shift += 7;
            if byte & 0x80 == 0 { return val; }
        }
    }

    pub fn byte_pos(&self) -> usize {
        self.pos - (if self.bits == 0 { 0 } else { 1 })
    }

    pub fn align(&mut self) {
        if self.bits > 0 {
            self.bits = 0;
            self.byte = 0;
        }
    }
}
