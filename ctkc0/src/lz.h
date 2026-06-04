#ifndef LZ_H
#define LZ_H

#include <stdint.h>
#include <stdlib.h>

#define MIN_MATCH 3
#define MAX_MATCH 65535
#define WINDOW 65536
#define MAX_CANDIDATES 256
#define NICE_MATCH 128
#define HASH_MASK 0xFFFF

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

void ht_build(HashTables *ht, const uint8_t *data, size_t len, HashType type);
void ht_free(HashTables *ht);
int ht_find_match(const HashTables *ht, const uint8_t *data, size_t len, size_t pos, int64_t lit_cost, Token *out);
int64_t match_cost(uint32_t off, uint32_t ln);

#endif
