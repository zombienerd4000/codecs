# codecs

a collection of compression codecs built while learning how compression works.

## codecs

| name | language | version | notes |
|------|----------|---------|-------|
| **kom1** | rust | - | first attempt, basic lz |
| **tkc0** | rust | - | second iteration |
| **tkc1** | rust | - | added huffman coding |
| **tkc2** | rust | - | xor/add transforms |
| **tkc3** | rust | 0.2.0 | 4-byte flat array hash table |
| **stkc0** | rust | 0.2.0 | scan-based hash selection (3 or 4 byte) |
| **ctkc0** | c | 0.2.0 | c port of stkc0 (same format, bit-identical decompression) |

## tkc3

custom deflate-like compressor with a custom bitstream format (not deflate compatible).
uses a flat array hash table (counting-based bucket fill) with xor-mixed 16-bit hash
for match finding, up to 256 candidates per bucket.

### current performance vs gzip -9 (calgary corpus)

compression ratio:

| file | tkc3 | gzip -9 | diff |
|------|------|---------|------|
| alice29.txt | 53624 (36.1%) | 53502 (36.0%) | +122b |
| asyoulik.txt | 49060 (39.2%) | 48863 (39.0%) | +197b |
| bib | 35005 (31.5%) | 35063 (31.5%) | -58b |
| book1 | 313787 (40.8%) | 312609 (40.7%) | +1178b |
| book2 | 205244 (33.6%) | 206166 (33.8%) | -922b |
| cp.html | 8087 (32.9%) | 7965 (32.4%) | +122b |
| fields.c | 3282 (29.4%) | 3130 (28.1%) | +152b |
| geo | 69889 (68.3%) | 68335 (66.7%) | +1554b |
| grammar.lsp | 1355 (36.4%) | 1234 (33.2%) | +121b |
| kennedy.xls | 212048 (20.6%) | 209207 (20.3%) | +2841b |
| lcet10.txt | 141961 (33.9%) | 142657 (34.0%) | -696b |
| news | 143054 (37.9%) | 144548 (38.3%) | -1494b |
| obj1 | 10703 (49.8%) | 10318 (48.0%) | +385b |
| obj2 | 83978 (34.0%) | 81493 (33.0%) | +2485b |
| pi.txt | 425085 (42.5%) | 470465 (47.0%) | -45380b |
| pic | 55782 (10.9%) | 53717 (10.5%) | +2065b |
| plrabn12.txt | 193887 (41.2%) | 193287 (41.0%) | +600b |
| paper1 | 18657 (35.1%) | 18541 (34.9%) | +116b |
| progc | 13557 (34.2%) | 13357 (33.7%) | +200b |
| progl | 16371 (22.8%) | 16180 (22.6%) | +191b |
| progp | 11396 (23.1%) | 11196 (22.7%) | +200b |
| ptt5 | 55782 (10.9%) | 53717 (10.5%) | +2065b |
| sum | 13357 (34.9%) | 12951 (33.9%) | +406b |
| trans | 19184 (20.5%) | 18945 (20.2%) | +239b |
| xargs.1 | 1870 (44.2%) | 1748 (41.4%) | +122b |

beats gzip on pi.txt by 45kb and is within 1-2% on most text files.
main gaps are on binary-like files (kennedy, obj2, geo, pic).

encode speed (tkc3 vs gzip):

| file | tkc3 | gzip | ratio |
|------|------|------|-------|
| book1 | 80ms | 53ms | 1.5x slower |
| kennedy | 66ms | 136ms | 2.1x FASTER |
| geo | 10ms | 15ms | 1.5x FASTER |
| obj2 | 18ms | 14ms | 1.3x slower |
| pi.txt | 15ms | 63ms | 4.2x FASTER |
| pic | 23ms | 21ms | 1.1x slower |
| news | 29ms | 13ms | 2.2x slower |
| book2 | 51ms | 29ms | 1.8x slower |
| lcet10 | 39ms | 22ms | 1.8x slower |
| progl | 4ms | 3ms | 1.3x slower |
| trans | 4ms | 3ms | 1.3x slower |

flat array hash table replaced FxHashMap, speeding up both build_hash and
find_match by ~2x. encode is now competitive with gzip on most files and
faster on binary-heavy ones (kennedy, geo).

## stkc0

stkc0 is a copy of tkc3 with a scan-based hash strategy selection. it
samples 4x 1KB chunks at 0%, 25%, 50%, 75% of the file (or the whole file
if <= 4KB) and picks a 3-byte hash for binary data or 4-byte hash for
text-like data. also checks magic bytes (ELF, PE, ZIP, PNG, JPEG, PDF,
PGM, OLE2, etc.) for known format hints.

the 3-byte hash uses n-2 entries (vs n-3 for 4-byte), which creates more
match candidates from every position boundary, catching short matches in
binary data that the 4-byte hash misses. includes a special case for
uniform binary data (low unique bytes + high binary %) which uses HASH4
to avoid over-matching on images with repeated pixel values.

new features vs tkc3:
- RowDelta prefilter (vertical pixel differencing) for BMP images
- OLE2 magic detection (D0 CF 11 E0) with exe-like params (HASH3 + 256K blocks)
- 60% binary threshold for scan_hash_type fixes kennedy.xls regression
- 27+ magic signatures for raw passthrough

### current performance vs gzip -9 (calgary corpus)

compression ratio:

| file | stkc0 | gzip -9 | diff | vs tkc3 |
|------|-------|---------|------|---------|
| alice29.txt | 53625 (36.1%) | 53502 (36.0%) | +123b | +1 |
| asyoulik.txt | 49062 (39.2%) | 48863 (39.0%) | +199b | +2 |
| bib | 35007 (31.5%) | 35063 (31.5%) | -56b | +2 |
| book1 | 313790 (40.8%) | 312609 (40.7%) | +1181b | +3 |
| book2 | 205247 (33.6%) | 206166 (33.8%) | -919b | +3 |
| cp.html | 8088 (32.9%) | 7965 (32.4%) | +123b | +1 |
| fields.c | 3283 (29.4%) | 3130 (28.1%) | +153b | +1 |
| geo | 68506 (66.9%) | 68335 (66.7%) | +171b | -1383b |
| grammar.lsp | 1356 (36.4%) | 1234 (33.2%) | +122b | +1 |
| kennedy.xls | 212064 (20.6%) | 209207 (20.3%) | +2857b | +16 |
| lcet10.txt | 141963 (33.9%) | 142657 (34.0%) | -694b | +2 |
| news | 143056 (37.9%) | 144548 (38.3%) | -1492b | +2 |
| obj1 | 10390 (48.3%) | 10318 (48.0%) | +72b | -313b |
| obj2 | 81854 (33.2%) | 81493 (33.0%) | +361b | -2124b |
| pi.txt | 425100 (42.5%) | 470465 (47.0%) | -45365b | +15 |
| pic | 55784 (10.9%) | 53717 (10.5%) | +2067b | +2 |
| plrabn12.txt | 193889 (41.2%) | 193287 (41.0%) | +602b | +2 |
| paper1 | 18658 (35.1%) | 18541 (34.9%) | +117b | +1 |
| progc | 13558 (34.2%) | 13357 (33.7%) | +201b | +1 |
| progl | 16372 (22.9%) | 16180 (22.6%) | +192b | +1 |
| progp | 11397 (23.1%) | 11196 (22.7%) | +201b | +1 |
| ptt5 | 55784 (10.9%) | 53717 (10.5%) | +2067b | +2 |
| sum | 12904 (33.7%) | 12951 (33.9%) | -47b | -453b |
| trans | 19104 (20.4%) | 18945 (20.2%) | +159b | -80b |
| xargs.1 | 1871 (44.3%) | 1748 (41.4%) | +123b | +1 |

kennedy fix is the main win: 212064b vs 212317b (-253b). the small
+1-3b regressions on other files are from the 60% binary threshold
shifting some files from HASH3 to HASH4.

## ctkc0

ctkc0 is a c99 port of stkc0 with no external dependencies. it uses the
exact same bitstream format (MAGIC/MAGIC_RAW, filter bytes, VLQ, Huffman
tables) so compressed data is interchangeable between the two
implementations.

all 30 calgary/canterbury corpus files roundtrip correctly in both
directions and cross-decompression (rust-compressed opened by c, and
vice versa) passes every file. compression ratios match the rust version
within 1-2% on most files; the remaining differences are from floating
point rounding in entropy estimation and integer width edge cases.

## building

rust codecs:

```
cd stkc0
cargo build --release
./target/release/stkc0 bench-corpus
```

c codec (ctkc0):

```
cd ctkc0
clang -O3 -std=c99 -o ctkc0 src/main.c src/bit.c src/huff.c src/lz.c src/codec.c
./ctkc0 t ../test_data/calgary_bib
```

requires clang or gcc (tested with clang 18 on windows, should work on linux/macos too).

## license

mit
