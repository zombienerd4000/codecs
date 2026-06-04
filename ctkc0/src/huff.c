#include "huff.h"
#include <string.h>
#include <stdlib.h>

typedef struct Node {
    uint32_t freq;
    uint16_t sym;
    int has_sym;
    struct Node *left;
    struct Node *right;
} Node;

typedef struct {
    Node **data;
    int len;
    int cap;
} Heap;

static void heap_init(Heap *h) {
    h->data = NULL;
    h->len = 0;
    h->cap = 0;
}

static void heap_push(Heap *h, Node *n) {
    if (h->len >= h->cap) {
        int new_cap = h->cap ? h->cap * 2 : 256;
        Node **new_data = realloc(h->data, new_cap * sizeof(Node *));
        if (!new_data) abort();
        h->data = new_data;
        h->cap = new_cap;
    }
    int i = h->len++;
    h->data[i] = n;
    while (i > 0) {
        int p = (i - 1) / 2;
        if (h->data[p]->freq <= h->data[i]->freq &&
            (h->data[p]->freq != h->data[i]->freq ||
             (h->data[p]->has_sym && h->data[i]->has_sym &&
              h->data[p]->sym <= h->data[i]->sym))) break;
        Node *t = h->data[p]; h->data[p] = h->data[i]; h->data[i] = t;
        i = p;
    }
}

static Node *heap_pop(Heap *h) {
    if (h->len == 0) return NULL;
    Node *top = h->data[0];
    h->data[0] = h->data[--h->len];
    int i = 0;
    for (;;) {
        int smallest = i;
        int l = 2 * i + 1;
        int r = 2 * i + 2;
        if (l < h->len) {
            if (h->data[l]->freq < h->data[smallest]->freq ||
                (h->data[l]->freq == h->data[smallest]->freq &&
                 h->data[l]->has_sym && h->data[smallest]->has_sym &&
                 h->data[l]->sym < h->data[smallest]->sym)) {
                smallest = l;
            }
        }
        if (r < h->len) {
            if (h->data[r]->freq < h->data[smallest]->freq ||
                (h->data[r]->freq == h->data[smallest]->freq &&
                 h->data[r]->has_sym && h->data[smallest]->has_sym &&
                 h->data[r]->sym < h->data[smallest]->sym)) {
                smallest = r;
            }
        }
        if (smallest == i) break;
        Node *t = h->data[i]; h->data[i] = h->data[smallest]; h->data[smallest] = t;
        i = smallest;
    }
    return top;
}

static void heap_free(Heap *h) {
    free(h->data);
}

static void walk_tree(Node *n, uint8_t depth, uint8_t *lens) {
    if (n->has_sym) {
        lens[n->sym] = depth;
    } else {
        if (n->left) walk_tree(n->left, depth + 1, lens);
        if (n->right) walk_tree(n->right, depth + 1, lens);
    }
}

static void free_tree(Node *n) {
    if (!n->has_sym) {
        if (n->left) free_tree(n->left);
        if (n->right) free_tree(n->right);
    }
    free(n);
}

static void limit_lengths(uint8_t *lens, size_t n, uint8_t max_bits) {
    uint32_t count[256];
    memset(count, 0, sizeof(count));
    uint8_t max = 0;
    for (size_t i = 0; i < n; i++) {
        if (lens[i] > 0) { count[lens[i]]++; if (lens[i] > max) max = lens[i]; }
    }
    if (max <= max_bits) return;

    for (int len = (int)max; len > (int)max_bits; len--) {
        while (count[len] > 0) {
            count[len]--;
            count[max_bits]++;
            for (int l = (int)max_bits - 1; l >= 0; l--) {
                if (count[l] > 0) {
                    count[l]--;
                    count[l + 1]++;
                    break;
                }
            }
        }
    }

    uint16_t *syms_by_len[256];
    int syms_by_len_count[256];
    memset(syms_by_len_count, 0, sizeof(syms_by_len_count));
    for (size_t i = 0; i < 256; i++) {
        syms_by_len[i] = NULL;
    }

    for (size_t i = 0; i < n; i++) {
        if (lens[i] > 0 && lens[i] <= max) {
            int idx = lens[i];
            syms_by_len[idx] = realloc(syms_by_len[idx], (syms_by_len_count[idx] + 1) * sizeof(uint16_t));
            syms_by_len[idx][syms_by_len_count[idx]++] = (uint16_t)i;
        }
    }

    uint16_t *available = NULL;
    int avail_count = 0;
    for (int len = 1; len <= (int)max; len++) {
        for (int j = 0; j < syms_by_len_count[len]; j++) {
            available = realloc(available, (avail_count + 1) * sizeof(uint16_t));
            available[avail_count++] = syms_by_len[len][j];
        }
    }

    uint8_t *new_lens = calloc(n, 1);
    int idx = 0;
    for (int len = 1; len <= (int)max_bits; len++) {
        for (uint32_t c = 0; c < count[len]; c++) {
            if (idx < avail_count) {
                new_lens[available[idx]] = (uint8_t)len;
                idx++;
            }
        }
    }
    memcpy(lens, new_lens, n);
    free(new_lens);
    free(available);
    for (int i = 0; i < 256; i++) free(syms_by_len[i]);
}

void huff_init(Huffman *h, size_t n_syms) {
    h->n_syms = n_syms;
    h->code = calloc(n_syms, sizeof(uint32_t));
    h->len = calloc(n_syms, 1);
    memset(h->first_code, 0, sizeof(h->first_code));
    memset(h->syms_at_len_count, 0, sizeof(h->syms_at_len_count));
    h->syms_at_len = calloc(17, sizeof(uint16_t *));
}

void huff_free(Huffman *h) {
    free(h->code);
    free(h->len);
    for (int i = 0; i < 17; i++) free(h->syms_at_len[i]);
    free(h->syms_at_len);
}

void huff_build_tables(Huffman *h) {
    size_t n = h->n_syms;
    for (int i = 0; i < 17; i++) {
        free(h->syms_at_len[i]);
        h->syms_at_len[i] = NULL;
        h->syms_at_len_count[i] = 0;
    }
    uint32_t code = 0;
    uint8_t prev_len = 0;
    for (int len = 1; len <= MAX_HUFF_BITS; len++) {
        int count = 0;
        for (size_t s = 0; s < n; s++) {
            if (h->len[s] == len) count++;
        }
        if (count == 0) continue;
        h->syms_at_len[len] = malloc(count * sizeof(uint16_t));
        int idx = 0;
        for (size_t s = 0; s < n; s++) {
            if (h->len[s] == len) h->syms_at_len[len][idx++] = (uint16_t)s;
        }
        for (int i = 0; i < count - 1; i++) {
            for (int j = i + 1; j < count; j++) {
                if (h->syms_at_len[len][i] > h->syms_at_len[len][j]) {
                    uint16_t t = h->syms_at_len[len][i];
                    h->syms_at_len[len][i] = h->syms_at_len[len][j];
                    h->syms_at_len[len][j] = t;
                }
            }
        }
        h->syms_at_len_count[len] = count;
        code <<= (len - prev_len);
        h->first_code[len] = code;
        for (int i = 0; i < count; i++) {
            uint16_t sym = h->syms_at_len[len][i];
            h->code[sym] = code;
            code++;
        }
        prev_len = (uint8_t)len;
    }
}

void huff_build(Huffman *h, const uint32_t *freqs) {
    size_t n = h->n_syms;
    Heap heap;
    heap_init(&heap);

    for (size_t s = 0; s < n; s++) {
        if (freqs[s] > 0) {
            Node *node = malloc(sizeof(Node));
            node->freq = freqs[s];
            node->sym = (uint16_t)s;
            node->has_sym = 1;
            node->left = NULL;
            node->right = NULL;
            heap_push(&heap, node);
        }
    }

    if (heap.len == 0) {
        h->len[0] = 1;
        huff_build_tables(h);
        heap_free(&heap);
        return;
    }
    if (heap.len == 1) {
        memset(h->len, 0, n);
        Node *only = heap_pop(&heap);
        h->len[only->sym] = 1;
        free(only);
        huff_build_tables(h);
        heap_free(&heap);
        return;
    }

    while (heap.len > 1) {
        Node *a = heap_pop(&heap);
        Node *b = heap_pop(&heap);
        Node *merged = malloc(sizeof(Node));
        merged->freq = a->freq + b->freq;
        merged->has_sym = 0;
        merged->sym = 0;
        merged->left = a;
        merged->right = b;
        heap_push(&heap, merged);
    }

    Node *root = heap_pop(&heap);
    memset(h->len, 0, n);
    walk_tree(root, 0, h->len);
    free_tree(root);
    heap_free(&heap);

    limit_lengths(h->len, n, MAX_HUFF_BITS);
    huff_build_tables(h);
}

void huff_encode(const Huffman *h, uint16_t sym, uint32_t *out_code, uint8_t *out_len) {
    *out_code = h->code[sym];
    *out_len = h->len[sym];
}

uint16_t huff_decode(const Huffman *h, uint32_t (*read_bit)(void *), void *ctx) {
    uint32_t code = 0;
    for (int len = 1; len <= MAX_HUFF_BITS; len++) {
        code = (code << 1) | read_bit(ctx);
        uint32_t idx = code - h->first_code[len];
        if (idx < (uint32_t)h->syms_at_len_count[len]) {
            return h->syms_at_len[len][idx];
        }
    }
    return 0;
}
