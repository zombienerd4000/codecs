#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "codec.h"

static double now_ms(void) {
    clock_t c = clock();
    return (double)c * 1000.0 / (double)CLOCKS_PER_SEC;
}

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "ctkc0: Scan-based LZ77 compressor (C port)\n");
        fprintf(stderr, "Usage:\n");
        fprintf(stderr, "  ctkc0 c <input> [output]   Compress\n");
        fprintf(stderr, "  ctkc0 d <input> [output]   Decompress\n");
        fprintf(stderr, "  ctkc0 t <input>            Test round-trip\n");
        return 1;
    }

    if (argv[1][0] == 'c' && argv[1][1] == '\0') {
        if (argc < 3) { fprintf(stderr, "usage: ctkc0 c <input> [output]\n"); return 1; }
        FILE *f = fopen(argv[2], "rb");
        if (!f) { perror("fopen"); return 1; }
        fseek(f, 0, SEEK_END);
        long fsize = ftell(f);
        fseek(f, 0, SEEK_SET);
        uint8_t *data = malloc(fsize);
        fread(data, 1, fsize, f);
        fclose(f);

        double t0 = now_ms();
        size_t out_len;
        uint8_t *out = compress(data, fsize, &out_len);
        double elapsed = now_ms() - t0;

        char dest[1024];
        if (argc > 3) {
            strcpy(dest, argv[3]);
        } else {
            snprintf(dest, sizeof(dest), "%s.ctkc0", argv[2]);
        }
        FILE *fo = fopen(dest, "wb");
        if (!fo) { perror("fopen"); return 1; }
        fwrite(out, 1, out_len, fo);
        fclose(fo);

        double pct = 100.0 * out_len / fsize;
        fprintf(stderr, "%s -> %s (%.1f%%, %.1fms)\n", argv[2], dest, pct, elapsed);
        free(data); free(out);
        return 0;
    }

    if (argv[1][0] == 'd' && argv[1][1] == '\0') {
        if (argc < 3) { fprintf(stderr, "usage: ctkc0 d <input> [output]\n"); return 1; }
        FILE *f = fopen(argv[2], "rb");
        if (!f) { perror("fopen"); return 1; }
        fseek(f, 0, SEEK_END);
        long fsize = ftell(f);
        fseek(f, 0, SEEK_SET);
        uint8_t *data = malloc(fsize);
        fread(data, 1, fsize, f);
        fclose(f);

        double t0 = now_ms();
        size_t out_len;
        uint8_t *out = decompress(data, fsize, &out_len);
        double elapsed = now_ms() - t0;

        char dest[1024];
        if (argc > 3) {
            strcpy(dest, argv[3]);
        } else {
            size_t n = strlen(argv[2]);
            if (n > 6 && strcmp(argv[2] + n - 6, ".ctkc0") == 0) {
                memcpy(dest, argv[2], n - 6);
                dest[n - 6] = '\0';
            } else {
                snprintf(dest, sizeof(dest), "%s.out", argv[2]);
            }
        }
        FILE *fo = fopen(dest, "wb");
        if (!fo) { perror("fopen"); return 1; }
        fwrite(out, 1, out_len, fo);
        fclose(fo);

        fprintf(stderr, "%s -> %s (%zuB, %.1fms)\n", argv[2], dest, out_len, elapsed);
        free(data); free(out);
        return 0;
    }

    if (argv[1][0] == 't' && argv[1][1] == '\0') {
        if (argc < 3) { fprintf(stderr, "usage: ctkc0 t <input>\n"); return 1; }
        FILE *f = fopen(argv[2], "rb");
        if (!f) { perror("fopen"); return 1; }
        fseek(f, 0, SEEK_END);
        long fsize = ftell(f);
        fseek(f, 0, SEEK_SET);
        uint8_t *data = malloc(fsize);
        fread(data, 1, fsize, f);
        fclose(f);

        double t0 = now_ms();
        size_t clen;
        uint8_t *c = compress(data, fsize, &clen);
        double ct = now_ms() - t0;

        t0 = now_ms();
        size_t dlen;
        uint8_t *dec = decompress(c, clen, &dlen);
        double dt = now_ms() - t0;

        int ok = (fsize == (long)dlen) && (memcmp(data, dec, fsize) == 0);
        double pct = 100.0 * clen / fsize;
        fprintf(stderr, "%s: %ldB -> %zuB (%.1f%%) enc:%.1fms dec:%.1fms [%s]\n",
                argv[2], fsize, clen, pct, ct, dt, ok ? "OK" : "FAIL");

        free(data); free(c); free(dec);
        return ok ? 0 : 1;
    }

    fprintf(stderr, "unknown command: %s\n", argv[1]);
    return 1;
}
