#include "lz.h"
#include <string.h>

#ifdef _MSC_VER
#include <intrin.h>
static uint32_t my_ctzll(uint64_t x) {
    unsigned long idx;
    _BitScanForward64(&idx, x);
    return (uint32_t)idx;
}
#define CTZLL(x) my_ctzll(x)
#else
#define CTZLL(x) __builtin_ctzll(x)
#endif

void tb_init(TokenBuf *tb) {
    tb->data = NULL;
    tb->count = 0;
    tb->cap = 0;
}

void tb_push(TokenBuf *tb, int is_match, uint8_t lit, uint32_t off, uint32_t ln) {
    if (tb->count * 2 + 2 > tb->cap) {
        tb->cap = tb->cap ? tb->cap * 2 : 16384;
        tb->data = realloc(tb->data, tb->cap * sizeof(uint32_t));
    }
    uint32_t tag = (uint32_t)(is_match ? 0x80000000U : 0) | (is_match ? off : lit);
    tb->data[tb->count * 2] = tag;
    tb->data[tb->count * 2 + 1] = ln;
    tb->count++;
}

void tb_free(TokenBuf *tb) {
    free(tb->data);
    tb->data = NULL;
    tb->count = 0;
    tb->cap = 0;
}

static uint32_t hash_4(const uint8_t *data, size_t i) {
    uint32_t v = *(const uint32_t *)(data + i);
    return ((v * 2654435761U) >> 15) & HASH_MASK;
}

static uint32_t hash_3(const uint8_t *data, size_t i) {
    uint32_t v = (uint32_t)data[i] | ((uint32_t)data[i + 1] << 8) | ((uint32_t)data[i + 2] << 16);
    return ((v * 2654435761U) >> 15) & HASH_MASK;
}

void ht_build(HashTables *ht, const uint8_t *data, size_t len, HashType type) {
    ht->ht = type;
    ht->entries = NULL;
    ht->entries_len = 0;
    ht->offsets = NULL;

    size_t n = len;
    uint32_t *counts = calloc(HASH_MASK + 1, sizeof(uint32_t));
    uint32_t *offsets = calloc(HASH_MASK + 2, sizeof(uint32_t));

    if (type == HASH_TYPE_HASH4) {
        if (n < 4) { free(counts); free(offsets); ht->offsets = offsets; return; }
        for (size_t i = 0; i < n - 3; i++) {
            counts[hash_4(data, i)]++;
        }
    } else {
        if (n < 3) { free(counts); free(offsets); ht->offsets = offsets; return; }
        for (size_t i = 0; i < n - 2; i++) {
            counts[hash_3(data, i)]++;
        }
    }

    uint32_t sum = 0;
    for (int slot = 0; slot <= HASH_MASK; slot++) {
        offsets[slot] = sum;
        sum += counts[slot];
    }
    offsets[HASH_MASK + 1] = sum;

    uint32_t *entries = malloc(sum * sizeof(uint32_t));
    uint32_t *cursors = malloc((HASH_MASK + 1) * sizeof(uint32_t));
    memcpy(cursors, offsets, (HASH_MASK + 1) * sizeof(uint32_t));

    if (type == HASH_TYPE_HASH4) {
        for (size_t i = 0; i < n - 3; i++) {
            uint32_t key = hash_4(data, i);
            uint32_t slot = cursors[key];
            entries[slot] = (uint32_t)i;
            cursors[key] = slot + 1;
        }
    } else {
        for (size_t i = 0; i < n - 2; i++) {
            uint32_t key = hash_3(data, i);
            uint32_t slot = cursors[key];
            entries[slot] = (uint32_t)i;
            cursors[key] = slot + 1;
        }
    }

    ht->entries = entries;
    ht->entries_len = sum;
    ht->offsets = offsets;

    free(counts);
    free(cursors);
}

void ht_free(HashTables *ht) {
    free(ht->entries);
    free(ht->offsets);
}



static void find_in_slice(const uint8_t *data, size_t pos, const uint32_t *slice, size_t slice_len,
                          size_t max_len, uint32_t nice, uint32_t *best_off, uint32_t *best_ln,
                          int64_t *best_sav, int64_t lit_cost, uint32_t min_match) {
    size_t lo = 0, hi = slice_len;
    while (lo < hi) {
        size_t mid = (lo + hi) / 2;
        if (slice[mid] < (uint32_t)pos) lo = mid + 1;
        else hi = mid;
    }
    size_t n_candidates = lo;
    size_t slot_limit = MAX_CANDIDATES < MAX_SLOT_CANDIDATES ? MAX_CANDIDATES : MAX_SLOT_CANDIDATES;
    size_t iter_start = (n_candidates > slot_limit) ? n_candidates - slot_limit : 0;
    if (n_candidates == iter_start) return;

    uint32_t pos_bytes = *(const uint32_t *)(data + pos);

    for (size_t ci = n_candidates; ci > iter_start; ) {
        ci--;
        size_t cu = slice[ci];
        size_t diff = pos - cu;
        if (diff > WINDOW) break;

        if ((*(const uint32_t *)(data + cu) & 0x00FFFFFF) != (pos_bytes & 0x00FFFFFF)) continue;

        size_t ln = 3;
        size_t max_ln = max_len;
        while (ln + 8 <= max_ln) {
            uint64_t va = *(const uint64_t *)(data + pos + ln);
            uint64_t vb = *(const uint64_t *)(data + cu + ln);
            if (va != vb) {
                ln += (va ^ vb) ? (CTZLL(va ^ vb) / 8) : 0;
                break;
            }
            ln += 8;
        }
        while (ln < max_ln && data[pos + ln] == data[cu + ln]) ln++;

        if (ln >= min_match) {
            int64_t sav = (int64_t)ln * lit_cost - match_cost((uint32_t)diff, (uint32_t)ln, lit_cost);
            if (sav > *best_sav) {
                *best_sav = sav;
                *best_off = (uint32_t)diff;
                *best_ln = (uint32_t)ln;
            }
            if (ln >= nice) break;
        }
    }
}

int ht_find_match(const HashTables *ht, const uint8_t *data, size_t len, size_t pos, int64_t lit_cost, Token *out) {
    uint32_t min_match = MIN_MATCH;
    uint32_t nice = (lit_cost <= 3) ? MAX_MATCH : (g_is_text_block ? 24 : NICE_MATCH);
    out->is_match = 0;

    if (ht->ht == HASH_TYPE_HASH4) {
        if (pos + 4 > len) return 0;
        uint32_t hash = hash_4(data, pos);
        uint32_t start = ht->offsets[hash];
        uint32_t end = ht->offsets[hash + 1];
        if (start >= end) return 0;
        size_t max_len = (len - pos) < MAX_MATCH ? (len - pos) : MAX_MATCH;

        uint32_t best_off = 0, best_ln = 0;
        int64_t best_sav = 0;

        find_in_slice(data, pos, ht->entries + start, end - start, max_len, nice,
                      &best_off, &best_ln, &best_sav, lit_cost, min_match);

        if (best_ln >= MIN_MATCH && best_sav > 0) {
            out->is_match = 1;
            out->off = best_off;
            out->ln = best_ln;
            return 1;
        }
    } else {
        if (pos + 3 > len) return 0;
        uint32_t hash = hash_3(data, pos);
        uint32_t start = ht->offsets[hash];
        uint32_t end = ht->offsets[hash + 1];
        if (start >= end) return 0;
        size_t max_len = (len - pos) < MAX_MATCH ? (len - pos) : MAX_MATCH;

        uint32_t best_off = 0, best_ln = 0;
        int64_t best_sav = 0;

        find_in_slice(data, pos, ht->entries + start, end - start, max_len, nice,
                      &best_off, &best_ln, &best_sav, lit_cost, min_match);

        if (best_ln >= MIN_MATCH && best_sav > 0) {
            out->is_match = 1;
            out->off = best_off;
            out->ln = best_ln;
            return 1;
        }
    }
    return 0;
}

int ht_find_match_cached(MatchCache *mc, const HashTables *ht, const uint8_t *data, size_t len, size_t pos, int64_t lit_cost, Token *out) {
    size_t idx = pos & 7;
    if (mc->valid[idx] && mc->positions[idx] == pos) {
        if (mc->tokens[idx].is_match) {
            *out = mc->tokens[idx];
            return 1;
        }
        return 0;
    }
    int r = ht_find_match(ht, data, len, pos, lit_cost, out);
    mc->valid[idx] = 1;
    mc->positions[idx] = pos;
    mc->tokens[idx] = *out;
    return r;
}
