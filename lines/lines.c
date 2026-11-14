// Cache lines touched, and time to read and decode them, for several layouts of the per-item code stream, driven by
// the per-item demand that the native prototype dumps (`vexact ... demand`): for each query, answer size and row, the
// deepest level read (0: none, l + 1: through level l, nl + 1: through the last level and then the full vector).
//
// Layouts (W = the stream's bytes per item, rounded up):
//   rowtight      each item's stream contiguous at stride W; an item reads bytes [0, d) depth-first
//   row64/row128  each item's stream at an aligned stride (5 or 6 lines); reads in steps of 64 or 128 B
//   col64         blocks of 64 items per cluster, byte-column-major (one line = one byte of 64 items); level walk:
//                 a block reads the level's columns when any of its items is still undecided
//   col32         as built: per level, blocks of 32 items with 4-bit rows of 16 B (one line = 2 bytes of 32 items)
//   hyb:k:S:C     col64 for bytes [0, C); a block switches to rows (stride aligned, steps of S) at the first level
//                 where at most k of its items are still undecided, or where the level ends past C; rows hold the
//                 whole stream, so storage is C + row stride per item
// Full-vector reads (1,536 B per item) are the same for every layout and are counted apart.
//
// count mode: exact distinct lines, runs of consecutive lines (each run starts with a non-sequential access), and
// distinct 128-byte pairs (what adjacent-line prefetch brings), per query.
// time mode: replays the reads in order over a buffer of the layout's size filled with random bytes, decoding each
// 32-byte vector with two pshufb lookups (low and high nibble) into accumulators; L3 is flushed before every run.
//
// build: gcc -O3 -march=native -o lines lines.c -lm
#define _GNU_SOURCE
#include <immintrin.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>

static int nrows, ncl, nl, nq, nfr;
static int *cr0, *cr1;
static int Bi[64]; // cumulative stream bytes through level l, rounded up
static int W;

typedef struct {
    uint64_t off;
    uint32_t len; // bytes
    uint8_t kind; // 0 row item, 1 column block
} Seg;

typedef struct {
    char name[32];
    int kind; // 0 rowtight, 1 rowS, 2 colB, 3 hybrid
    int S, B, k, C;
    uint64_t size; // address space of the layout (bytes)
    uint64_t colsize;
    int rstride;
} Lay;

static Seg *segs;
static size_t nseg, capseg;
static inline void push(uint64_t off, uint32_t len, uint8_t kind) {
    if (nseg == capseg) {
        capseg = capseg ? capseg * 2 : 1 << 20;
        segs = realloc(segs, capseg * sizeof(Seg));
    }
    segs[nseg++] = (Seg){off, len, kind};
}

static int need_bytes(uint8_t v) {
    if (v == 0) return 0;
    int l = v > nl ? nl : v;
    return Bi[l - 1];
}

static int ceil_div(int a, int b) { return (a + b - 1) / b; }

// block bases for column layouts: cluster c's blocks of B items start at colbase[c]
static uint64_t *colbase;

static void layout_size(Lay *L) {
    if (L->kind == 0) L->size = (uint64_t)nrows * W + 64;
    if (L->kind == 1) {
        L->rstride = ceil_div(W, L->S) * L->S;
        L->size = (uint64_t)nrows * L->rstride;
    }
    if (L->kind == 2 || L->kind == 3) {
        int B = L->kind == 2 ? L->B : 64;
        uint64_t per = L->kind == 2 ? (L->B == 64 ? (uint64_t)W * 64 : (uint64_t)ceil_div(W, 2) * 64) : (uint64_t)L->C * 64;
        uint64_t o = 0;
        for (int c = 0; c < ncl; c++) {
            colbase[c] = o; // only valid for the last layout sized; recomputed in gen
            o += (uint64_t)ceil_div(cr1[c] - cr0[c], B) * per;
        }
        L->colsize = o;
        L->size = o;
        if (L->kind == 3) {
            L->rstride = ceil_div(W, L->S) * L->S;
            L->size = o + (uint64_t)nrows * L->rstride;
        }
    }
}

// cluster placement for the 50M-scale run: with spread set, cluster c's regions start at place[c] (its column blocks,
// then its rows); otherwise every region is dense, clusters concatenated
static uint64_t *place;
static int spread;

// Generate the reads of one query for layout L, in execution order. lv: the demand of every row (0 for rows the
// query does not read, including invisible ones).
static void gen(Lay *L, const uint8_t *lv) {
    nseg = 0;
    if (L->kind == 0 || L->kind == 1) {
        for (int c = 0; c < ncl; c++)
            for (int r = cr0[c]; r < cr1[c]; r++) {
                int d = need_bytes(lv[r]);
                if (!d) continue;
                uint64_t rr = spread ? (uint64_t)(r - cr0[c]) : (uint64_t)r;
                uint64_t b0 = spread ? place[c] : 0;
                if (L->kind == 0)
                    push(b0 + rr * W, d, 0);
                else
                    push(b0 + rr * L->rstride, ceil_div(d, L->S) * L->S, 0);
            }
        return;
    }
    int B = L->kind == 2 ? L->B : 64;
    uint64_t per = L->kind == 2 ? (L->B == 64 ? (uint64_t)W * 64 : (uint64_t)ceil_div(W, 2) * 64) : (uint64_t)L->C * 64;
    uint64_t cb = 0;
    for (int c = 0; c < ncl; c++) {
        int n = cr1[c] - cr0[c];
        int nb = ceil_div(n, B);
        // per block: the level walk
        for (int l = 0; l < nl; l++) {
            int lo = l ? Bi[l - 1] : 0, hi = Bi[l];
            for (int b = 0; b < nb; b++) {
                int a0 = cr0[c] + b * B, a1 = a0 + B < cr1[c] ? a0 + B : cr1[c];
                int s = 0;
                for (int r = a0; r < a1; r++) s += lv[r] > l;
                if (!s) continue;
                uint64_t base = (spread ? place[c] : cb) + (uint64_t)b * per;
                if (L->kind == 2) {
                    if (B == 64)
                        push(base + (uint64_t)lo * 64, (hi - lo) * 64, 1);
                    else {
                        int l0 = lo / 2, l1 = ceil_div(hi, 2);
                        // a line already read for the previous level (odd boundary) is not read again
                        if (l && (lo & 1)) l0 += 1;
                        if (l1 > l0) push(base + (uint64_t)l0 * 64, (l1 - l0) * 64, 1);
                    }
                    continue;
                }
                // hybrid: is the block in row mode from this level? (switch decided at the level's start)
                int sw_prev = 0;
                if (l) {
                    // the block switched at an earlier level if, at that level, it qualified; recompute
                    for (int m = 0; m < l; m++) {
                        int sm = 0;
                        for (int r = a0; r < a1; r++) sm += lv[r] > m;
                        if (sm && (sm <= L->k || Bi[m] > L->C)) { sw_prev = 1; break; }
                    }
                }
                if (sw_prev) continue;
                if (s <= L->k || hi > L->C) {
                    // switch: each survivor reads its row from the line holding byte lo to its need, in steps of S
                    uint64_t rb = spread ? place[c] + (uint64_t)nb * per : L->colsize;
                    for (int r = a0; r < a1; r++) {
                        if (lv[r] <= l) continue;
                        int d = need_bytes(lv[r]);
                        int st = lo / L->S * L->S, en = ceil_div(d, L->S) * L->S;
                        uint64_t rr = spread ? (uint64_t)(r - cr0[c]) : (uint64_t)r;
                        push(rb + rr * L->rstride + st, en - st, 0);
                    }
                } else
                    push(base + (uint64_t)lo * 64, (hi - lo) * 64, 1);
            }
        }
        cb += (uint64_t)nb * per;
    }
}

// count mode: distinct lines, runs and 128-byte pairs over the reads
static uint64_t *bits;
static size_t nbits_words;
static void count(Lay *L, uint64_t *lines, uint64_t *runs, uint64_t *pairs, uint64_t *bytes) {
    size_t nw = L->size / 64 / 64 + 2;
    memset(bits, 0, nw * 8);
    uint64_t by = 0;
    for (size_t i = 0; i < nseg; i++) {
        uint64_t a = segs[i].off / 64, e = (segs[i].off + segs[i].len + 63) / 64;
        by += segs[i].len;
        for (uint64_t x = a; x < e; x++) bits[x >> 6] |= 1ull << (x & 63);
    }
    uint64_t ln = 0, rn = 0, pr = 0;
    uint64_t prevtop = 0;
    for (size_t w = 0; w < nw; w++) {
        uint64_t v = bits[w];
        ln += __builtin_popcountll(v);
        // a run starts at a set bit whose predecessor is clear
        uint64_t pred = (v << 1) | prevtop;
        rn += __builtin_popcountll(v & ~pred);
        prevtop = v >> 63;
        // pairs: bits 2j or 2j+1
        uint64_t pv = (v | (v >> 1)) & 0x5555555555555555ull;
        pr += __builtin_popcountll(pv);
    }
    *lines = ln;
    *runs = rn;
    *pairs = pr;
    *bytes = by;
}

// time mode
static uint8_t *buf;
static uint8_t *flushbuf;
static volatile uint64_t sink;
static double now(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec + t.tv_nsec * 1e-9;
}
static void flush(void) {
    __m256i a = _mm256_setzero_si256();
    for (size_t i = 0; i < (64u << 20); i += 64) a = _mm256_add_epi8(a, _mm256_load_si256((const __m256i *)(flushbuf + i)));
    sink += _mm256_extract_epi64(a, 0);
}

static double replay(int pf) {
    const __m256i tab = _mm256_setr_epi8(0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 42, 45, 0, 3, 6, 9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 42, 45);
    const __m256i tab2 = _mm256_setr_epi8(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
    const __m256i m0f = _mm256_set1_epi8(0x0f);
    const __m256i thr = _mm256_set1_epi16(1000);
    uint64_t dec = 0;
    double t0 = now();
    for (size_t i = 0; i < nseg; i++) {
        if (pf && i + 8 < nseg) {
            const uint8_t *p = buf + segs[i + 8].off;
            uint32_t ln = segs[i + 8].len;
            for (uint32_t x = 0; x < ln && x < 256; x += 64) _mm_prefetch((const char *)p + x, _MM_HINT_T0);
        }
        const uint8_t *p = buf + segs[i].off;
        uint32_t len = segs[i].len;
        if (segs[i].kind == 0) {
            // one item: sum of table values over its bytes, then a compare
            __m256i acc = _mm256_setzero_si256();
            for (uint32_t x = 0; x < len; x += 32) {
                __m256i c = _mm256_loadu_si256((const __m256i *)(p + x));
                if (len - x < 32) {
                    // only the bytes of this read
                    __m256i idx = _mm256_setr_epi8(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31);
                    c = _mm256_and_si256(c, _mm256_cmpgt_epi8(_mm256_set1_epi8((char)(len - x)), idx));
                }
                __m256i lo = _mm256_and_si256(c, m0f), hi = _mm256_and_si256(_mm256_srli_epi16(c, 4), m0f);
                acc = _mm256_add_epi64(acc, _mm256_sad_epu8(_mm256_shuffle_epi8(tab, lo), _mm256_setzero_si256()));
                acc = _mm256_add_epi64(acc, _mm256_sad_epu8(_mm256_shuffle_epi8(tab2, hi), _mm256_setzero_si256()));
            }
            __m128i s = _mm_add_epi64(_mm256_castsi256_si128(acc), _mm256_extracti128_si256(acc, 1));
            uint64_t v = _mm_cvtsi128_si64(s) + _mm_extract_epi64(s, 1);
            dec += v > 1000;
        } else {
            // a block of 64 items: each line is one byte of every item; 16-bit accumulators, then a compare
            __m256i a0 = _mm256_setzero_si256(), a1 = a0, a2 = a0, a3 = a0;
            for (uint32_t x = 0; x < len; x += 64) {
                __m256i c0 = _mm256_load_si256((const __m256i *)(p + x));
                __m256i c1 = _mm256_load_si256((const __m256i *)(p + x + 32));
                __m256i r0 = _mm256_add_epi8(_mm256_shuffle_epi8(tab, _mm256_and_si256(c0, m0f)), _mm256_shuffle_epi8(tab2, _mm256_and_si256(_mm256_srli_epi16(c0, 4), m0f)));
                __m256i r1 = _mm256_add_epi8(_mm256_shuffle_epi8(tab, _mm256_and_si256(c1, m0f)), _mm256_shuffle_epi8(tab2, _mm256_and_si256(_mm256_srli_epi16(c1, 4), m0f)));
                a0 = _mm256_add_epi16(a0, _mm256_and_si256(r0, _mm256_set1_epi16(0xff)));
                a1 = _mm256_add_epi16(a1, _mm256_srli_epi16(r0, 8));
                a2 = _mm256_add_epi16(a2, _mm256_and_si256(r1, _mm256_set1_epi16(0xff)));
                a3 = _mm256_add_epi16(a3, _mm256_srli_epi16(r1, 8));
            }
            uint32_t m = _mm256_movemask_epi8(_mm256_cmpgt_epi16(a0, thr)) ^ _mm256_movemask_epi8(_mm256_cmpgt_epi16(a1, thr)) ^ _mm256_movemask_epi8(_mm256_cmpgt_epi16(a2, thr)) ^ _mm256_movemask_epi8(_mm256_cmpgt_epi16(a3, thr));
            dec += __builtin_popcount(m);
        }
    }
    double t = now() - t0;
    sink += dec;
    return t;
}

static int cmpd(const void *a, const void *b) {
    double x = *(const double *)a, y = *(const double *)b;
    return x < y ? -1 : x > y;
}

static uint64_t rng = 88172645463325252ull;
static inline uint64_t xr(void) {
    rng ^= rng << 13;
    rng ^= rng >> 7;
    rng ^= rng << 17;
    return rng;
}

int main(int argc, char **argv) {
    if (argc < 6) {
        fprintf(stderr, "usage: lines DIR CLASS MODE(count|time) SCALE(1|50) NQ [layouts...]\n");
        return 2;
    }
    const char *dir = argv[1], *cls = argv[2], *mode = argv[3];
    int scale = atoi(argv[4]);
    int nqt = atoi(argv[5]);
    char path[512];
    snprintf(path, sizeof path, "%s/levels.txt", dir);
    FILE *f = fopen(path, "r");
    double b;
    nl = 0;
    while (fscanf(f, "%lf", &b) == 1) Bi[nl++] = (int)ceil(b - 1e-9);
    fclose(f);
    W = Bi[nl - 1];
    snprintf(path, sizeof path, "%s/clusters.txt", dir);
    f = fopen(path, "r");
    cr0 = malloc(4 * 4096);
    cr1 = malloc(4 * 4096);
    int a, e;
    ncl = 0;
    while (fscanf(f, "%d %d", &a, &e) == 2) cr0[ncl] = a, cr1[ncl] = e, ncl++;
    fclose(f);
    nrows = cr0[ncl - 1];
    ncl--;
    nfr = 5;
    double fr[5] = {0.0001, 0.001, 0.01, 0.1, 0.5};
    snprintf(path, sizeof path, "%s/%s_lv.u8", dir, cls);
    f = fopen(path, "rb");
    fseek(f, 0, SEEK_END);
    long fsz = ftell(f);
    fseek(f, 0, SEEK_SET);
    nq = fsz / ((long)nfr * nrows);
    uint8_t *lv = malloc(fsz);
    if (fread(lv, 1, fsz, f) != (size_t)fsz) return 3;
    fclose(f);
    if (nqt > nq) nqt = nq;
    colbase = calloc(ncl, 8);
    place = calloc(ncl, 8);
    // visibility masks
    uint8_t *vis10 = malloc(nrows);
    for (int r = 0; r < nrows; r++) vis10[r] = xr() % 10 == 0;
    uint8_t *cur = malloc(nrows);

    // layouts
    const char *defl[] = {"rowtight", "row64", "row128", "col64", "col32", "hyb:0:64:43", "hyb:0:64:22", "hyb:0:128:43", "hyb:1:64:285", "hyb:2:64:285", "hyb:4:64:285", "hyb:8:64:285", "hyb:16:64:285", "hyb:32:64:285", "hyb:4:128:285", "hyb:8:128:285", "hyb:16:128:285", "hyb:8:64:84", "hyb:16:64:84", "hyb:8:64:43", "hyb:16:64:43"};
    int nlay = argc > 6 ? argc - 6 : (int)(sizeof defl / sizeof *defl);
    Lay *Ls = calloc(nlay, sizeof(Lay));
    uint64_t maxsize = 0;
    for (int i = 0; i < nlay; i++) {
        const char *s = argc > 6 ? argv[6 + i] : defl[i];
        Lay *L = &Ls[i];
        snprintf(L->name, sizeof L->name, "%s", s);
        if (!strcmp(s, "rowtight")) L->kind = 0;
        else if (!strncmp(s, "row", 3)) L->kind = 1, L->S = atoi(s + 3);
        else if (!strncmp(s, "col", 3)) L->kind = 2, L->B = atoi(s + 3);
        else {
            L->kind = 3;
            sscanf(s, "hyb:%d:%d:%d", &L->k, &L->S, &L->C);
            if (L->C > W) L->C = W;
        }
        layout_size(L);
        if (L->size > maxsize) maxsize = L->size;
    }
    int timing = !strcmp(mode, "time");
    uint64_t span = maxsize;
    if (scale > 1) {
        // clusters spread over scale times the space (4 KB pages, so only the pages read take memory); a slot holds
        // the largest cluster's regions in the largest layout
        spread = 1;
        int maxn = 0;
        for (int c = 0; c < ncl; c++) maxn = cr1[c] - cr0[c] > maxn ? cr1[c] - cr0[c] : maxn;
        uint64_t big = (uint64_t)(maxn + 64) * (maxsize / nrows + 64) * 2;
        uint64_t slot = maxsize / ncl * scale;
        if (slot < big) slot = big;
        slot = (slot + 4095) / 4096 * 4096;
        int *perm = malloc(ncl * sizeof(int));
        for (int c = 0; c < ncl; c++) perm[c] = c;
        for (int c = ncl - 1; c > 0; c--) {
            int j = xr() % (c + 1), t = perm[c];
            perm[c] = perm[j], perm[j] = t;
        }
        span = slot * ncl + maxsize;
        for (int c = 0; c < ncl; c++) place[c] = (uint64_t)perm[c] * slot;
        free(perm);
    }
    nbits_words = span / 64 / 64 + 2;
    if (!timing) bits = malloc(nbits_words * 8);
    if (timing) {
        buf = mmap(NULL, span + 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0);
        if (scale == 1) madvise(buf, span + 4096, MADV_HUGEPAGE);
        flushbuf = aligned_alloc(64, 64u << 20);
        for (size_t i = 0; i < (64u << 20); i++) flushbuf[i] = i;
    }
    printf("class,layout,frac,vis,query,lines,runs,pairs,bytes,segs,full,touched,ms,ms_pf\n");
    for (int li = 0; li < nlay; li++) {
        Lay *L = &Ls[li];
        if (timing) {
            // fill every page this layout's reads can reach with random bytes (the whole layout for scale 1)
            if (scale == 1) {
                for (uint64_t x = 0; x < L->size; x += 8) *(uint64_t *)(buf + x) = xr();
            }
        }
        if (timing && scale > 1) madvise(buf, span + 4096, MADV_DONTNEED);
        for (int q = 0; q < nqt; q++)
            for (int fi = 0; fi < nfr; fi++)
                for (int v = 0; v < 2; v++) {
                    const uint8_t *src = lv + ((size_t)q * nfr + fi) * nrows;
                    uint64_t full = 0, touched = 0;
                    for (int r = 0; r < nrows; r++) {
                        uint8_t x = src[r];
                        if (v && !vis10[r]) x = 0;
                        cur[r] = x;
                        full += x == nl + 1;
                        touched += x > 0;
                    }
                    gen(L, cur);
                    if (!timing) {
                        uint64_t ln, rn, pr, by;
                        count(L, &ln, &rn, &pr, &by);
                        printf("%s,%s,%g,%s,%d,%lu,%lu,%lu,%lu,%zu,%lu,%lu,,\n", cls, L->name, fr[fi], v ? "10%" : "all", q, ln, rn, pr, by, nseg, full, touched);
                    } else {
                        if (scale > 1) {
                            // fill the pages this query reads
                            for (size_t i = 0; i < nseg; i++) {
                                uint64_t a0 = segs[i].off & ~4095ull, a1 = (segs[i].off + segs[i].len + 4095) & ~4095ull;
                                for (uint64_t x = a0; x < a1; x += 4096)
                                    if (!buf[x + 1]) {
                                        for (int y = 0; y < 4096; y += 8) *(uint64_t *)(buf + x + y) = xr() | 0xff00;
                                    }
                            }
                        }
                        double t[5], tp[5];
                        for (int rep = 0; rep < 5; rep++) {
                            flush();
                            t[rep] = replay(0);
                            flush();
                            tp[rep] = replay(1);
                        }
                        qsort(t, 5, 8, cmpd);
                        qsort(tp, 5, 8, cmpd);
                        uint64_t by = 0;
                        for (size_t i = 0; i < nseg; i++) by += segs[i].len;
                        printf("%s,%s,%g,%s,%d,,,,%lu,%zu,%lu,%lu,%.4f,%.4f\n", cls, L->name, fr[fi], v ? "10%" : "all", q, by, nseg, full, touched, t[2] * 1e3, tp[2] * 1e3);
                    }
                    fflush(stdout);
                }
    }
    return 0;
}
