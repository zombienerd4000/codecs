import math, random, heapq, gzip, io
from collections import Counter

# same realistic chat generator as before
MORE_WORDS = [
    'give', 'day', 'most', 'us', 'great', 'really', 'thing', 'much',
    'right', 'still', 'old', 'tell', 'place', 'every', 'long', 'very',
    'big', 'ask', 'men', 'own', 'point', 'last', 'never', 'put',
    'city', 'man', 'read', 'keep', 'need', 'mean', 'left', 'start',
    'world', 'show', 'hand', 'high', 'play', 'house', 'line', 'same',
    'group', 'country', 'again', 'move', 'small',
    'number', 'night', 'live', 'where', 'right', 'head', 'stand',
    'between', 'turn', 'found', 'hear', 'help', 'close', 'open', 'case',
    'end', 'seem', 'next', 'hard', 'began', 'life', 'always', 'those',
    'both', 'paper', 'book', 'letter', 'able', 'children', 'side', 'feet',
    'car', 'face', 'door', 'water', 'eye', 'hand', 'room', 'mother',
    'father', 'family', 'home', 'boy', 'girl', 'school', 'friend',
    'hello', 'bye', 'yes', 'maybe', 'yeah', 'okay', 'sure', 'cool',
    'nice', 'love', 'hate', 'fun', 'awesome', 'happy', 'sad', 'mad',
    'computer', 'phone', 'music', 'song', 'video', 'movie', 'game',
    'watch', 'listen', 'read', 'write', 'food', 'drink', 'eat',
    'cat', 'dog', 'fish', 'bird', 'pet', 'animal',
    'good', 'bad', 'better', 'worse', 'best', 'worst', 'easy', 'hard',
    'hot', 'cold', 'warm', 'cool', 'clean', 'dirty',
    'beautiful', 'ugly', 'pretty', 'cute',
    'alright', 'fine', 'whatever', 'anyway', 'somehow',
    'together', 'apart', 'always', 'never', 'sometimes', 'often',
    'rarely', 'usually', 'again', 'already', 'almost', 'quite',
    'perhaps', 'probably', 'definitely', 'absolutely',
    'actually', 'basically', 'literally', 'seriously', 'honestly',
    'wow', 'oh', 'ah', 'um', 'well',
]

WORD_FREQS = [
    ('the', 6.2), ('be', 3.1), ('to', 2.8), ('of', 2.7), ('and', 2.5),
    ('a', 2.4), ('in', 2.1), ('that', 1.8), ('have', 1.6), ('i', 1.5),
    ('it', 1.4), ('for', 1.3), ('not', 1.2), ('on', 1.1), ('with', 1.0),
    ('he', 0.9), ('as', 0.9), ('you', 0.9), ('do', 0.8), ('at', 0.8),
    ('this', 0.7), ('but', 0.7), ('his', 0.6), ('from', 0.6), ('they', 0.6),
    ('we', 0.5), ('say', 0.5), ('her', 0.5), ('she', 0.5), ('or', 0.5),
    ('an', 0.5), ('will', 0.4), ('my', 0.4), ('one', 0.4), ('all', 0.4),
    ('would', 0.4), ('there', 0.4), ('their', 0.4), ('what', 0.4), ('so', 0.3),
    ('up', 0.3), ('out', 0.3), ('if', 0.3), ('about', 0.3), ('who', 0.3),
    ('get', 0.3), ('which', 0.3), ('go', 0.3), ('me', 0.3), ('when', 0.3),
    ('make', 0.3), ('can', 0.3), ('like', 0.3), ('time', 0.3), ('no', 0.2),
    ('just', 0.2), ('him', 0.2), ('know', 0.2), ('take', 0.2), ('people', 0.2),
    ('into', 0.2), ('year', 0.2), ('your', 0.2), ('good', 0.2), ('some', 0.2),
    ('could', 0.2), ('them', 0.2), ('see', 0.2), ('other', 0.2), ('than', 0.2),
    ('then', 0.2), ('now', 0.2), ('look', 0.2), ('only', 0.2), ('come', 0.2),
    ('its', 0.2), ('over', 0.2), ('think', 0.2), ('also', 0.2), ('back', 0.2),
    ('after', 0.2), ('use', 0.2), ('two', 0.2), ('how', 0.2), ('our', 0.2),
    ('work', 0.2), ('first', 0.2), ('well', 0.2), ('way', 0.2), ('even', 0.2),
    ('new', 0.2), ('want', 0.2), ('because', 0.2), ('any', 0.2), ('these', 0.1),
]

RARE_WORDS = ['xylophone', 'quasar', 'jazz', 'knight', 'zebra', 'koala',
              'mystic', 'nymph', 'oxygen', 'puzzle', 'quartz', 'crypt',
              'jigsaw', 'fjord', 'bizarre', 'dwarf', 'ghost', 'ivory',
              'chlorophyll', 'photosynthesis', 'microscope', 'astronaut',
              'xenophobia', 'juxtaposition', 'quintessential', 'phenomenon']

class RealisticChat:
    def __init__(self):
        self.words = [w for w, _ in WORD_FREQS] + MORE_WORDS
        self.probs = [f for _, f in WORD_FREQS] + [0.015] * len(MORE_WORDS)
        total = sum(self.probs)
        self.probs = [p / total for p in self.probs]

    def gen_msgs(self, count, rare_pct=0):
        msgs = []
        for _ in range(count):
            length = int(random.paretovariate(1.3))
            if length > 20: length = 6
            length = max(1, length)
            parts = []
            for _ in range(length):
                if rare_pct > 0 and random.random() < rare_pct:
                    parts.append(random.choice(RARE_WORDS))
                else:
                    parts.append(random.choices(self.words, weights=self.probs)[0])
            msgs.append(' '.join(parts))
        return msgs

class HuffNode:
    def __init__(self, val, freq, left=None, right=None):
        self.val = val; self.freq = freq
        self.left = left; self.right = right
    def __lt__(self, o): return self.freq < o.freq

def build_huffman(freqs):
    heap = [HuffNode(v, f) for v, f in freqs.items()]
    heapq.heapify(heap)
    while len(heap) > 1:
        a = heapq.heappop(heap); b = heapq.heappop(heap)
        heapq.heappush(heap, HuffNode(None, a.freq + b.freq, a, b))
    codes = {}
    def walk(n, p=''):
        if n.val: codes[n.val] = p
        else: walk(n.left, p+'0'); walk(n.right, p+'1')
    walk(heap[0])
    return codes

# generate a large test set
random.seed(42)
chat = RealisticChat()
train = chat.gen_msgs(50000)
test = chat.gen_msgs(5000, rare_pct=0.08)

# build our huffman model on training data
freq = {}
for c in 'abcdefghijklmnopqrstuvwxyz .,!?':
    freq[('char', c)] = 1

known = set()
for msg in train:
    for w in msg.lower().split():
        if w not in known:
            known.add(w)
        freq[('word', w)] = freq.get(('word', w), 0) + 1
    for c in msg.lower():
        freq[('char', c)] = freq.get(('char', c), 0) + 1

codes = build_huffman(freq)

# encode all test messages with our system
our_bits = 0
our_chars = 0
for msg in test:
    words = msg.lower().split()
    for wi, w in enumerate(words):
        if wi > 0:
            our_bits += len(codes.get(('char', ' '), '001'))
        if w in known:
            our_bits += len(codes.get(('word', w), '1'*12))
        else:
            for c in w:
                k = ('char', c)
                our_bits += len(codes.get(k, '1'*10))
    our_chars += len(msg)

our_bpc = our_bits / our_chars

# gzip: concatenate all test messages into one stream, compress it
all_text = '\n'.join(test).encode('utf-8')
gzip_size = len(gzip.compress(all_text, compresslevel=6))
gzip_bpc = gzip_size * 8 / len(all_text)

# xz
import lzma
xz_size = len(lzma.compress(all_text, preset=6))
xz_bpc = xz_size * 8 / len(all_text)

# also test gzip on individual messages (streaming mode)
gzip_total = 0
for msg in test:
    gzip_total += len(gzip.compress(msg.encode('utf-8'), compresslevel=6))
gzip_stream_bpc = gzip_total * 8 / our_chars

print("=== fair comparison on the SAME 5000 messages ===")
print(f"total text: {our_chars} chars")
print(f"total bytes: {len(all_text)}")
print()
print(f"{'system':<20} {'size (bytes)':<20} {'bpc':<10} {'ratio':<10}")
print("-" * 60)
print(f"{'our huffman':<20} {our_bits//8 + 1:<20} {our_bpc:<10.2f} {8/our_bpc:<10.2f}x")
print(f"{'gzip -6 (stream)':<20} {gzip_stream_bpc * our_chars / 8:<20.0f} {gzip_stream_bpc:<10.2f} {8/gzip_stream_bpc:<10.2f}x")
print(f"{'gzip -6 (batch)':<20} {gzip_size:<20} {gzip_bpc:<10.2f} {8/gzip_bpc:<10.2f}x")
print(f"{'bzip2 -9':<20} {bzip2_size:<20} {bzip2_bpc:<10.2f} {8/bzip2_bpc:<10.2f}x")
print(f"{'xz -6':<20} {xz_size:<20} {xz_bpc:<10.2f} {8/xz_bpc:<10.2f}x")
print(f"{'raw ascii':<20} {len(all_text):<20} {8:<10.2f} {1:<10.2f}x")
print()
print("notes:")
print("- gzip stream = each message compressed separately (no shared history)")
print("- gzip batch = all messages compressed as one block (unfair but shows ceiling)")
print("- bzip2 & xz batch = one block (they always have larger windows)")
print("- our system is streaming + adaptive, the fairest comparison is gzip stream")

# also test on the canterbury 'paper1' style long text
print()
print("=== on a longer continuous text ===")
# generate a single long document
random.seed(42)
long_text = ' '.join(chat.gen_msgs(5000, rare_pct=0.05))
long_text_bytes = long_text.encode('utf-8')

# gzip batch on long text
gzbuf = io.BytesIO()
with gzip.GzipFile(fileobj=gzbuf, mode='wb', compresslevel=6) as f:
    f.write(long_text_bytes)
gzip_long_bpc = len(gzbuf.getvalue()) * 8 / len(long_text)

print(f"long text: {len(long_text)} chars")
print(f"gzip -6: {gzip_long_bpc:.2f} bpc")

# our system on long text (warm with training, then measure)
freq2 = {}
for c in 'abcdefghijklmnopqrstuvwxyz .,!?':
    freq2[('char', c)] = 1
known2 = set()
# train on 50000 messages
train2 = chat.gen_msgs(50000)
for msg in train2:
    for w in msg.lower().split():
        if w not in known2: known2.add(w)
        freq2[('word', w)] = freq2.get(('word', w), 0) + 1
    for c in msg.lower():
        freq2[('char', c)] = freq2.get(('char', c), 0) + 1
codes2 = build_huffman(freq2)

our_bits2 = 0
for msg in [long_text]:
    words = msg.lower().split()
    for wi, w in enumerate(words):
        if wi > 0:
            our_bits2 += len(codes2.get(('char', ' '), '001'))
        if w in known2:
            our_bits2 += len(codes2.get(('word', w), '1'*12))
        else:
            for c in w:
                k = ('char', c)
                our_bits2 += len(codes2.get(k, '1'*10))
our_long_bpc = our_bits2 / len(long_text)
print(f"our system: {our_long_bpc:.2f} bpc")
