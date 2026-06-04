#ifndef HUFF_H
#define HUFF_H

#include <stdint.h>
#include <stdlib.h>

#define MAX_HUFF_BITS 15

typedef struct {
    uint32_t *code;
    uint8_t *len;
    size_t n_syms;
    uint32_t first_code[17];
    uint16_t **syms_at_len;
    int syms_at_len_count[17];
} Huffman;

void huff_init(Huffman *h, size_t n_syms);
void huff_free(Huffman *h);
void huff_build(Huffman *h, const uint32_t *freqs);
void huff_build_tables(Huffman *h);
void huff_encode(const Huffman *h, uint16_t sym, uint32_t *out_code, uint8_t *out_len);
uint16_t huff_decode(const Huffman *h, uint32_t (*read_bit)(void *), void *ctx);

#endif
