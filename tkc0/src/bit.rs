pub struct BitWriter {
    buf: Vec<u8>,
    byte: u8,
    pos: u8,
}

impl BitWriter {
    pub fn new() -> Self {
        BitWriter { buf: Vec::new(), byte: 0, pos: 0 }
    }

    pub fn write_bit(&mut self, b: u8) {
        self.byte = (self.byte << 1) | (b & 1);
        self.pos += 1;
        if self.pos == 8 {
            self.buf.push(self.byte);
            self.byte = 0;
            self.pos = 0;
        }
    }

    pub fn write_bits(&mut self, value: u32, mut n: u32) {
        while n > 0 {
            n -= 1;
            self.write_bit(((value >> n) & 1) as u8);
        }
    }

    pub fn write_vlq(&mut self, mut value: u32) {
        loop {
            let chunk = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                self.write_bits(chunk as u32, 8);
                break;
            }
            self.write_bits((chunk | 0x80) as u32, 8);
        }
    }

    pub fn write_byte(&mut self, b: u8) {
        self.write_bits(b as u32, 8);
    }

    pub fn flush(&mut self) {
        if self.pos > 0 {
            self.byte <<= 8 - self.pos;
            self.buf.push(self.byte);
            self.byte = 0;
            self.pos = 0;
        }
    }

    pub fn into_bytes(mut self) -> Vec<u8> {
        self.flush();
        self.buf
    }
}

pub struct BitReader<'a> {
    data: &'a [u8],
    bp: usize,
    bitp: u8,
    byte: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            bp: 0,
            bitp: 0,
            byte: if data.is_empty() { 0 } else { data[0] },
        }
    }

    pub fn read_bit(&mut self) -> u8 {
        if self.bp >= self.data.len() {
            return 0;
        }
        let b = (self.byte >> 7) & 1;
        self.byte = (self.byte << 1) & 0xff;
        self.bitp += 1;
        if self.bitp == 8 {
            self.bp += 1;
            self.bitp = 0;
            if self.bp < self.data.len() {
                self.byte = self.data[self.bp];
            }
        }
        b
    }

    pub fn read_bits(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.read_bit() as u32;
        }
        v
    }

    pub fn read_vlq(&mut self) -> u32 {
        let mut v = 0u32;
        let mut shift = 0u32;
        loop {
            let b = self.read_bits(8);
            v |= (b & 0x7f) << shift;
            shift += 7;
            if b & 0x80 == 0 {
                break;
            }
        }
        v
    }

    pub fn byte_pos(&self) -> usize {
        self.bp
    }

    pub fn skip_bytes(&mut self, n: usize) {
        self.bp += n;
        self.bitp = 0;
        if self.bp < self.data.len() {
            self.byte = self.data[self.bp];
        }
    }
}
