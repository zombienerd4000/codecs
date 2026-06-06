#ifndef LZ_H
#define LZ_H

#include <stdint.h>
#include <stdlib.h>

#define MIN_MATCH 3
#define MAX_MATCH 65535
#define WINDOW 65536
#define MAX_CANDIDATES 2560
#define NICE_MATCH 128
#define HASH_MASK 0x1FFFF

typedef enum {
    HASH_TYPE_HASH4,
    HASH_TYPE_HASH3,
} HashType;

typedef struct {
    HashType ht;
    uint32_t *entries;
    uint32_t *offsets;
    size_t entries_len;
} HashTables;

typedef struct {
    int is_match;
    uint32_t off;
    uint32_t ln;
    uint8_t lit;
} Token;

typedef struct {
    uint32_t *data;  // packed: low 31 bits = off/byte, high bit = is_match; second uint = ln
    size_t count;
    size_t cap;
} TokenBuf;

void tb_init(TokenBuf *tb);
void tb_push(TokenBuf *tb, int is_match, uint8_t lit, uint32_t off, uint32_t ln);
void tb_free(TokenBuf *tb);

void ht_build(HashTables *ht, const uint8_t *data, size_t len, HashType type);
void ht_free(HashTables *ht);
int ht_find_match(const HashTables *ht, const uint8_t *data, size_t len, size_t pos, int64_t lit_cost, Token *out);
int64_t match_cost(uint32_t off, uint32_t ln, int64_t lit_cost);
extern const uint8_t *g_match_main_lens;
extern const uint8_t *g_match_dist_lens;
extern const uint8_t *g_match_main_lens;
extern const uint8_t *g_match_dist_lens;

#endif
