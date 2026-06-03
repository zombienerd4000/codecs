use serde::Deserialize;
use std::collections::{BinaryHeap, HashMap};
use std::fs;
use std::time::Instant;
use tokenizers::models::bpe::{BpeTrainer, BPE};
use tokenizers::pre_tokenizers::whitespace::Whitespace;
use tokenizers::{Tokenizer, TokenizerBuilder};

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct DiscordExport {
    messages: Vec<Message>,
}
#[derive(Deserialize)]
struct Message {
    content: String,
}

// ---------------------------------------------------------------------------
// Preprocessing
// ---------------------------------------------------------------------------

fn preprocess(msg: &str) -> String {
    let msg = msg.to_lowercase();
    let mut r = String::with_capacity(msg.len());
    let bytes = msg.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 7 < bytes.len()
            && ((bytes[i] == b'h' && &bytes[i..i + 7] == b"http://")
                || (bytes[i] == b'h' && &bytes[i..i + 8] == b"https://"))
        {
            while i < bytes.len() && bytes[i] != b' ' { i += 1; }
            continue;
        }
        if bytes[i] == b'<' {
            while i < bytes.len() && bytes[i] != b'>' { i += 1; }
            if i < bytes.len() { i += 1; }
            continue;
        }
        if bytes[i] == b'@' && i + 8 < bytes.len() {
            if &bytes[i..i + 9] == b"@everyone" || &bytes[i..i + 5] == b"@here" {
                while i < bytes.len() && bytes[i] != b' ' { i += 1; }
                continue;
            }
        }
        let c = bytes[i] as char;
        if c.is_ascii_graphic() || c == ' ' { r.push(c); }
        i += 1;
    }
    r.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Huffman
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct HuffNode {
    prob: u64,
    val: Option<u32>,
    left: Option<Box<HuffNode>>,
    right: Option<Box<HuffNode>>,
}
impl PartialEq for HuffNode { fn eq(&self, other: &Self) -> bool { self.prob == other.prob } }
impl Eq for HuffNode {}
impl PartialOrd for HuffNode { fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) } }
impl Ord for HuffNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering { other.prob.cmp(&self.prob) }
}

fn build_huffman(freqs: &HashMap<u32, u64>) -> HashMap<u32, String> {
    if freqs.is_empty() { return HashMap::new(); }
    let mut heap: BinaryHeap<HuffNode> = BinaryHeap::new();
    for (&k, &v) in freqs { heap.push(HuffNode { prob: v, val: Some(k), left: None, right: None }); }
    if heap.len() == 1 {
        let node = heap.pop().unwrap();
        let mut codes = HashMap::new();
        codes.insert(node.val.unwrap(), "0".to_string());
        return codes;
    }
    while heap.len() > 1 {
        let a = heap.pop().unwrap();
        let b = heap.pop().unwrap();
        heap.push(HuffNode { prob: a.prob + b.prob, val: None, left: Some(Box::new(a)), right: Some(Box::new(b)) });
    }
    let root = heap.pop().unwrap();
    let mut codes = HashMap::new();
    fn walk(node: &HuffNode, prefix: &str, codes: &mut HashMap<u32, String>) {
        if let Some(v) = node.val { codes.insert(v, prefix.to_string()); }
        else {
            if let Some(ref l) = node.left { walk(l, &(prefix.to_string() + "0"), codes); }
            if let Some(ref r) = node.right { walk(r, &(prefix.to_string() + "1"), codes); }
        }
    }
    walk(&root, "", &mut codes);
    codes
}

// ---------------------------------------------------------------------------
// Word+char baseline
// ---------------------------------------------------------------------------

fn word_char_baseline(train: &[String], test: &[String]) -> (f64, f64, usize) {
    let mut word_counts: HashMap<String, u64> = HashMap::new();
    let mut char_counts: HashMap<char, u64> = HashMap::new();
    for msg in train {
        for c in msg.chars() { *char_counts.entry(c).or_insert(0) += 1; }
        for w in msg.split_whitespace() { *word_counts.entry(w.to_string()).or_insert(0) += 1; }
    }
    let mut token_freqs: HashMap<u32, u64> = HashMap::new();
    let mut char_to_id: HashMap<char, u32> = HashMap::new();
    for (&c, &f) in &char_counts { char_to_id.insert(c, c as u32); token_freqs.insert(c as u32, f); }
    let mut next_id: u32 = 256;
    let mut word_to_id: HashMap<String, u32> = HashMap::new();
    for (w, &f) in &word_counts { let id = next_id; next_id += 1; word_to_id.insert(format!(" {w}"), id); token_freqs.insert(id, f); }
    let codes = build_huffman(&token_freqs);
    let n_tokens = token_freqs.len();
    let encode = |msgs: &[String]| -> u64 {
        let mut bits: u64 = 0;
        for msg in msgs {
            let words: Vec<&str> = msg.split_whitespace().collect();
            for (i, w) in words.iter().enumerate() {
                if let Some(&tid) = word_to_id.get(&format!(" {w}")) {
                    bits += codes.get(&tid).map(|c| c.len() as u64).unwrap_or(8);
                } else {
                    if i > 0 { bits += codes.get(char_to_id.get(&' ').unwrap_or(&0)).map(|c| c.len() as u64).unwrap_or(8); }
                    for c in w.chars() {
                        bits += codes.get(char_to_id.get(&c).unwrap_or(&0)).map(|c| c.len() as u64).unwrap_or(8);
                    }
                }
            }
        }
        bits
    };
    let tb = encode(train); let tt = encode(test);
    let tc: u64 = train.iter().map(|m| m.len() as u64).sum();
    let ttc: u64 = test.iter().map(|m| m.len() as u64).sum();
    (tb as f64 / tc as f64, tt as f64 / ttc as f64, n_tokens)
}

// ---------------------------------------------------------------------------
// BPE via tokenizers
// ---------------------------------------------------------------------------

fn run_bpe(train: &[String], test: &[String], vocab_size: usize) -> (f64, f64, usize) {
    let mut model = BPE::default();
    let mut tokenizer = Tokenizer::new(model);
    tokenizer.with_pre_tokenizer(Some(Whitespace::default()));

    let mut trainer = BpeTrainer::builder()
        .vocab_size(vocab_size)
        .show_progress(false)
        .min_frequency(2)
        .build();

    tokenizer.train(&mut trainer, train.iter().map(|s| s.as_str()))
        .expect("bpe training failed");

    // Tokenize train
    let mut token_freqs: HashMap<u32, u64> = HashMap::new();
    let mut train_ids: Vec<Vec<u32>> = Vec::with_capacity(train.len());
    for msg in train {
        let enc = tokenizer.encode(msg.as_str(), false).unwrap();
        let ids = enc.get_ids().to_vec();
        for &tid in &ids { *token_freqs.entry(tid).or_insert(0) += 1; }
        train_ids.push(ids);
    }

    let codes = build_huffman(&token_freqs);
    let n_tokens = token_freqs.len();

    let train_bits: u64 = train_ids.iter().flat_map(|ids| ids.iter()).map(|&tid| codes.get(&tid).map(|c| c.len() as u64).unwrap_or(8)).sum();
    let train_chars: u64 = train.iter().map(|m| m.len() as u64).sum();
    let train_bpc = train_bits as f64 / train_chars as f64;

    let mut test_bits: u64 = 0;
    for msg in test {
        for tid in tokenizer.encode(msg.as_str(), false).unwrap().get_ids() {
            test_bits += codes.get(tid).map(|c| c.len() as u64).unwrap_or(8);
        }
    }
    let test_chars: u64 = test.iter().map(|m| m.len() as u64).sum();
    let test_bpc = test_bits as f64 / test_chars as f64;

    (train_bpc, test_bpc, n_tokens)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let path = r"C:\Users\komori\Desktop\New folder\rndm\Dev\web\DiscordChatExporter.win-x64\Direct Messages - mari [1490968020233748480].json";

    eprint!("loading... ");
    let t0 = Instant::now();
    let export: DiscordExport = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    eprintln!("{:.3}s ({} msgs)", t0.elapsed().as_secs_f64(), export.messages.len());

    eprint!("preprocessing... ");
    let t0 = Instant::now();
    let mut msgs: Vec<String> = export.messages.iter().map(|m| preprocess(&m.content)).filter(|m| !m.is_empty()).collect();
    eprintln!("{:.3}s", t0.elapsed().as_secs_f64());

    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    msgs.sort_by_key(|m| { let mut h = DefaultHasher::new(); m.hash(&mut h); h.finish() ^ 42u64 });

    let split = (msgs.len() as f64 * 0.9) as usize;
    let (train, test) = msgs.split_at(split);
    let tc: usize = train.iter().map(|m| m.len()).sum();
    let ttc: usize = test.iter().map(|m| m.len()).sum();
    eprintln!("train: {} msgs ({} chars)  test: {} msgs ({} chars)", train.len(), tc, test.len(), ttc);
    println!();

    // Baseline
    println!("=== kom1 word+char baseline ===");
    let t0 = Instant::now();
    let (bl_tr, bl_te, bl_n) = word_char_baseline(train, test);
    eprintln!("  tokens: {bl_n}  train: {bl_tr:.4} bpc  test: {bl_te:.4} bpc  [{:.3}s]", t0.elapsed().as_secs_f64());
    println!();

    // BPE
    println!("=== kom1 BPE ===");
    for &v in &[128, 256, 384, 512, 1024, 2048, 4096] {
        let t0 = Instant::now();
        let (tr, te, n) = run_bpe(train, test, v);
        let t = t0.elapsed();
        println!("  v={v:5}  tokens={n:5}  train={tr:.4} ({:+.3})  test={te:.4} ({:+.3})  [{:.3}s]",
            tr - bl_tr, te - bl_te, t.as_secs_f64());
    }
}
