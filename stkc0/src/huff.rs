use std::cmp::Reverse;
use std::collections::BinaryHeap;

const MAX_HUFF_BITS: u8 = 15;

pub struct Huffman {
    pub code: Vec<u32>,
    pub len: Vec<u8>,
    pub first_code: Vec<u32>,
    pub syms_at_len: Vec<Vec<u16>>,
}

pub fn build(freqs: &[u32]) -> Huffman {
    let n_syms = freqs.len();
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
        let mut h = Huffman::new(n_syms);
        h.len[0] = 1;
        return h;
    }
    if heap.len() == 1 {
        let mut h = Huffman::new(n_syms);
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

    let mut lens = vec![0u8; n_syms];
    fn walk(node: &Node, depth: u8, lens: &mut [u8]) {
        if let Some(sym) = node.sym {
            lens[sym as usize] = depth;
        } else {
            if let Some(ref l) = node.left { walk(l, depth + 1, lens); }
            if let Some(ref r) = node.right { walk(r, depth + 1, lens); }
        }
    }
    walk(&root, 0, &mut lens);

    limit_lengths(&mut lens, MAX_HUFF_BITS);

    let mut h = Huffman::new(n_syms);
    h.len = lens;
    h.build_tables();
    h
}

fn limit_lengths(lens: &mut [u8], max_bits: u8) {
    let n = lens.len();
    let mut count = vec![0u32; 256];
    let mut max = 0u8;
    for &l in lens.iter() {
        if l > 0 { count[l as usize] += 1; if l > max { max = l; } }
    }
    if max <= max_bits { return; }

    for len in (max_bits as usize + 1..=max as usize).rev() {
        while count[len] > 0 {
            count[len] -= 1;
            count[max_bits as usize] += 1;
            for l in (0..max_bits as usize).rev() {
                if count[l] > 0 {
                    count[l] -= 1;
                    count[l + 1] += 1;
                    break;
                }
            }
        }
    }

    let mut new_lens = vec![0u8; n];
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

    lens.copy_from_slice(&new_lens);
}

impl Huffman {
    pub fn new(n_syms: usize) -> Self {
        Huffman {
            code: vec![0u32; n_syms],
            len: vec![0u8; n_syms],
            first_code: vec![0u32; 17],
            syms_at_len: vec![Vec::new(); 17],
        }
    }

    pub fn build_tables(&mut self) {
        let n = self.len.len();
        let mut code = 0u32;
        let mut prev_len = 0u8;
        for len in 1..=MAX_HUFF_BITS {
            let mut syms: Vec<u16> = (0..n as u16)
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
