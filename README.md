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
samples the first 4KB of input and picks a 3-byte hash for binary data
(>200 unique bytes or >5% non-printable chars) or a 4-byte hash for
text-like data. this helps compress binary files where 3-byte hash finds
more matches, while keeping the 4-byte hash for text where precision matters.

the 3-byte hash uses n-2 entries (vs n-3 for 4-byte), which is key: for
binary data, the extra hash slot creates more match candidates from every
position boundary, catching short matches that the 4-byte hash skips.

### current performance vs gzip -9 (calgary corpus)

compression ratio:

| file | stkc0 | gzip -9 | diff | vs tkc3 |
|------|-------|---------|------|---------|
| alice29.txt | 53624 (36.1%) | 53502 (36.0%) | +122b | +0 |
| asyoulik.txt | 49060 (39.2%) | 48863 (39.0%) | +197b | +0 |
| bib | 35005 (31.5%) | 35063 (31.5%) | -58b | +0 |
| book1 | 313787 (40.8%) | 312609 (40.7%) | +1178b | +0 |
| book2 | 205244 (33.6%) | 206166 (33.8%) | -922b | +0 |
| cp.html | 8087 (32.9%) | 7965 (32.4%) | +122b | +0 |
| fields.c | 3282 (29.4%) | 3130 (28.1%) | +152b | +0 |
| geo | 68504 (66.9%) | 68335 (66.7%) | +169b | -1385b |
| grammar.lsp | 1355 (36.4%) | 1234 (33.2%) | +121b | +0 |
| kennedy.xls | 212317 (20.6%) | 209207 (20.3%) | +3110b | +269b |
| lcet10.txt | 141961 (33.9%) | 142657 (34.0%) | -696b | +0 |
| news | 143054 (37.9%) | 144548 (38.3%) | -1494b | +0 |
| obj1 | 10389 (48.3%) | 10318 (48.0%) | +71b | -314b |
| obj2 | 81850 (33.2%) | 81493 (33.0%) | +357b | -2128b |
| pi.txt | 425085 (42.5%) | 470465 (47.0%) | -45380b | +0 |
| pic | 57501 (11.2%) | 53717 (10.5%) | +3784b | +1719b |
| plrabn12.txt | 193887 (41.2%) | 193287 (41.0%) | +600b | +0 |
| paper1 | 18657 (35.1%) | 18541 (34.9%) | +116b | +0 |
| progc | 13557 (34.2%) | 13357 (33.7%) | +200b | +0 |
| progl | 16371 (22.8%) | 16180 (22.6%) | +191b | +0 |
| progp | 11396 (23.1%) | 11196 (22.7%) | +200b | +0 |
| ptt5 | 57501 (11.2%) | 53717 (10.5%) | +3784b | +1719b |
| sum | 12903 (33.7%) | 12951 (33.9%) | -48b | -454b |
| trans | 19103 (20.4%) | 18945 (20.2%) | +158b | -81b |
| xargs.1 | 1870 (44.2%) | 1748 (41.4%) | +122b | +0 |

vs tkc3 (4-byte only): geo -1385b, obj1 -314b, obj2 -2128b, sum -454b,
trans -81b = -4362b improvement on binary files. regressions: pic +1719b,
ptt5 +1719b, kennedy +269b = +3707b total loss. net: -655b across the
corpus.

## building (rust codecs)

```
cd tkc3
cargo build --release
./target/release/tkc3 bench-corpus
```

## license

mit
