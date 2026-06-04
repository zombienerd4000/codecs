#ifndef BIT_H
#define BIT_H

#include <stdint.h>
#include <stdlib.h>

typedef struct {
    uint8_t *buf;
    size_t cap;
    size_t len;
    uint32_t byte;
    uint32_t bits;
} BitWriter;

void bw_init(BitWriter *w);
void bw_free(BitWriter *w);
void bw_write_bit(BitWriter *w, uint32_t b);
void bw_write_bits(BitWriter *w, uint32_t val, uint32_t n);
void bw_write_vlq(BitWriter *w, uint32_t v);
void bw_write_byte(BitWriter *w, uint8_t b);
void bw_flush(BitWriter *w);
uint8_t *bw_into_bytes(BitWriter *w, size_t *out_len);

typedef struct {
    const uint8_t *data;
    size_t data_len;
    size_t pos;
    uint32_t byte;
    uint32_t bits;
} BitReader;

void br_init(BitReader *r, const uint8_t *data, size_t len);
uint32_t br_read_bit(BitReader *r);
uint32_t br_read_bits(BitReader *r, uint32_t n);
uint32_t br_read_vlq(BitReader *r);
size_t br_byte_pos(BitReader *r);
void br_align(BitReader *r);
void br_advance_bytes(BitReader *r, size_t n);

#endif
