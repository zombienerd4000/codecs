import json, re, heapq, random, sys, time
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

# ---------------------------------------------------------------------------
# Huffman
# ---------------------------------------------------------------------------

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

def avg_huffman_len(freqs):
    codes = build_huffman(freqs)
    total_freq = sum(freqs.values())
    if total_freq == 0:
        return 0
    return sum(freqs[k] * len(codes[k]) for k in freqs) / total_freq

# ---------------------------------------------------------------------------
# Baseline: word+char Huffman
# ---------------------------------------------------------------------------

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

# ---------------------------------------------------------------------------
# BPE via tokenizers library
# ---------------------------------------------------------------------------

def run_bpe(train_texts, test_texts, vocab_size):
    from tokenizers import Tokenizer, models, trainers, pre_tokenizers

    tokenizer = Tokenizer(models.BPE(unk_token="<unk>"))
    tokenizer.pre_tokenizer = pre_tokenizers.Whitespace()

    trainer = trainers.BpeTrainer(
        vocab_size=vocab_size,
        special_tokens=["<unk>"],
        min_frequency=2,
        show_progress=False,
    )

    tokenizer.train_from_iterator(train_texts, trainer)

    # get token -> id mapping
    token_vocab = tokenizer.get_vocab()

    # tokenize train, count frequencies
    train_token_ids = [tokenizer.encode(msg).ids for msg in train_texts]
    token_freqs = Counter()
    for ids in train_token_ids:
        for tid in ids:
            token_freqs[tid] += 1
    actual_tokens = len(token_freqs)

    # build huffman codes
    codes = build_huffman(dict(token_freqs))

    # bpc
    train_bits = sum(len(codes[tid]) if tid in codes else 8 for ids in train_token_ids for tid in ids)
    train_chars = sum(len(m) for m in train_texts)
    train_bpc = train_bits / train_chars

    test_bits = 0
    for msg in test_texts:
        ids = tokenizer.encode(msg).ids
        for tid in ids:
            if tid in codes:
                test_bits += len(codes[tid])
            else:
                test_bits += 8
    test_chars = sum(len(m) for m in test_texts)
    test_bpc = test_bits / test_chars

    return train_bpc, test_bpc, actual_tokens, tokenizer

# ---------------------------------------------------------------------------
# Frequency-based subword tokenizer (alternative: take top N substrings)
# This is simpler: find the most common N substrings across all messages
# and use them as tokens with word-level segmentation.
# ---------------------------------------------------------------------------

def run_freq_subword(train_texts, test_texts, vocab_size, min_len=2, max_len=6):
    """learn the top vocab_size most frequent substrings and use them as tokens"""
    substr_freqs = Counter()
    for msg in train_texts:
        for length in range(min_len, max_len + 1):
            for i in range(len(msg) - length + 1):
                substr = msg[i:i+length]
                substr_freqs[substr] += 1

    # take top substrings as tokens
    top_substrs = set(s for s, _ in substr_freqs.most_common(vocab_size))

    # build token frequencies on training set using greedy longest-match segmentation
    def segment(text, vocab):
        tokens = []
        i = 0
        while i < len(text):
            # try longest match first
            matched = False
            for length in range(min(max_len, len(text) - i), 0, -1):
                if text[i:i+length] in vocab:
                    tokens.append(text[i:i+length])
                    i += length
                    matched = True
                    break
            if not matched:
                tokens.append(text[i])
                i += 1
        return tokens

    train_tokens = [segment(m, top_substrs) for m in train_texts]
    token_freqs = Counter()
    for tokens in train_tokens:
        for t in tokens:
            token_freqs[t] += 1
    actual_tokens = len(token_freqs)

    codes = build_huffman(dict(token_freqs))

    train_bits = sum(len(codes[t]) if t in codes else 8 for tokens in train_tokens for t in tokens)
    train_chars = sum(len(m) for m in train_texts)
    train_bpc = train_bits / train_chars

    test_bits = 0
    for msg in test_texts:
        tokens = segment(msg, top_substrs)
        for t in tokens:
            if t in codes:
                test_bits += len(codes[t])
            else:
                test_bits += 8  # fallback
    test_chars = sum(len(m) for m in test_texts)
    test_bpc = test_bits / test_chars

    return train_bpc, test_bpc, actual_tokens

# ===================================================================

def main():
    msgs = load_messages(CHAT_PATH)
    msgs = [preprocess(m) for m in msgs if m.strip()]
    random.seed(42)
    random.shuffle(msgs)

    total_chars = sum(len(m) for m in msgs)
    print(f"messages: {len(msgs)}")
    print(f"total chars: {total_chars}")
    print(f"avg msg len: {total_chars/len(msgs):.1f}")
    print()

    split = int(len(msgs) * 0.9)
    train_msgs = msgs[:split]
    test_msgs = msgs[split:]
    print(f"train: {len(train_msgs)} msgs, {sum(len(m) for m in train_msgs)} chars")
    print(f"test:  {len(test_msgs)} msgs, {sum(len(m) for m in test_msgs)} chars")
    print()

    t0 = time.time()

    # Baseline
    print("=== word+char huffman baseline ===")
    bl_train, bl_test, bl_vocab = word_char_baseline(train_msgs, test_msgs)
    print(f"  vocab: {bl_vocab} tokens")
    print(f"  train: {bl_train:.4f} bpc")
    print(f"  test:  {bl_test:.4f} bpc")
    print(f"  time: {time.time()-t0:.1f}s")
    print()

    # BPE via tokenizers library
    print("=== BPE (tokenizers library, Whitespace pre-tokenizer) ===")
    for target in [128, 192, 256, 320, 384, 448, 512, 640, 768, 1024]:
        t1 = time.time()
        train_bpc, test_bpc, actual, tok = run_bpe(train_msgs, test_msgs, target)
        t = time.time() - t1
        print(f"  v={target:5d} (actual: {actual:4d})  train: {train_bpc:.4f}  test: {test_bpc:.4f}  [{t:.1f}s]")
    print()

    # Frequency-based subword tokenizer
    print("=== Frequency subword tokenizer (longest-match segmentation) ===")
    for target in [128, 256, 384, 512, 768, 1024]:
        t1 = time.time()
        train_bpc, test_bpc, actual = run_freq_subword(train_msgs, test_msgs, target)
        t = time.time() - t1
        print(f"  v={target:5d} (actual: {actual:4d})  train: {train_bpc:.4f}  test: {test_bpc:.4f}  [{t:.1f}s]")
    print()

if __name__ == '__main__':
    main()
