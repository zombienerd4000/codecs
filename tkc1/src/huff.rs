use std::cmp::Reverse;
use std::collections::BinaryHeap;

const MAX_HUFF_BITS: u8 = 15;
const MAX_SYMBOLS: usize = 257;

pub struct Huffman {
    pub code: [u32; MAX_SYMBOLS],
    pub len: [u8; MAX_SYMBOLS],
    pub first_code: [u32; 17],
    pub syms_at_len: [Vec<u16>; 17],
}

pub fn build(freqs: &[u32; MAX_SYMBOLS]) -> Huffman {
    #[derive(Clone, Eq, PartialEq)]
    struct Node {
        freq: u32,
        sym: Option<u16>,
        left: Option<Box<Node>>,
        right: Option<Box<Node>>,
    }
    impl Ord for Node {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.freq.cmp(&other.freq).then_with(|| self.sym.cmp(&other.sym))
        }
    }
    impl PartialOrd for Node {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    let mut heap = BinaryHeap::new();
    for (sym, &f) in freqs.iter().enumerate() {
        if f > 0 {
            heap.push(Reverse(Box::new(Node { freq: f, sym: Some(sym as u16), left: None, right: None })));
        }
    }

    if heap.is_empty() {
        let mut h = Huffman::new();
        h.len[0] = 1;
        return h;
    }
    if heap.len() == 1 {
        let mut h = Huffman::new();
        let sym = heap.pop().unwrap().0.sym.unwrap();
        h.len[sym as usize] = 1;
        h.build_tables();
        return h;
    }

    while heap.len() > 1 {
        let a = heap.pop().unwrap().0;
        let b = heap.pop().unwrap().0;
        let merged = Box::new(Node {
            freq: a.freq + b.freq,
            sym: None,
            left: Some(a),
            right: Some(b),
        });
        heap.push(Reverse(merged));
    }

    let root = heap.pop().unwrap().0;

    let mut lens = [0u8; MAX_SYMBOLS];
    fn walk(node: &Node, depth: u8, lens: &mut [u8; MAX_SYMBOLS]) {
        if let Some(sym) = node.sym {
            lens[sym as usize] = depth;
        } else {
            if let Some(ref l) = node.left { walk(l, depth + 1, lens); }
            if let Some(ref r) = node.right { walk(r, depth + 1, lens); }
        }
    }
    walk(&root, 0, &mut lens);

    limit_lengths(&mut lens, MAX_HUFF_BITS);

    let mut h = Huffman::new();
    h.len = lens;
    h.build_tables();
    h
}

fn limit_lengths(lens: &mut [u8; MAX_SYMBOLS], max_bits: u8) {
    let mut count = [0u32; 256];
    let mut max = 0u8;
    for &l in lens.iter() {
        if l > 0 { count[l as usize] += 1; if l > max { max = l; } }
    }
    if max <= max_bits { return; }

    for len in (max_bits as usize + 1..=max as usize).rev() {
        while count[len] > 0 {
            count[len] -= 1;
            count[max_bits as usize] += 1;
            for l in (0..=max_bits as usize).rev() {
                if count[l] > 0 {
                    count[l] -= 1;
                    count[l + 1] += 1;
                    break;
                }
            }
        }
    }

    let mut new_lens = [0u8; MAX_SYMBOLS];
    let mut syms_by_old_len: Vec<Vec<u16>> = vec![Vec::new(); 256];
    for (s, &l) in lens.iter().enumerate() {
        if l > 0 { syms_by_old_len[l as usize].push(s as u16); }
    }

    let mut available = Vec::new();
    for len in 1..=max as usize {
        for &sym in &syms_by_old_len[len] {
            available.push(sym);
        }
    }

    let mut idx = 0;
    for len in 1..=max_bits as usize {
        for _ in 0..count[len] {
            if idx < available.len() {
                new_lens[available[idx] as usize] = len as u8;
                idx += 1;
            }
        }
    }

    *lens = new_lens;
}

impl Huffman {
    pub fn new() -> Self {
        Huffman {
            code: [0u32; MAX_SYMBOLS],
            len: [0u8; MAX_SYMBOLS],
            first_code: [0u32; 17],
            syms_at_len: Default::default(),
        }
    }

    pub fn build_tables(&mut self) {
        let mut code = 0u32;
        let mut prev_len = 0u8;
        for len in 1..=MAX_HUFF_BITS {
            let mut syms: Vec<u16> = (0..MAX_SYMBOLS as u16)
                .filter(|&s| self.len[s as usize] == len)
                .collect();
            syms.sort();
            if syms.is_empty() { continue; }

            code <<= len - prev_len;
            self.first_code[len as usize] = code;
            self.syms_at_len[len as usize] = syms.clone();

            for &sym in &syms {
                self.code[sym as usize] = code;
                code += 1;
            }
            prev_len = len;
        }
    }

    pub fn encode(&self, sym: u16) -> (u32, u8) {
        (self.code[sym as usize], self.len[sym as usize])
    }

    pub fn decode<R: FnMut() -> u32>(&self, mut read_bit: R) -> u16 {
        let mut code = 0u32;
        for len in 1..=MAX_HUFF_BITS as usize {
            code = (code << 1) | read_bit();
            if code - self.first_code[len] < self.syms_at_len[len].len() as u32 {
                return self.syms_at_len[len][(code - self.first_code[len]) as usize];
            }
        }
        0
    }
}
