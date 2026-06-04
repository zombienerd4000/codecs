#include "bit.h"
#include <string.h>

void bw_init(BitWriter *w) {
    w->buf = NULL;
    w->cap = 0;
    w->len = 0;
    w->byte = 0;
    w->bits = 0;
}

void bw_free(BitWriter *w) {
    free(w->buf);
}

static void bw_grow(BitWriter *w) {
    if (w->len < w->cap) return;
    size_t new_cap = w->cap ? w->cap * 2 : 1024;
    uint8_t *new_buf = realloc(w->buf, new_cap);
    if (!new_buf) { abort(); }
    w->buf = new_buf;
    w->cap = new_cap;
}

void bw_write_bit(BitWriter *w, uint32_t b) {
    w->byte = (w->byte << 1) | (b & 1);
    w->bits++;
    if (w->bits == 8) {
        bw_grow(w);
        w->buf[w->len++] = (uint8_t)w->byte;
        w->byte = 0;
        w->bits = 0;
    }
}

void bw_write_bits(BitWriter *w, uint32_t val, uint32_t n) {
    for (uint32_t i = n; i > 0; i--) {
        bw_write_bit(w, (val >> (i - 1)) & 1);
    }
}

void bw_write_vlq(BitWriter *w, uint32_t v) {
    for (;;) {
        uint8_t byte = (uint8_t)(v & 0x7f);
        v >>= 7;
        if (v != 0) byte |= 0x80;
        bw_write_bits(w, byte, 8);
        if (v == 0) break;
    }
}

void bw_write_byte(BitWriter *w, uint8_t b) {
    bw_write_bits(w, b, 8);
}

void bw_flush(BitWriter *w) {
    if (w->bits > 0) {
        w->byte <<= 8 - w->bits;
        bw_grow(w);
        w->buf[w->len++] = (uint8_t)w->byte;
        w->byte = 0;
        w->bits = 0;
    }
}

uint8_t *bw_into_bytes(BitWriter *w, size_t *out_len) {
    bw_flush(w);
    *out_len = w->len;
    uint8_t *result = w->buf;
    w->buf = NULL;
    w->cap = 0;
    w->len = 0;
    return result;
}

void br_init(BitReader *r, const uint8_t *data, size_t len) {
    r->data = data;
    r->data_len = len;
    r->pos = 0;
    r->byte = 0;
    r->bits = 0;
}

static void br_refill(BitReader *r) {
    if (r->bits == 0 && r->pos < r->data_len) {
        r->byte = r->data[r->pos];
        r->bits = 8;
        r->pos++;
    }
}

uint32_t br_read_bit(BitReader *r) {
    br_refill(r);
    if (r->bits == 0) return 0;
    r->bits--;
    return (r->byte >> r->bits) & 1;
}

uint32_t br_read_bits(BitReader *r, uint32_t n) {
    uint32_t val = 0;
    for (uint32_t i = 0; i < n; i++) {
        val = (val << 1) | br_read_bit(r);
    }
    return val;
}

uint32_t br_read_vlq(BitReader *r) {
    uint32_t val = 0;
    uint32_t shift = 0;
    for (;;) {
        uint32_t byte = br_read_bits(r, 8);
        val |= (byte & 0x7f) << shift;
        shift += 7;
        if ((byte & 0x80) == 0) return val;
    }
}

size_t br_byte_pos(BitReader *r) {
    if (r->bits == 0) {
        return r->pos ? r->pos - 1 : 0;
    }
    return r->pos ? r->pos - 1 : 0;
}

void br_align(BitReader *r) {
    if (r->bits > 0) {
        r->bits = 0;
        r->byte = 0;
    }
}

void br_advance_bytes(BitReader *r, size_t n) {
    r->pos += n;
}
