// Kernel benchmark for a columnar bulk pass with a query-chosen column order. Synthetic data, real memory layout.
// Version 3: adds the tighter check used by the byte counts of qcol2.py. Each picked column also adds a lower bound on the
// item's energy in that coordinate to one 16-bit accumulator per block, and the half-width is
// hc + sum_b w[b] * sqrt(norm_b^2 - energy read in block b), in floats. argv[6] = 0 runs version 2's check.
// Version 2 of colscan.c: the order selection is vectorised (threshold bisection and index compression in place of a scalar
// quickselect and a selection sort) and timed apart from the prefetch of the picked columns. argv[5] = 0 restores the scalar selection.
//
// Layout per cluster of M items: plane 1 as N columns of M nibbles (two items per byte), NBK block-norm columns of M bytes,
// one int16 base per item. A query, per cluster: picks its K2 best columns (partial selection on a per-column gain),
// builds a 16-entry int16 table per picked column, accumulates the first K1 columns over all items, forms each item's
// half-width from its block norms and tests both sides, then accumulates the remaining K2-K1 columns and tests again.
// No compaction: every item is carried through both stages, which is the worst case for this pass.
// Times are per item, single thread. The decisions on random data mean nothing; only the work is measured.
//
// build: gcc -O3 -march=native -o colscan3 colscan3.c
#include <immintrin.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define N 768
#define M 1024          /* items per cluster, a multiple of 128 */
#define NBK 6
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + 1e-9 * t.tv_nsec; }
static uint64_t rs = 88172645463325252ull;
static inline uint64_t rnd(void) { rs ^= rs << 13; rs ^= rs >> 7; rs ^= rs << 17; return rs; }

// indices of the k largest gains (unordered), by quickselect on a copy
static void topk(const float *gain, int n, int k, int *out) {
    static float buf[N]; memcpy(buf, gain, n * sizeof(float));
    int lo = 0, hi = n - 1, target = n - k;
    while (lo < hi) {
        float p = buf[(lo + hi) >> 1]; int i = lo, j = hi;
        while (i <= j) { while (buf[i] < p) i++; while (buf[j] > p) j--; if (i <= j) { float t = buf[i]; buf[i] = buf[j]; buf[j] = t; i++; j--; } }
        if (target <= j) hi = j; else if (target >= i) lo = i; else break;
    }
    float thr = buf[target]; int c = 0;
    for (int i = 0; i < n && c < k; i++) if (gain[i] > thr) out[c++] = i;
    for (int i = 0; i < n && c < k; i++) if (gain[i] == thr) out[c++] = i;
}


// threshold with exactly k gains above it when gains are distinct: bisection on the value, counting by vector compares
static float kth_threshold(const float *gain, int n, int k, float lo, float hi) {
    for (int it = 0; it < 30; it++) {
        float mid = 0.5f * (lo + hi); __m512 vm = _mm512_set1_ps(mid); int cnt = 0;
        for (int i = 0; i < n; i += 16) cnt += _mm_popcnt_u32(_mm512_cmp_ps_mask(_mm512_loadu_ps(gain + i), vm, _CMP_GT_OQ));
        if (cnt == k) return mid;
        if (cnt > k) lo = mid; else hi = mid;
    }
    return hi;
}
// the k1 largest gains first (unordered), then the next k2 - k1 (unordered)
static void topk_simd(const float *gain, int n, int k1, int k2, int *out) {
    __m512 vmx = _mm512_set1_ps(-1e30f), vmn = _mm512_set1_ps(1e30f);
    for (int i = 0; i < n; i += 16) { __m512 v = _mm512_loadu_ps(gain + i); vmx = _mm512_max_ps(vmx, v); vmn = _mm512_min_ps(vmn, v); }
    float hi = _mm512_reduce_max_ps(vmx), lo = _mm512_reduce_min_ps(vmn) - 1e-20f;
    float t1 = kth_threshold(gain, n, k1, lo, hi), t2 = kth_threshold(gain, n, k2, lo, t1);
    int tmp[N + 16], c = 0, c2 = 0; __m512i idx = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
    const __m512 v1 = _mm512_set1_ps(t1), v2 = _mm512_set1_ps(t2);
    for (int i = 0; i < n; i += 16) {
        __m512 v = _mm512_loadu_ps(gain + i); __mmask16 a = _mm512_cmp_ps_mask(v, v1, _CMP_GT_OQ), b = _mm512_cmp_ps_mask(v, v2, _CMP_GT_OQ) & ~a;
        _mm512_mask_compressstoreu_epi32(tmp + c, a, idx); c += _mm_popcnt_u32(a);
        _mm512_mask_compressstoreu_epi32(out + N + c2, b, idx); c2 += _mm_popcnt_u32(b);
        idx = _mm512_add_epi32(idx, _mm512_set1_epi32(16));
    }
    // ties at a threshold can leave a set a little short or long; pad or cut so the counts are exact (the work is what is timed)
    int m = 0;
    for (int i = 0; i < c && m < k1; i++) out[m++] = tmp[i];
    for (int i = 0; i < c2 && m < k2; i++) out[m++] = out[N + i];
    for (int i = 0; m < k2; i++) out[m++] = i;
}

static inline void accumulate(const uint8_t *cols, const int *pick, int k0, int k1, const int16_t (*lut)[16], int16_t *acc) {
    const __m512i m0f = _mm512_set1_epi8(0x0F);
    __m512i TL[64], TH[64];
    for (int c = k0; c < k1; c++) {
        uint8_t lo[16], hi[16];
        for (int e = 0; e < 16; e++) { lo[e] = (uint8_t)(lut[c][e] & 0xFF); hi[e] = (uint8_t)((uint16_t)lut[c][e] >> 8); }
        TL[c - k0] = _mm512_broadcast_i32x4(_mm_loadu_si128((const __m128i *)lo));
        TH[c - k0] = _mm512_broadcast_i32x4(_mm_loadu_si128((const __m128i *)hi));
    }
    for (int ch = 0; ch < M / 128; ch++) {
        __m512i a0 = _mm512_loadu_si512(acc + ch * 128), a1 = _mm512_loadu_si512(acc + ch * 128 + 32);
        __m512i a2 = _mm512_loadu_si512(acc + ch * 128 + 64), a3 = _mm512_loadu_si512(acc + ch * 128 + 96);
        for (int c = k0; c < k1; c++) {
            __m512i v = _mm512_loadu_si512(cols + (size_t)pick[c] * (M / 2) + ch * 64);
            __m512i l = _mm512_and_si512(v, m0f), h = _mm512_and_si512(_mm512_srli_epi16(v, 4), m0f);
            __m512i ll = _mm512_shuffle_epi8(TL[c - k0], l), lh = _mm512_shuffle_epi8(TH[c - k0], l);
            __m512i hl = _mm512_shuffle_epi8(TL[c - k0], h), hh = _mm512_shuffle_epi8(TH[c - k0], h);
            a0 = _mm512_add_epi16(a0, _mm512_unpacklo_epi8(ll, lh)); a1 = _mm512_add_epi16(a1, _mm512_unpackhi_epi8(ll, lh));
            a2 = _mm512_add_epi16(a2, _mm512_unpacklo_epi8(hl, hh)); a3 = _mm512_add_epi16(a3, _mm512_unpackhi_epi8(hl, hh));
        }
        _mm512_storeu_si512(acc + ch * 128, a0); _mm512_storeu_si512(acc + ch * 128 + 32, a1);
        _mm512_storeu_si512(acc + ch * 128 + 64, a2); _mm512_storeu_si512(acc + ch * 128 + 96, a3);
    }
}

// half-width = hc + sum_b w[b] * norm[b] (16-bit fixed point); undecided iff acc - hw < thr <= acc + hw (base folded into acc)
static inline int check(const int16_t *acc, const uint8_t *norms, const uint16_t *w, int16_t hc, int16_t thr, uint64_t *und) {
    int cnt = 0; const __m512i vthr = _mm512_set1_epi16(thr), vhc = _mm512_set1_epi16(hc);
    for (int i = 0; i < M; i += 32) {
        __m512i hw = vhc;
        for (int b = 0; b < NBK; b++) {
            __m512i r = _mm512_slli_epi16(_mm512_cvtepu8_epi16(_mm256_loadu_si256((const __m256i *)(norms + b * M + i))), 8);
            hw = _mm512_adds_epi16(hw, _mm512_mulhi_epu16(r, _mm512_set1_epi16((short)w[b])));
        }
        __m512i a = _mm512_loadu_si512(acc + i);
        __mmask32 out = _mm512_cmplt_epi16_mask(_mm512_adds_epi16(a, hw), vthr);      // upper bound below t: not a match
        __mmask32 in = _mm512_cmpge_epi16_mask(_mm512_subs_epi16(a, hw), vthr);       // lower bound at or above t: a match
        uint32_t u = ~(out | in); und[i >> 6] = (i & 32) ? (und[i >> 6] | ((uint64_t)u << 32)) : u; cnt += _mm_popcnt_u32(u);
    }
    return cnt;
}


// half-width = hc + sum_b w[b] * sqrt(max(0, norm[b]^2 - low[b])): the block norms less the energy already read
static inline int check_tight(const int16_t *acc, const uint8_t *norms, uint16_t (*low)[M], const float *wf, float hc, float thr, uint64_t *und) {
    int cnt = 0; const __m512 vthr = _mm512_set1_ps(thr); const __m512i zero = _mm512_setzero_si512();
    for (int i = 0; i < M; i += 16) {
        __m512 hw = _mm512_set1_ps(hc);
        for (int b = 0; b < NBK; b++) {
            __m512i nb = _mm512_cvtepu8_epi32(_mm_loadu_si128((const __m128i *)(norms + b * M + i)));
            __m512i lw = _mm512_cvtepu16_epi32(_mm256_loadu_si256((const __m256i *)(low[b] + i)));
            __m512 rem = _mm512_cvtepi32_ps(_mm512_max_epi32(_mm512_sub_epi32(_mm512_mullo_epi32(nb, nb), lw), zero));
            hw = _mm512_fmadd_ps(_mm512_sqrt_ps(rem), _mm512_set1_ps(wf[b]), hw);
        }
        __m512 a = _mm512_cvtepi32_ps(_mm512_cvtepi16_epi32(_mm256_loadu_si256((const __m256i *)(acc + i))));
        __mmask16 out = _mm512_cmp_ps_mask(_mm512_add_ps(a, hw), vthr, _CMP_LT_OQ), in = _mm512_cmp_ps_mask(_mm512_sub_ps(a, hw), vthr, _CMP_GE_OQ);
        uint32_t u = (uint16_t)~(out | in); if ((i & 63) == 0) und[i >> 6] = 0; und[i >> 6] |= (uint64_t)u << (i & 63); cnt += _mm_popcnt_u32(u);
    }
    return cnt;
}

int main(int argc, char **argv) {
    int K = argc > 1 ? atoi(argv[1]) : 1000, K1 = argc > 2 ? atoi(argv[2]) : 16, K2 = argc > 3 ? atoi(argv[3]) : 48, REP = argc > 4 ? atoi(argv[4]) : 3, SIMD = argc > 5 ? atoi(argv[5]) : 1, TIGHT = argc > 6 ? atoi(argv[6]) : 1;
    size_t csz = (size_t)N * (M / 2);
    uint8_t *cols = aligned_alloc(64, (size_t)K * csz), *norms = aligned_alloc(64, (size_t)K * NBK * M);
    int16_t *base = aligned_alloc(64, (size_t)K * M * 2);
    for (size_t i = 0; i < (size_t)K * csz; i += 8) { uint64_t r = rnd(); memcpy(cols + i, &r, 8); }
    for (size_t i = 0; i < (size_t)K * NBK * M; i++) norms[i] = (uint8_t)(rnd() >> 56);
    for (size_t i = 0; i < (size_t)K * M; i++) base[i] = (int16_t)((rnd() >> 52) - 2048);
    float *d2 = aligned_alloc(64, (size_t)K * N * sizeof(float)); static float lam[N] __attribute__((aligned(64)));
    for (int j = 0; j < N; j++) lam[j] = 1.0f / (1 + j);
    for (size_t i = 0; i < (size_t)K * N; i++) { float u = (float)(rnd() >> 40) / (1 << 24); d2[i] = u * u; }   // stands for the query's offset energies
    static int16_t acc[M] __attribute__((aligned(64))); static int16_t lut[64][16]; static uint64_t und[M / 64];
    static uint16_t low[NBK][M] __attribute__((aligned(64))); static int16_t lut2[64][16]; static int pk2[64]; float wf[NBK];
    static int pick[2 * N + 32]; static float gain[N] __attribute__((aligned(64))); uint16_t w[NBK];
    double tsel = 0, tpre = 0, tlut = 0, tacc = 0, tchk = 0, best = 1e9; long und_total = 0;
    for (int rep = 0; rep < REP; rep++) {
        double t0 = now(), s0 = 0, sp = 0, s1 = 0, s2 = 0, s3 = 0; und_total = 0;
        for (int k = 0; k < K; k++) {
            const uint8_t *cc = cols + (size_t)k * csz; double a = now();
            if (SIMD) {
                for (int j = 0; j < N; j += 16) _mm512_store_ps(gain + j, _mm512_mul_ps(_mm512_loadu_ps(d2 + (size_t)k * N + j), _mm512_load_ps(lam + j)));
                topk_simd(gain, N, K1, K2, pick);
            } else {
                for (int j = 0; j < N; j++) gain[j] = d2[(size_t)k * N + j] * lam[j];
                topk(gain, N, K2, pick);
                for (int i = 0; i < K1; i++) { int bi = i; for (int j = i + 1; j < K2; j++) if (gain[pick[j]] > gain[pick[bi]]) bi = j; int t = pick[i]; pick[i] = pick[bi]; pick[bi] = t; }
            }
            double ap = now();
            for (int c = 0; c < K2; c++) for (int l = 0; l < M / 2; l += 64) _mm_prefetch((const char *)(cc + (size_t)pick[c] * (M / 2) + l), _MM_HINT_T0);
            double b = now();
            for (int c = 0; c < K2; c++) { float wt = 6.0f * gain[pick[c]] / (gain[pick[0]] + 1e-9f); for (int e = 0; e < 16; e++) lut[c][e] = (int16_t)(wt * ((e & 7) + 0.5f) * ((e & 8) ? -1 : 1)); }
            for (int bb = 0; bb < NBK; bb++) w[bb] = (uint16_t)(40 + 7 * bb);
            double c0 = now();
            memcpy(acc, base + (size_t)k * M, M * 2);
            accumulate(cc, pick, 0, K1, lut, acc);
            if (TIGHT) {                                   // energy lower bounds of the same columns, block by block
                memset(low, 0, sizeof(low));
                for (int bb = 0; bb < NBK; bb++) { int nb_ = 0; for (int c = 0; c < K1; c++) if (pick[c] * NBK / N == bb) { pk2[nb_] = pick[c]; for (int e = 0; e < 16; e++) lut2[nb_][e] = (int16_t)((e & 7) * (e & 7) * 3); nb_++; }
                    if (nb_) accumulate(cc, pk2, 0, nb_, lut2, (int16_t *)low[bb]); }
            }
            double c1 = now();
            if (TIGHT) { for (int bb = 0; bb < NBK; bb++) wf[bb] = 0.4f + 0.07f * bb; und_total += check_tight(acc, norms + (size_t)k * NBK * M, low, wf, 5.0f, 100.0f, und); }
            else und_total += check(acc, norms + (size_t)k * NBK * M, w, 5, 100, und);
            double c2 = now();
            accumulate(cc, pick, K1, K2, lut, acc);
            if (TIGHT) {
                for (int bb = 0; bb < NBK; bb++) { int nb_ = 0; for (int c = K1; c < K2; c++) if (pick[c] * NBK / N == bb) { pk2[nb_] = pick[c]; for (int e = 0; e < 16; e++) lut2[nb_][e] = (int16_t)((e & 7) * (e & 7) * 3); nb_++; }
                    if (nb_) accumulate(cc, pk2, 0, nb_, lut2, (int16_t *)low[bb]); }
            }
            double c3 = now();
            for (int bb = 0; bb < NBK; bb++) w[bb] = (uint16_t)(20 + 3 * bb);
            if (TIGHT) { for (int bb = 0; bb < NBK; bb++) wf[bb] = 0.2f + 0.03f * bb; und_total += check_tight(acc, norms + (size_t)k * NBK * M, low, wf, 9.0f, 100.0f, und); }
            else und_total += check(acc, norms + (size_t)k * NBK * M, w, 5, 100, und);
            double c4 = now();
            s0 += ap - a; sp += b - ap; s1 += c0 - b; s2 += (c1 - c0) + (c3 - c2); s3 += (c2 - c1) + (c4 - c3);
        }
        double tot = now() - t0;
        if (tot < best) { best = tot; tsel = s0; tpre = sp; tlut = s1; tacc = s2; tchk = s3; }
    }
    double items = (double)K * M;
    printf("clusters %d x %d items, %d then %d more columns (%.1f B of code per item), %d block norms, %s selection, %s check\n", K, M, K1, K2 - K1, K2 / 2.0, NBK, SIMD ? "vector" : "scalar", TIGHT ? "energy-accounting" : "norms-only");
    printf("ns per item: total %.2f | order selection %.2f (%.2f us per cluster) | prefetch %.2f | tables %.2f | accumulate %.2f (%.3f per item-column) | two checks %.2f\n",
           1e9 * best / items, 1e9 * tsel / items, 1e6 * tsel / K, 1e9 * tpre / items, 1e9 * tlut / items, 1e9 * tacc / items, 1e9 * tacc / items / K2, 1e9 * tchk / items);
    printf("(timer overhead is inside the parts; undecided count on random data %ld)\n", und_total);
    return 0;
}
