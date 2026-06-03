# codecs

a collection of compression codecs built while learning how compression works.

## codecs

| name | language | notes |
|------|----------|-------|
| **kom1** | rust | first attempt, basic lz |
| **tkc0** | rust | second iteration |
| **tkc1** | rust | added huffman coding |
| **tkc2** | rust | xor/add transforms |
| **tkc3** | rust | current version, deflate-style block structure |
| **python-demos** | python | early prototypes (paw_bpe, cmp) |

## building (rust codecs)

```
cd tkc3
cargo build --release
./target/release/tkc3 bench-corpus
```

## license

mit
