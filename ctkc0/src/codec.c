#include "codec.h"
#include "bit.h"
#include "lz.h"
#include "huff.h"
#include <string.h>
#include <math.h>

#define MAGIC 0x434B5453
#define MAGIC_RAW 0x53544B43
#define LEN_CODES 29
#define MAIN_SYMS 285
#define DIST_CODES 32

#define BLOCK_SIZE_64K 65536
#define BLOCK_SIZE_256K 262144
#define MIN_LAST_BLOCK 32768

static const uint16_t len_base[LEN_CODES] = {
    3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258
};
static const uint8_t len_extra[LEN_CODES] = {
    0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0
};

static const uint32_t dist_base[32] = {
    1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577,32769,49152
};
static const uint8_t dist_extra[32] = {
    0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13,14,14
};

static FormatParams fmt_raw(void) {
    FormatParams fp;
    fp.hash_type = HASH_TYPE_HASH4;
    fp.use_raw = 1;
    fp.block_size_set = 0;
    fp.block_size = 0;
    fp.filter.type = FILTER_NONE;
    fp.filter.stride = 0;
    return fp;
}

static FormatParams fmt_audio(void) {
    FormatParams fp;
    fp.hash_type = HASH_TYPE_HASH3;
    fp.use_raw = 0;
    fp.block_size_set = 1;
    fp.block_size = 65536;
    fp.filter.type = FILTER_DELTA16;
    fp.filter.stride = 0;
    return fp;
}

static FormatParams fmt_exe(void) {
    FormatParams fp;
    fp.hash_type = HASH_TYPE_HASH3;
    fp.use_raw = 0;
    fp.block_size_set = 1;
    fp.block_size = 262144;
    fp.filter.type = FILTER_NONE;
    fp.filter.stride = 0;
    return fp;
}

static FormatParams fmt_default(HashType ht) {
    FormatParams fp;
    fp.hash_type = ht;
    fp.use_raw = 0;
    fp.block_size_set = 0;
    fp.block_size = 0;
    fp.filter.type = FILTER_NONE;
    fp.filter.stride = 0;
    return fp;
}

static int is_binary(uint8_t b) {
    return b == 0 || (b < 0x09) || b == 0x0B || b == 0x0C || (b > 0x0D && b < 0x20) || b == 0x7F || (b >= 0x80 && b <= 0x9F);
}

HashType scan_hash_type(const uint8_t *data, size_t n) {
    int seen[256];
    memset(seen, 0, sizeof(seen));
    size_t binary_chars = 0;
    size_t unique = 0;
    size_t total = 0;

    if (n <= 4096) {
        for (size_t i = 0; i < n; i++) {
            uint8_t b = data[i];
            if (!seen[b]) { seen[b] = 1; unique++; }
            if (is_binary(b)) binary_chars++;
        }
        total = n;
    } else {
        size_t positions[4] = {0, n / 4, n / 2, n * 3 / 4};
        for (int p = 0; p < 4; p++) {
            size_t start = positions[p];
            size_t end = (start + 1024) < n ? (start + 1024) : n;
            for (size_t i = start; i < end; i++) {
                uint8_t b = data[i];
                if (!seen[b]) { seen[b] = 1; unique++; }
                if (is_binary(b)) binary_chars++;
            }
            total += end - start;
        }
    }

    if (binary_chars > total * 9 / 10 && unique < 50) return HASH_TYPE_HASH4;
    if (binary_chars > total * 3 / 5) return HASH_TYPE_HASH4;
    if (unique > 100 || binary_chars > total / 20) return HASH_TYPE_HASH3;
    return HASH_TYPE_HASH4;
}

static FormatParams detect_magic(const uint8_t *data, size_t len) {
    FormatParams none_fp;
    none_fp.hash_type = HASH_TYPE_HASH4;
    none_fp.use_raw = -1;
    none_fp.block_size_set = 0;
    none_fp.block_size = 0;
    none_fp.filter.type = FILTER_NONE;
    none_fp.filter.stride = 0;

    if (len < 4) return none_fp;

    uint8_t a = data[0], b = data[1], c = data[2], d = data[3];

    if (a == 0x50 && b == 0x4B && c == 0x03 && d == 0x04) return fmt_raw();
    if (a == 0x1F && b == 0x8B) return fmt_raw();
    if (a == 0x42 && b == 0x5A) return fmt_raw();
    if (a == 0x28 && b == 0xB5 && c == 0x2F && d == 0xFD) return fmt_raw();
    if (a == 0x04 && b == 0x22 && c == 0x4D && d == 0x18) return fmt_raw();
    if (a == 0x52 && b == 0x61 && c == 0x72 && d == 0x21) return fmt_raw();
    if (a == 0x37 && b == 0x7A && c == 0xBC && d == 0xAF) return fmt_raw();
    if (a == 0xFD && b == 0x37 && c == 0x7A && d == 0x58) return fmt_raw();
    if (a == 0x89 && b == 0x50 && c == 0x4E && d == 0x47) return fmt_raw();
    if (a == 0xFF && b == 0xD8) return fmt_raw();
    if (a == 0x47 && b == 0x49 && c == 0x46) return fmt_raw();
    if (a == 0x49 && b == 0x44 && c == 0x33) return fmt_raw();
    if (a == 0xFF && (b & 0xF0) == 0xF0) return fmt_raw();
    if (a == 0x66 && b == 0x4C && c == 0x61 && d == 0x43) return fmt_raw();
    if (a == 0x4F && b == 0x67 && c == 0x67 && d == 0x53) return fmt_raw();
    if (a == 0x25 && b == 0x50 && c == 0x44 && d == 0x46) return fmt_raw();

    if (a == 0x52 && b == 0x49 && c == 0x46 && d == 0x46) return fmt_audio();

    if (a == 0x42 && b == 0x4D && len >= 30) {
        uint32_t w = (uint32_t)data[18] | ((uint32_t)data[19] << 8) | ((uint32_t)data[20] << 16) | ((uint32_t)data[21] << 24);
        int32_t h = (int32_t)((uint32_t)data[22] | ((uint32_t)data[23] << 8) | ((uint32_t)data[24] << 16) | ((uint32_t)data[25] << 24));
        uint16_t bpp = (uint16_t)data[28] | ((uint16_t)data[29] << 8);
        uint32_t row_size = ((w * bpp + 31) / 32) * 4;
        uint32_t total_px = row_size * (uint32_t)(h < 0 ? -h : h);
        FormatParams fp;
        fp.hash_type = HASH_TYPE_HASH4;
        fp.use_raw = 0;
        fp.block_size_set = 0;
        fp.block_size = 0;
        if (row_size > 0 && total_px > 0 && total_px <= len) {
            fp.filter.type = FILTER_ROW_DELTA;
            fp.filter.stride = row_size;
        } else {
            fp.filter.type = FILTER_NONE;
            fp.filter.stride = 0;
        }
        return fp;
    }
    if (a == 0x49 && b == 0x49 && c == 0x2A && d == 0x00) { FormatParams fp = fmt_default(HASH_TYPE_HASH4); return fp; }
    if (a == 0x4D && b == 0x4D && c == 0x00 && d == 0x2A) { FormatParams fp = fmt_default(HASH_TYPE_HASH4); return fp; }
    if (a == 0x50 && (b == 0x34 || b == 0x35 || b == 0x36) && len >= 20) {
        size_t pos = 2;
        uint32_t w = 0, h = 0;
        while (pos < len && (data[pos] == ' ' || data[pos] == '\t' || data[pos] == '\n' || data[pos] == '\r')) pos++;
        while (pos < len && data[pos] == '#') { while (pos < len && data[pos] != '\n') pos++; while (pos < len && (data[pos] == ' ' || data[pos] == '\t' || data[pos] == '\n' || data[pos] == '\r')) pos++; }
        while (pos < len && data[pos] >= '0' && data[pos] <= '9') { w = w * 10 + (data[pos] - '0'); pos++; }
        while (pos < len && (data[pos] == ' ' || data[pos] == '\t' || data[pos] == '\n' || data[pos] == '\r')) pos++;
        while (pos < len && data[pos] >= '0' && data[pos] <= '9') { h = h * 10 + (data[pos] - '0'); pos++; }
        while (pos < len && (data[pos] == ' ' || data[pos] == '\t' || data[pos] == '\n' || data[pos] == '\r')) pos++;
        uint32_t stride, pixel_data;
        if (b == 0x34) {
            stride = (w + 7) / 8;
            pixel_data = stride * h;
        } else {
            uint32_t maxval = 0;
            while (pos < len && (data[pos] == ' ' || data[pos] == '\t' || data[pos] == '\n' || data[pos] == '\r')) pos++;
            while (pos < len && data[pos] >= '0' && data[pos] <= '9') { maxval = maxval * 10 + (data[pos] - '0'); pos++; }
            if (data[pos] == '\n' || data[pos] == ' ') pos++;
            uint32_t bytes_per_pixel = (b == 0x36) ? 3 : 1;
            stride = w * bytes_per_pixel;
            pixel_data = stride * h;
        }
        FormatParams fp;
        fp.hash_type = HASH_TYPE_HASH4;
        fp.use_raw = 0;
        fp.block_size_set = 0;
        fp.block_size = 0;
        if (stride > 0 && h > 0 && pos + pixel_data <= len) {
            fp.filter.type = FILTER_ROW_DELTA;
            fp.filter.stride = stride;
        } else {
            fp.filter.type = FILTER_NONE;
            fp.filter.stride = 0;
        }
        return fp;
    }

    if (a == 0xD0 && b == 0xCF && c == 0x11 && d == 0xE0) return fmt_exe();
    if (a == 0x7F && b == 0x45 && c == 0x4C && d == 0x46) return fmt_exe();
    if (a == 0x4D && b == 0x5A) return fmt_exe();
    if (a == 0xFE && b == 0xED && (c == 0xFA || c == 0xFB) && (d == 0xCE || d == 0xCF)) return fmt_exe();
    if (a == 0xCA && b == 0xFE && c == 0xBA && d == 0xBE) return fmt_exe();
    if (a == 0xCF && b == 0xFA && c == 0xED && d == 0xFE) return fmt_exe();
    if (a == 0x00 && b == 0x61 && c == 0x73 && d == 0x6D) return fmt_exe();

    return none_fp;
}

static uint32_t find_best_stride(const uint8_t *data, size_t len) {
    if (len < 8) return 0;
    size_t sample = len < 65536 ? len : 65536;

    uint32_t raw_zeros = 0;
    for (size_t i = 4; i < sample; i++) {
        if (data[i] == 0) raw_zeros++;
    }

    uint32_t candidates[100];
    uint32_t nc = 0;
    uint32_t max_stride = (len < 65536) ? (uint32_t)len : 65536;
    uint32_t min_stride = 4;
    uint32_t sqrt_n = (uint32_t)sqrt((double)len);

    for (uint32_t i = min_stride; i <= sqrt_n && i <= max_stride && nc < 100; i++) {
        if (len % i == 0) candidates[nc++] = i;
    }
    for (uint32_t i = min_stride; i <= sqrt_n && i <= max_stride && nc < 100; i++) {
        if (len % i == 0) {
            uint32_t p = (uint32_t)(len / i);
            if (p != i && p >= min_stride && p <= max_stride) candidates[nc++] = p;
        }
    }

    if (nc == 0) return 0;

    uint32_t best_stride = 0;
    uint32_t best_zeros = raw_zeros;

    for (uint32_t ci = 0; ci < nc; ci++) {
        uint32_t s = candidates[ci];
        uint32_t zeros = 0;
        for (size_t i = s; i < sample; i++) {
            if (data[i] == data[i - s]) zeros++;
        }
        if (zeros > best_zeros) {
            best_zeros = zeros;
            best_stride = s;
        }
    }

    if (best_stride != 0 && best_zeros > raw_zeros) {
        return best_stride;
    }
    return 0;
}

FormatParams detect_format(const uint8_t *data, size_t len) {
    FormatParams mp = detect_magic(data, len);
    if (mp.use_raw == 1) return mp;
    if (mp.use_raw == 0) return mp;

    HashType ht = scan_hash_type(data, len);
    FormatParams fp = fmt_default(ht);

    uint32_t stride = find_best_stride(data, len);
    if (stride > 0) {
        size_t scan_end = len < 65536 ? len : 65536;
        uint32_t zero_or_ff = 0;
        for (size_t i = 0; i < scan_end; i++) {
            if (data[i] == 0x00 || data[i] == 0xFF) zero_or_ff++;
        }
        if (zero_or_ff > scan_end * 4 / 5) {
            fp.filter.type = FILTER_ROW_DELTA_XOR;
            fp.filter.stride = stride;
        }
    }

    return fp;
}

static void prefilter_block(uint8_t *data, size_t len, Filter filter) {
    if (filter.type == FILTER_NONE) return;

    if (filter.type == FILTER_DELTA16) {
        int16_t prev = 0;
        size_t n = len / 2;
        for (size_t i = 0; i < n; i++) {
            int16_t val = (int16_t)((uint16_t)data[i*2] | ((uint16_t)data[i*2+1] << 8));
            int16_t delta = val - prev;
            prev = val;
            data[i*2] = (uint8_t)(uint16_t)delta;
            data[i*2+1] = (uint8_t)((uint16_t)delta >> 8);
        }
        return;
    }

    if (filter.type == FILTER_ROW_DELTA) {
        size_t s = filter.stride;
        if (s > 0 && s < len) {
            for (size_t i = s; i < len; i++) {
                data[i] = data[i] - data[i - s];
            }
        }
        return;
    }
}

static uint16_t length_to_code(uint32_t ln, uint32_t *extra) {
    if (ln == 3) { *extra = 0; return 256; }
    if (ln == 4) { *extra = 0; return 257; }
    if (ln == 5) { *extra = 0; return 258; }
    if (ln == 6) { *extra = 0; return 259; }
    if (ln == 7) { *extra = 0; return 260; }
    if (ln == 8) { *extra = 0; return 261; }
    if (ln == 9) { *extra = 0; return 262; }
    if (ln == 10) { *extra = 0; return 263; }
    if (ln <= 12) { *extra = ln - 11; return 264; }
    if (ln <= 14) { *extra = ln - 13; return 265; }
    if (ln <= 16) { *extra = ln - 15; return 266; }
    if (ln <= 18) { *extra = ln - 17; return 267; }
    if (ln <= 22) { *extra = ln - 19; return 268; }
    if (ln <= 26) { *extra = ln - 23; return 269; }
    if (ln <= 30) { *extra = ln - 27; return 270; }
    if (ln <= 34) { *extra = ln - 31; return 271; }
    if (ln <= 42) { *extra = ln - 35; return 272; }
    if (ln <= 50) { *extra = ln - 43; return 273; }
    if (ln <= 58) { *extra = ln - 51; return 274; }
    if (ln <= 66) { *extra = ln - 59; return 275; }
    if (ln <= 82) { *extra = ln - 67; return 276; }
    if (ln <= 98) { *extra = ln - 83; return 277; }
    if (ln <= 114) { *extra = ln - 99; return 278; }
    if (ln <= 130) { *extra = ln - 115; return 279; }
    if (ln <= 162) { *extra = ln - 131; return 280; }
    if (ln <= 194) { *extra = ln - 163; return 281; }
    if (ln <= 226) { *extra = ln - 195; return 282; }
    if (ln <= 257) { *extra = ln - 227; return 283; }
    *extra = 258; return 284;
}

static uint16_t match_sym(uint32_t ln) {
    uint32_t extra;
    return length_to_code(ln, &extra);
}

static int sym_to_match(uint16_t sym) {
    return (int)(sym - 256);
}

static uint16_t distance_to_code(uint32_t dist, uint32_t *extra, int *overflow) {
    if (dist == 1) { *extra = 0; *overflow = 0; return 0; }
    if (dist == 2) { *extra = 0; *overflow = 0; return 1; }
    if (dist == 3) { *extra = 0; *overflow = 0; return 2; }
    if (dist == 4) { *extra = 0; *overflow = 0; return 3; }
    if (dist <= 6) { *extra = dist - 5; *overflow = 0; return 4; }
    if (dist <= 8) { *extra = dist - 7; *overflow = 0; return 5; }
    if (dist <= 12) { *extra = dist - 9; *overflow = 0; return 6; }
    if (dist <= 16) { *extra = dist - 13; *overflow = 0; return 7; }
    if (dist <= 24) { *extra = dist - 17; *overflow = 0; return 8; }
    if (dist <= 32) { *extra = dist - 25; *overflow = 0; return 9; }
    if (dist <= 48) { *extra = dist - 33; *overflow = 0; return 10; }
    if (dist <= 64) { *extra = dist - 49; *overflow = 0; return 11; }
    if (dist <= 96) { *extra = dist - 65; *overflow = 0; return 12; }
    if (dist <= 128) { *extra = dist - 97; *overflow = 0; return 13; }
    if (dist <= 192) { *extra = dist - 129; *overflow = 0; return 14; }
    if (dist <= 256) { *extra = dist - 193; *overflow = 0; return 15; }
    if (dist <= 384) { *extra = dist - 257; *overflow = 0; return 16; }
    if (dist <= 512) { *extra = dist - 385; *overflow = 0; return 17; }
    if (dist <= 768) { *extra = dist - 513; *overflow = 0; return 18; }
    if (dist <= 1024) { *extra = dist - 769; *overflow = 0; return 19; }
    if (dist <= 1536) { *extra = dist - 1025; *overflow = 0; return 20; }
    if (dist <= 2048) { *extra = dist - 1537; *overflow = 0; return 21; }
    if (dist <= 3072) { *extra = dist - 2049; *overflow = 0; return 22; }
    if (dist <= 4096) { *extra = dist - 3073; *overflow = 0; return 23; }
    if (dist <= 6144) { *extra = dist - 4097; *overflow = 0; return 24; }
    if (dist <= 8192) { *extra = dist - 6145; *overflow = 0; return 25; }
    if (dist <= 12288) { *extra = dist - 8193; *overflow = 0; return 26; }
    if (dist <= 16384) { *extra = dist - 12289; *overflow = 0; return 27; }
    if (dist <= 24576) { *extra = dist - 16385; *overflow = 0; return 28; }
    if (dist <= 32768) { *extra = dist - 24577; *overflow = 0; return 29; }
    if (dist <= 49152) { *extra = dist - 32769; *overflow = 0; return 30; }
    *extra = dist; *overflow = 1; return 31;
}

static int lazy_skip(const uint8_t *data, size_t len, size_t pos, uint32_t ln, uint32_t off, const HashTables *ht, int64_t lit_cost) {
    if (ln > 5) return 0;
    if (pos + 1 + 5 > len) return 0;
    Token next;
    if (!ht_find_match(ht, data, len, pos + 1, lit_cost, &next)) return 0;
    if (!next.is_match) return 0;
    int64_t cur_sav = (int64_t)ln * 8 - match_cost(off, ln);
    int64_t nxt_sav = (int64_t)(next.ln + 1) * 8 - (6 + match_cost(next.off, next.ln));
    return nxt_sav > cur_sav + 8;
}

static int lazy_skip_cost(const uint8_t *data, size_t len, size_t pos, uint32_t ln, uint32_t off, const HashTables *ht, int64_t lit_cost) {
    if (ln > 5) return 0;
    if (pos + 1 + 5 > len) return 0;
    Token next;
    if (!ht_find_match(ht, data, len, pos + 1, lit_cost, &next)) return 0;
    if (!next.is_match) return 0;
    int64_t skip_cost = lit_cost + match_cost(next.off, next.ln);
    int64_t take_cost = match_cost(off, ln);
    return skip_cost < take_cost;
}

static size_t vlq_byte_size(uint32_t v) {
    size_t n = 1;
    while (v >= 128) { v >>= 7; n++; }
    return n;
}

static void write_huff_table(BitWriter *w, const uint8_t *lens, size_t n) {
    size_t runs = 0;
    size_t rle_run_bytes = 0;
    for (size_t i = 0; i < n; ) {
        runs++;
        size_t j = i + 1;
        while (j < n && lens[j] == lens[i]) j++;
        rle_run_bytes += vlq_byte_size((uint32_t)(j - i));
        i = j;
    }
    size_t rle_bytes = vlq_byte_size((uint32_t)runs) + rle_run_bytes + 1;

    if (rle_bytes < (n + 1) / 2) {
        bw_write_bit(w, 0);
        bw_write_vlq(w, (uint32_t)runs);
        for (size_t i = 0; i < n; ) {
            size_t j = i + 1;
            while (j < n && lens[j] == lens[i]) j++;
            bw_write_vlq(w, (uint32_t)(j - i));
            bw_write_bits(w, lens[i], 4);
            i = j;
        }
    } else {
        bw_write_bit(w, 1);
        for (size_t i = 0; i < n / 2; i++) {
            uint8_t lo = lens[i * 2];
            uint8_t hi = lens[i * 2 + 1];
            bw_write_byte(w, (uint8_t)((hi << 4) | lo));
        }
        if (n % 2 == 1) {
            bw_write_byte(w, (uint8_t)(lens[n - 1] << 4));
        }
    }
}

static void read_huff_table(BitReader *r, Huffman *huff, size_t n) {
    uint8_t *lens = calloc(n, 1);
    if (br_read_bit(r) == 1) {
        for (size_t i = 0; i < n / 2; i++) {
            uint8_t byte = (uint8_t)br_read_bits(r, 8);
            uint8_t lo = byte & 0x0f;
            uint8_t hi = byte >> 4;
            lens[i * 2] = lo;
            lens[i * 2 + 1] = hi;
        }
        if (n % 2 == 1) {
            uint8_t last = (uint8_t)br_read_bits(r, 8);
            lens[n - 1] = last >> 4;
        }
    } else {
        uint32_t runs = br_read_vlq(r);
        size_t pos = 0;
        for (uint32_t ri = 0; ri < runs && pos < n; ri++) {
            uint32_t run_len = br_read_vlq(r);
            uint8_t len_val = (uint8_t)br_read_bits(r, 4);
            for (uint32_t j = 0; j < run_len && pos < n; j++) {
                lens[pos++] = len_val;
            }
        }
    }

    huff_init(huff, n);
    free(huff->len);
    huff->len = lens;

    huff_build_tables(huff);
}

uint8_t *compress(const uint8_t *data, size_t len, size_t *out_len) {
    if (len == 0) { *out_len = 0; return NULL; }

    FormatParams fmt = detect_format(data, len);

    uint8_t *filtered = NULL;
    const uint8_t *work = data;

    if (fmt.filter.type == FILTER_ROW_DELTA) {
        filtered = malloc(len);
        memcpy(filtered, data, len);
        size_t s = fmt.filter.stride;
        if (s > 0 && s < len) {
            for (size_t i = s; i < len; i++) {
                filtered[i] = data[i] - data[i - s];
            }
        }
        work = filtered;
    } else if (fmt.filter.type == FILTER_ROW_DELTA_XOR) {
        filtered = malloc(len);
        memcpy(filtered, data, len);
        size_t s = fmt.filter.stride;
        if (s > 0 && s < len) {
            for (size_t i = s; i < len; i++) {
                filtered[i] = data[i] ^ data[i - s];
            }
        }
        work = filtered;
    } else if (fmt.filter.type == FILTER_DELTA16) {
        filtered = malloc(len);
        memcpy(filtered, data, len);
        prefilter_block(filtered, len, fmt.filter);
        work = filtered;
    }

    if (fmt.use_raw) {
        BitWriter w;
        bw_init(&w);
        bw_write_bits(&w, MAGIC_RAW, 32);
        bw_write_vlq(&w, (uint32_t)len);
        size_t hdr_len;
        uint8_t *hdr = bw_into_bytes(&w, &hdr_len);
        *out_len = hdr_len + len;
        uint8_t *result = malloc(*out_len);
        memcpy(result, hdr, hdr_len);
        memcpy(result + hdr_len, work, len);
        free(hdr);
        free(filtered);
        return result;
    }

    uint32_t freq[256];
    memset(freq, 0, sizeof(freq));
    int unique_count = 0;
    size_t scan_end = len < 65536 ? len : 65536;
    for (size_t i = 0; i < scan_end; i++) {
        if (freq[work[i]] == 0) unique_count++;
        freq[work[i]]++;
    }

    size_t block_size;
    if (fmt.block_size_set) {
        block_size = fmt.block_size;
    } else {
        if (unique_count <= 10) {
            block_size = BLOCK_SIZE_64K;
        } else {
            uint32_t max_freq = 0;
            for (int i = 0; i < 256; i++) if (freq[i] > max_freq) max_freq = freq[i];
            if (max_freq > scan_end / 2) {
                block_size = BLOCK_SIZE_256K;
            } else if (len > 131072) {
                uint32_t freq2[256];
                memset(freq2, 0, sizeof(freq2));
                size_t scan2_end = 131072 < len ? 131072 : len;
                for (size_t i = 65536; i < scan2_end; i++) {
                    freq2[work[i]]++;
                }
                double l1 = 0.0;
                for (int i = 0; i < 256; i++) {
                    l1 += fabs((double)freq[i] / 65536.0 - (double)freq2[i] / 65536.0);
                }
                block_size = (l1 < 0.15) ? BLOCK_SIZE_256K : BLOCK_SIZE_64K;
            } else {
                block_size = BLOCK_SIZE_64K;
            }
        }
    }

    uint8_t *output = NULL;
    size_t output_len = 0;
    size_t output_cap = 0;

    HashType ht = fmt.hash_type;
    size_t block_start = 0;

    while (block_start < len) {
        size_t block_end = block_start + block_size;
        if (block_end > len) block_end = len;
        size_t remaining_after = len - block_end;
        if (remaining_after > 0 && remaining_after < MIN_LAST_BLOCK) {
            block_end = len;
        }

        size_t win_start = (block_start > WINDOW) ? block_start - WINDOW : 0;
        size_t ext_len = block_end - win_start;

        uint32_t block_freq[256];
        memset(block_freq, 0, sizeof(block_freq));
        int block_unique = 0;
        for (size_t i = block_start; i < block_end; i++) {
            if (block_freq[work[i]] == 0) block_unique++;
            block_freq[work[i]]++;
        }
        size_t block_len = block_end - block_start;
        uint32_t max_block_freq = 0;
        for (int i = 0; i < 256; i++) if (block_freq[i] > max_block_freq) max_block_freq = block_freq[i];

        int low_entropy = (block_unique <= 32) || (max_block_freq * 2 > (uint32_t)block_len);
        int64_t lit_cost = 8;
        if (low_entropy) {
            double total = (double)block_len;
            double entropy = 0.0;
            for (int i = 0; i < 256; i++) {
                if (block_freq[i] > 0) {
                    double p = (double)block_freq[i] / total;
                    entropy -= p * log2(p);
                }
            }
            lit_cost = (int64_t)(entropy + 0.5);
            if (lit_cost < 2) lit_cost = 2;
            if (lit_cost > 8) lit_cost = 8;
        }

        TokenBuf tokens;
        tb_init(&tokens);
        uint32_t *main_freq = calloc(MAIN_SYMS, sizeof(uint32_t));
        uint32_t *dist_freq = calloc(DIST_CODES, sizeof(uint32_t));

        const uint8_t *ext_data = work + win_start;
        size_t pos = block_start - win_start;

        if (block_unique <= 10) {
            while (pos < ext_len) {
                uint8_t b = ext_data[pos];
                tb_push(&tokens, 0, b, 0, 0);
                main_freq[b]++;
                pos++;
            }
        } else {
            HashTables htable;
            ht_build(&htable, ext_data, ext_len, ht);
            size_t n = ext_len;

            while (pos < n) {
                Token tok;
                if (ht_find_match(&htable, ext_data, n, pos, lit_cost, &tok)) {
                    uint32_t ln = tok.ln;
                    uint32_t off = tok.off;
                    int do_lazy;
                    if (low_entropy) {
                        do_lazy = lazy_skip_cost(ext_data, n, pos, ln, off, &htable, lit_cost);
                    } else {
                        do_lazy = lazy_skip(ext_data, n, pos, ln, off, &htable, lit_cost);
                    }
                    if (do_lazy) {
                        tb_push(&tokens, 0, ext_data[pos], 0, 0);
                        main_freq[ext_data[pos]]++;
                        pos++;
                        continue;
                    }
                    tb_push(&tokens, 1, 0, off, ln);
                    uint16_t sym = match_sym(ln);
                    main_freq[sym]++;
                    uint32_t dummy_extra;
                    int dummy_overflow;
                    uint16_t d_code = distance_to_code(off, &dummy_extra, &dummy_overflow);
                    dist_freq[d_code]++;
                    pos += ln;
                } else {
                    tb_push(&tokens, 0, ext_data[pos], 0, 0);
                    main_freq[ext_data[pos]]++;
                    pos++;
                }
            }
            ht_free(&htable);
        }

        Huffman main_huff, dist_huff;
        huff_init(&main_huff, MAIN_SYMS);
        huff_init(&dist_huff, DIST_CODES);
        huff_build(&main_huff, main_freq);
        huff_build(&dist_huff, dist_freq);

        BitWriter w;
        bw_init(&w);
        for (size_t ti = 0; ti < tokens.count; ti++) {
            uint32_t tag = tokens.data[ti * 2];
            uint32_t val2 = tokens.data[ti * 2 + 1];
            if (!(tag & 0x80000000U)) {
                uint8_t b = (uint8_t)tag;
                uint32_t code; uint8_t clen;
                huff_encode(&main_huff, b, &code, &clen);
                bw_write_bits(&w, code, clen);
            } else {
                uint32_t off = tag & 0x7FFFFFFFU;
                uint32_t ln = val2;
                uint16_t sym = match_sym(ln);
                uint32_t code; uint8_t clen;
                huff_encode(&main_huff, sym, &code, &clen);
                bw_write_bits(&w, code, clen);

                uint32_t extra;
                uint16_t lc = length_to_code(ln, &extra);
                uint32_t len_extra_bits = len_extra[lc - 256];
                if (lc == 284) {
                    if (ln > 258) {
                        bw_write_bit(&w, 1);
                        bw_write_vlq(&w, ln - 258);
                    } else {
                        bw_write_bit(&w, 0);
                    }
                } else if (len_extra_bits > 0) {
                    bw_write_bits(&w, extra, len_extra_bits);
                }

                uint32_t d_extra;
                int d_overflow;
                uint16_t d_code = distance_to_code(off, &d_extra, &d_overflow);
                huff_encode(&dist_huff, d_code, &code, &clen);
                bw_write_bits(&w, code, clen);
                if (d_overflow) {
                    bw_write_vlq(&w, d_extra);
                } else {
                    if (dist_extra[d_code] > 0) {
                        bw_write_bits(&w, d_extra, dist_extra[d_code]);
                    }
                }
            }
        }

        size_t bitstream_len;
        uint8_t *bitstream = bw_into_bytes(&w, &bitstream_len);

        BitWriter h;
        bw_init(&h);
        bw_write_bits(&h, MAGIC, 32);
        if (fmt.filter.type == FILTER_NONE) {
            bw_write_byte(&h, 0);
        } else if (fmt.filter.type == FILTER_DELTA16) {
            bw_write_byte(&h, 1);
        } else if (fmt.filter.type == FILTER_ROW_DELTA) {
            bw_write_byte(&h, 2);
            bw_write_vlq(&h, fmt.filter.stride);
        } else {
            bw_write_byte(&h, 3);
            bw_write_vlq(&h, fmt.filter.stride);
        }
        bw_write_vlq(&h, (uint32_t)(block_end - block_start));
        write_huff_table(&h, main_huff.len, MAIN_SYMS);
        write_huff_table(&h, dist_huff.len, DIST_CODES);
        size_t hdr_len;
        uint8_t *hdr = bw_into_bytes(&h, &hdr_len);

        if (output_len + hdr_len + bitstream_len > output_cap) {
            output_cap = output_len + hdr_len + bitstream_len + 65536;
            output = realloc(output, output_cap);
        }
        memcpy(output + output_len, hdr, hdr_len);
        output_len += hdr_len;
        memcpy(output + output_len, bitstream, bitstream_len);
        output_len += bitstream_len;

        free(bitstream);
        free(hdr);
        huff_free(&main_huff);
        huff_free(&dist_huff);
        free(main_freq);
        free(dist_freq);

        block_start = block_end;
    }

    free(filtered);
    *out_len = output_len;
    return output;
}

uint8_t *decompress(const uint8_t *compressed, size_t len, size_t *out_len) {
    if (len < 4) { *out_len = 0; return NULL; }

    BitReader r;
    br_init(&r, compressed, len);
    uint8_t *out = NULL;
    size_t out_len_val = 0;
    size_t out_cap = 0;

    Filter pending_filter;
    int has_pending_filter = 0;

    for (;;) {
        if (br_byte_pos(&r) >= len - 1) break;
        uint32_t magic = br_read_bits(&r, 32);

        if (magic == MAGIC_RAW) {
            uint32_t n = br_read_vlq(&r);
            size_t byte_pos = br_byte_pos(&r);
            if (byte_pos + n <= len) {
                if (out_len_val + n > out_cap) {
                    out_cap = out_len_val + n + 65536;
                    out = realloc(out, out_cap);
                }
                memcpy(out + out_len_val, compressed + byte_pos, n);
                out_len_val += n;
                br_advance_bytes(&r, n);
            }
        } else if (magic == MAGIC) {
            uint8_t filter_byte = (uint8_t)br_read_bits(&r, 8);
            Filter filter;
            if (filter_byte == 0) {
                filter.type = FILTER_NONE;
                filter.stride = 0;
            } else if (filter_byte == 1) {
                filter.type = FILTER_DELTA16;
                filter.stride = 0;
            } else if (filter_byte == 2) {
                filter.type = FILTER_ROW_DELTA;
                filter.stride = br_read_vlq(&r);
            } else {
                filter.type = FILTER_ROW_DELTA_XOR;
                filter.stride = br_read_vlq(&r);
            }

            if (filter_byte != 0 && !has_pending_filter) {
                pending_filter = filter;
                has_pending_filter = 1;
            }

            uint32_t block_n = br_read_vlq(&r);
            Huffman main_huff, dist_huff;
            read_huff_table(&r, &main_huff, MAIN_SYMS);
            read_huff_table(&r, &dist_huff, DIST_CODES);
            br_align(&r);

            size_t dst = out_len_val;
            if (out_len_val + block_n > out_cap) {
                out_cap = out_len_val + block_n + 65536;
                out = realloc(out, out_cap);
            }
            out_len_val += block_n;

            uint32_t decoded = 0;
            while (decoded < block_n) {
                uint16_t sym = huff_decode(&main_huff,
                    (uint32_t (*)(void *))br_read_bit, &r);
                if (sym < 256) {
                    out[dst + decoded] = (uint8_t)sym;
                    decoded++;
                } else {
                    int code_idx = sym_to_match(sym);
                    uint32_t ln = len_base[code_idx];
                    if (code_idx == 28) {
                        uint32_t marker = br_read_bit(&r);
                        if (marker == 1) {
                            ln += br_read_vlq(&r);
                        }
                    } else if (len_extra[code_idx] > 0) {
                        ln += br_read_bits(&r, len_extra[code_idx]);
                    }

                    uint16_t d_sym = huff_decode(&dist_huff,
                        (uint32_t (*)(void *))br_read_bit, &r);
                    uint32_t off;
                    if (d_sym == 31) {
                        off = br_read_vlq(&r);
                    } else {
                        off = dist_base[d_sym];
                        if (dist_extra[d_sym] > 0) {
                            off += br_read_bits(&r, dist_extra[d_sym]);
                        }
                    }

                    size_t src = (dst + decoded) - off;
                    for (uint32_t i = 0; i < ln; i++) {
                        out[dst + decoded + i] = out[src + i];
                    }
                    decoded += ln;
                }
            }
            br_align(&r);

            huff_free(&main_huff);
            huff_free(&dist_huff);
        } else {
            break;
        }
    }

    if (has_pending_filter) {
        if (pending_filter.type == FILTER_ROW_DELTA) {
            size_t s = pending_filter.stride;
            if (s > 0 && s < out_len_val) {
                for (size_t i = s; i < out_len_val; i++) {
                    out[i] = out[i] + out[i - s];
                }
            }
        } else if (pending_filter.type == FILTER_ROW_DELTA_XOR) {
            size_t s = pending_filter.stride;
            if (s > 0 && s < out_len_val) {
                for (size_t i = s; i < out_len_val; i++) {
                    out[i] = out[i] ^ out[i - s];
                }
            }
        } else if (pending_filter.type == FILTER_DELTA16) {
            int16_t prev = 0;
            size_t n = out_len_val / 2;
            for (size_t i = 0; i < n; i++) {
                int16_t delta = (int16_t)((uint16_t)out[i*2] | ((uint16_t)out[i*2+1] << 8));
                int16_t val = delta + prev;
                prev = val;
                out[i*2] = (uint8_t)(uint16_t)val;
                out[i*2+1] = (uint8_t)((uint16_t)val >> 8);
            }
        }
    }

    *out_len = out_len_val;
    return out;
}
