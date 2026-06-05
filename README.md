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
| **ctkc0** | c | 0.7.1 | c port with RLE huffman tables, XOR RowDelta, bitmap detection, vs gzip (20/30 files win) |

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

ctkc0 is a c99 compressor with no external dependencies, evolved from the
stkc0/tkc3 codecs. it uses a custom bitstream format (not deflate compatible)
with RLE-compressed Huffman tables, run-length encoded match tokens, and
prefilters (RowDelta, XOR RowDelta, Delta16) for specific data types.

currently 20/30 files beat gzip 1.14 -9 (git for windows) on the
calgary/canterbury corpus. total: ctkc0=2,193,679 gzip=2,257,101 (-63,422).

### compression ratios vs gzip 1.14 -9 (calgary/canterbury corpus)

| file | ctkc0 | gzip | diff |
|------|-------|------|------|
| alice29.txt | 53092 (35.8%) | 53441 (36.0%) | -349 |
| asyoulik.txt | 48372 (38.6%) | 48840 (39.0%) | -468 |
| bib | 34443 (31.0%) | 34908 (31.4%) | -465 |
| book1 | 308647 (40.1%) | 312289 (40.6%) | -3642 |
| book2 | 203000 (33.2%) | 206166 (33.8%) | -3166 |
| cp.html | 7963 (32.4%) | 7992 (32.5%) | -29 |
| fields.c | 3163 (28.4%) | 3147 (28.2%) | +16 |
| geo | 67853 (66.3%) | 68422 (66.8%) | -569 |
| grammar.lsp | 1244 (33.4%) | 1257 (33.8%) | -13 |
| kennedy.xls | 210110 (20.4%) | 209744 (20.4%) | +366 |
| lcet10.txt | 140600 (33.5%) | 142590 (34.0%) | -1990 |
| news | 141843 (37.6%) | 144408 (38.3%) | -2565 |
| obj1 | 10325 (48.0%) | 10328 (48.0%) | -3 |
| obj2 | 81361 (33.0%) | 81095 (32.9%) | +266 |
| pi.txt | 424777 (42.5%) | 470445 (47.0%) | -45668 |
| pic | 50617 (9.9%) | 52389 (10.2%) | -1772 |
| plrabn12.txt | 190872 (40.5%) | 193118 (41.0%) | -2246 |
| paper1 | 18529 (34.9%) | 18551 (34.9%) | -22 |
| paper2 | 29543 (35.9%) | 29675 (36.1%) | -132 |
| paper3 | 18016 (38.7%) | 18082 (38.9%) | -66 |
| paper4 | 5530 (41.6%) | 5542 (41.7%) | -12 |
| paper5 | 5019 (42.0%) | 5003 (41.9%) | +16 |
| paper6 | 13434 (35.3%) | 13221 (34.7%) | +213 |
| progc | 13500 (34.1%) | 13269 (33.5%) | +231 |
| progl | 16284 (22.7%) | 16172 (22.6%) | +112 |
| progp | 11262 (22.8%) | 11194 (22.7%) | +68 |
| ptt5 | 50617 (9.9%) | 52393 (10.2%) | -1776 |
| sum | 12839 (33.6%) | 12783 (33.4%) | +56 |
| trans | 19065 (20.3%) | 18870 (20.1%) | +195 |
| xargs.1 | 1759 (41.6%) | 1767 (41.8%) | -8 |

key improvements vs stkc0: RLE Huffman table encoding (saves ~100-150 bytes
per block on most files), XOR RowDelta prefilter for 1-bit bitmap data (PBM,
P4, 1-bit BMP), generic stride detection for zero/FF-heavy data, hash
function changed from xor-based to multiplicative golden ratio.

latest: two-pass encoding for non-text blocks (pass 0 counts stepwise
freqs, pass 1 uses actual huffman tree costs). net -1590B, 20/30 beat gzip.
obj1 now beats gzip (-3), kennedy -382, geo -342, pic/ptt5 -341, obj2 -140.

### building

```
cd ctkc0

# clang/gcc:
clang -O3 -std=c99 -Wall -Wextra -Wpedantic -lm -o ctkc0 src/main.c src/bit.c src/huff.c src/lz.c src/codec.c

# msvc (from vs dev shell):
cl /O2 /std:c11 /Fe:ctkc0.exe src/main.c src/bit.c src/huff.c src/lz.c src/codec.c

# test round-trip on all corpus files:
for %f in (..\stkc0\test_data\*) do ctkc0 t %f
```

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
