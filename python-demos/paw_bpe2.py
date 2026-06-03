import json, re, heapq, random, time, math
from collections import Counter

CHAT_PATH = r"C:\Users\komori\Desktop\New folder\rndm\Dev\web\DiscordChatExporter.win-x64\Direct Messages - mari [1490968020233748480].json"

def load_messages(path):
    with open(path, 'r', encoding='utf-8') as f:
        data = json.load(f)
    return [m['content'] for m in data['messages']]

def preprocess(msg):
    msg = msg.lower()
    msg = re.sub(r'https?://\S+', '', msg)
    msg = re.sub(r'<a?:\w+:\d+>', '', msg)
    msg = re.sub(r'<@!?\d+>', '', msg)
    msg = re.sub(r'@everyone|@here', '', msg)
    msg = re.sub(r'[^\x20-\x7e]', '', msg)
    msg = re.sub(r'\s+', ' ', msg).strip()
    return msg

# Huffman
class Node:
    def __init__(self, prob, val=None, left=None, right=None):
        self.prob = prob
        self.val = val
        self.left = left
        self.right = right
    def __lt__(self, o): return self.prob < o.prob

def build_huffman(freqs):
    if not freqs:
        return {}
    heap = [Node(freqs[k], k) for k in freqs]
    heapq.heapify(heap)
    while len(heap) > 1:
        a = heapq.heappop(heap)
        b = heapq.heappop(heap)
        heapq.heappush(heap, Node(a.prob + b.prob, None, a, b))
    codes = {}
    def walk(n, prefix=''):
        if n.val is not None:
            codes[n.val] = prefix
        else:
            walk(n.left, prefix+'0')
            walk(n.right, prefix+'1')
    walk(heap[0])
    return codes

# Baseline
def word_char_baseline(train, test):
    word_freqs = Counter()
    char_freqs = Counter()
    for msg in train:
        for c in msg:
            char_freqs[c] += 1
        for w in msg.split():
            word_freqs[w] += 1
    token_freqs = {}
    for c, f in char_freqs.items():
        token_freqs[c] = f
    for w, f in word_freqs.items():
        token_freqs[f" {w}"] = f
    codes = build_huffman(token_freqs)
    train_bits = 0
    for msg in train:
        words = msg.split()
        for i, w in enumerate(words):
            tok = f" {w}"
            if tok in codes:
                train_bits += len(codes[tok])
            else:
                if i > 0:
                    train_bits += len(codes[' ']) if ' ' in codes else 8
                for c in w:
                    train_bits += len(codes[c]) if c in codes else 8
    test_bits = 0
    for msg in test:
        words = msg.split()
        for i, w in enumerate(words):
            tok = f" {w}"
            if tok in codes:
                test_bits += len(codes[tok])
            else:
                if i > 0:
                    test_bits += len(codes[' ']) if ' ' in codes else 8
                for c in w:
                    test_bits += len(codes[c]) if c in codes else 8
    train_chars = sum(len(m) for m in train)
    test_chars = sum(len(m) for m in test)
    return train_bits/train_chars, test_bits/test_chars, len(token_freqs)

# BPE wrapper
def run_bpe(train_texts, test_texts, vocab_size, pre_tokenizer='whitespace'):
    from tokenizers import Tokenizer, models, trainers, pre_tokenizers
    tokenizer = Tokenizer(models.BPE(unk_token="<unk>"))
    if pre_tokenizer == 'whitespace':
        tokenizer.pre_tokenizer = pre_tokenizers.Whitespace()
    elif pre_tokenizer == 'bert':
        tokenizer.pre_tokenizer = pre_tokenizers.BertPreTokenizer()
    elif pre_tokenizer == 'bytelevel':
        tokenizer.pre_tokenizer = pre_tokenizers.ByteLevel(add_prefix_space=False)
    elif pre_tokenizer == 'punctuation':
        tokenizer.pre_tokenizer = pre_tokenizers.Sequence([
            pre_tokenizers.Whitespace(),
            pre_tokenizers.Punctuation(),
        ])
    trainer = trainers.BpeTrainer(
        vocab_size=vocab_size,
        special_tokens=["<unk>"],
        min_frequency=2,
        show_progress=False,
    )
    tokenizer.train_from_iterator(train_texts, trainer)

    train_ids = [tokenizer.encode(m).ids for m in train_texts]
    token_freqs = Counter()
    for ids in train_ids:
        for tid in ids:
            token_freqs[tid] += 1
    codes = build_huffman(dict(token_freqs))

    train_bits = sum(len(codes[tid]) if tid in codes else 8 for ids in train_ids for tid in ids)
    train_chars = sum(len(m) for m in train_texts)

    test_bits = 0
    for msg in test_texts:
        ids = tokenizer.encode(msg).ids
        for tid in ids:
            if tid in codes:
                test_bits += len(codes[tid])
            else:
                test_bits += 8
    test_chars = sum(len(m) for m in test_texts)
    return train_bits/train_chars, test_bits/test_chars, len(token_freqs), tokenizer

def entropy_bpc(train_texts):
    """Entropy-based lower bound estimate"""
    freqs = Counter()
    total = 0
    for msg in train_texts:
        for c in msg:
            freqs[c] += 1
            total += 1
    H = 0
    for f in freqs.values():
        p = f / total
        H -= p * math.log2(p)
    return H

def main():

    msgs = load_messages(CHAT_PATH)
    msgs = [preprocess(m) for m in msgs if m.strip()]
    random.seed(42)
    random.shuffle(msgs)

    split = int(len(msgs) * 0.9)
    train_msgs = msgs[:split]
    test_msgs = msgs[split:]

    train_chars = sum(len(m) for m in train_msgs)
    test_chars = sum(len(m) for m in test_msgs)

    print(f"messages: {len(msgs)}  train: {len(train_msgs)}  test: {len(test_msgs)}")
    print(f"chars: {train_chars + test_chars}  train: {train_chars}  test: {test_chars}")
    print()

    # entropy lower bound (1st order char)
    H1 = entropy_bpc(train_msgs)
    print(f"1st-order char entropy: {H1:.3f} bpc (arithmetic lower bound)")
    print()

    # Baseline
    print("=== word+char huffman baseline ===")
    bl_tr, bl_te, bl_v = word_char_baseline(train_msgs, test_msgs)
    print(f"  tokens: {bl_v}  train: {bl_tr:.4f}  test: {bl_te:.4f}")
    print()

    # BPE with Whitespace
    print("=== BPE (Whitespace pre-tokenizer) ===")
    for v in [128, 192, 256, 320, 384, 448, 512, 640, 768, 1024, 1536, 2048, 3072, 4096]:
        t1 = time.time()
        tr, te, n, _ = run_bpe(train_msgs, test_msgs, v, 'whitespace')
        t = time.time() - t1
        delta_tr = tr - bl_tr
        delta_te = te - bl_te
        mark_tr = "WORSE" if delta_tr > 0.1 else ("ok" if delta_tr > 0.05 else "good")
        mark_te = "WORSE" if delta_te > 0.1 else ("ok" if delta_te > 0.05 else "good")
        print(f"  v={v:5d}  tokens={n:5d}  train={tr:.4f} ({delta_tr:+.3f})  test={te:.4f} ({delta_te:+.3f})  [{t:.1f}s]")

    print()

    # BPE with BertPreTokenizer (splits on whitespace+punctuation)
    print("=== BPE (BertPreTokenizer) ===")
    for v in [256, 512, 1024, 2048, 4096]:
        t1 = time.time()
        tr, te, n, _ = run_bpe(train_msgs, test_msgs, v, 'bert')
        t = time.time() - t1
        delta_tr = tr - bl_tr
        delta_te = te - bl_te
        print(f"  v={v:5d}  tokens={n:5d}  train={tr:.4f} ({delta_tr:+.3f})  test={te:.4f} ({delta_te:+.3f})  [{t:.1f}s]")

    print()

    # BPE with ByteLevel
    print("=== BPE (ByteLevel) ===")
    for v in [256, 512, 1024, 2048, 4096]:
        t1 = time.time()
        tr, te, n, _ = run_bpe(train_msgs, test_msgs, v, 'bytelevel')
        t = time.time() - t1
        delta_tr = tr - bl_tr
        delta_te = te - bl_te
        print(f"  v={v:5d}  tokens={n:5d}  train={tr:.4f} ({delta_tr:+.3f})  test={te:.4f} ({delta_te:+.3f})  [{t:.1f}s]")

    print()

    # show BPE examples
    print("=== Example tokenizations (BPE v=512, whitespace) ===")
    _, _, _, tok = run_bpe(train_msgs[:100], test_msgs[:10], 512, 'whitespace')
    examples = [
        "hey how are you doing today?",
        "im good thanks! what about you?",
        "lol thats so funny haha",
        "i dont know what to say about that",
        "love you so much!!!! <3",
    ]
    for msg in examples:
        enc = tok.encode(msg)
        print(f"  input: {msg}")
        print(f"  tokens: {enc.tokens}")
        print()

if __name__ == '__main__':
    main()
