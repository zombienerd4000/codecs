#ifndef CODEC_H
#define CODEC_H

#include <stdint.h>
#include <stdlib.h>
#include "lz.h"

typedef enum {
    FILTER_NONE,
    FILTER_DELTA16,
    FILTER_ROW_DELTA,
    FILTER_ROW_DELTA_XOR,
} FilterType;

typedef struct {
    FilterType type;
    uint32_t stride;
} Filter;

typedef struct {
    HashType hash_type;
    int use_raw;
    int block_size_set;
    size_t block_size;
    Filter filter;
} FormatParams;

uint8_t *compress(const uint8_t *data, size_t len, size_t *out_len);
uint8_t *decompress(const uint8_t *compressed, size_t len, size_t *out_len);
FormatParams detect_format(const uint8_t *data, size_t len);
HashType scan_hash_type(const uint8_t *data, size_t n);

#endif
