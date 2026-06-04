#ifndef CODEC_H
#define CODEC_H

#include <stdint.h>
#include <stdlib.h>

uint8_t *compress(const uint8_t *data, size_t len, size_t *out_len);
uint8_t *decompress(const uint8_t *compressed, size_t len, size_t *out_len);

#endif
