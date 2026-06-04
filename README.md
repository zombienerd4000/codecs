# codecs

a collection of compression codecs built while learning how compression works.

## codecs

| name | language | version | notes |
|------|----------|---------|-------|
| **kom1** | rust | - | first attempt, basic lz |
| **tkc0** | rust | - | second iteration |
| **tkc1** | rust | - | added huffman coding |
| **tkc2** | rust | - | xor/add transforms |
| **tkc3** | rust | 0.2.0 | current version, custom deflate-like codec |

## tkc3

custom deflate-like compressor with a custom bitstream format (not deflate compatible).
uses FxHashMap + binary search for match finding with up to 256 candidates per hash key.

### current performance vs gzip -9 (calgary corpus)

| file | tkc3 | gzip -9 | diff |
|------|------|---------|------|
| alice29.txt | 53623 (36.1%) | 53502 (36.0%) | +121b |
| asyoulik.txt | 49060 (39.2%) | 48863 (39.0%) | +197b |
| bib | 35005 (31.5%) | 35063 (31.5%) | -58b |
| book1 | 313782 (40.8%) | 312609 (40.7%) | +1173b |
| book2 | 205244 (33.6%) | 206166 (33.8%) | -922b |
| cp.html | 8087 (32.9%) | 7965 (32.4%) | +122b |
| fields.c | 3282 (29.4%) | 3130 (28.1%) | +152b |
| geo | 69890 (68.3%) | 68335 (66.7%) | +1555b |
| grammar.lsp | 1355 (36.4%) | 1234 (33.2%) | +121b |
| kennedy.xls | 211984 (20.6%) | 209207 (20.3%) | +2777b |
| lcet10.txt | 141960 (33.9%) | 142657 (34.0%) | -697b |
| news | 143052 (37.9%) | 144548 (38.3%) | -1496b |
| obj1 | 10703 (49.8%) | 10318 (48.0%) | +385b |
| obj2 | 83978 (34.0%) | 81493 (33.0%) | +2485b |
| pi.txt | 425085 (42.5%) | 470465 (47.0%) | -45380b |
| pic | 55786 (10.9%) | 53717 (10.5%) | +2069b |
| paper1 | 18657 (35.1%) | 18541 (34.9%) | +116b |
| progc | 13557 (34.2%) | 13357 (33.7%) | +200b |
| progl | 16374 (22.9%) | 16180 (22.6%) | +194b |
| progp | 11396 (23.1%) | 11196 (22.7%) | +200b |
| ptt5 | 55786 (10.9%) | 53717 (10.5%) | +2069b |
| sum | 13357 (34.9%) | 12951 (33.9%) | +406b |
| trans | 19182 (20.5%) | 18945 (20.2%) | +237b |
| xargs.1 | 1870 (44.2%) | 1748 (41.4%) | +122b |

beats gzip on pi.txt by 45kb and is within 1-2% on most text files.
main gaps are on binary-like files (kennedy, obj2, geo, pic).

### encode speed

~2-5x slower than gzip on most files, 4x faster on pi.txt.
main bottleneck is the FxHashMap lookups in match finding (~30ms for
build_hash + ~20-30ms for lookups).

## building (rust codecs)

```
cd tkc3
cargo build --release
./target/release/tkc3 bench-corpus
```

## license

mit
