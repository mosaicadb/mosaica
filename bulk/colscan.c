// Kernel benchmark for a columnar bulk pass with a query-chosen column order. Synthetic data, real memory layout.
//
// Layout per cluster of M items: plane 1 as N columns of M nibbles (two items per byte), NBK block-norm columns of M bytes,
// one int16 base per item. A query, per cluster: picks its K2 best columns (partial selection on a per-column gain),
// builds a 16-entry int16 table per picked column, accumulates the first K1 columns over all items, forms each item's
// half-width from its block norms and tests both sides, then accumulates the remaining K2-K1 columns and tests again.
// No compaction: every item is carried through both stages, which is the worst case for this pass.
// Times are per item, single thread. The decisions on random data mean nothing; only the work is measured.
//
// build: gcc -O3 -march=native -o colscan colscan.c
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

int main(int argc, char **argv) {
    int K = argc > 1 ? atoi(argv[1]) : 1000, K1 = argc > 2 ? atoi(argv[2]) : 16, K2 = argc > 3 ? atoi(argv[3]) : 48, REP = argc > 4 ? atoi(argv[4]) : 3;
    size_t csz = (size_t)N * (M / 2);
    uint8_t *cols = aligned_alloc(64, (size_t)K * csz), *norms = aligned_alloc(64, (size_t)K * NBK * M);
    int16_t *base = aligned_alloc(64, (size_t)K * M * 2);
    for (size_t i = 0; i < (size_t)K * csz; i += 8) { uint64_t r = rnd(); memcpy(cols + i, &r, 8); }
    for (size_t i = 0; i < (size_t)K * NBK * M; i++) norms[i] = (uint8_t)(rnd() >> 56);
    for (size_t i = 0; i < (size_t)K * M; i++) base[i] = (int16_t)((rnd() >> 52) - 2048);
    float *d2 = malloc((size_t)K * N * sizeof(float)), lam[N];
    for (int j = 0; j < N; j++) lam[j] = 1.0f / (1 + j);
    for (size_t i = 0; i < (size_t)K * N; i++) { float u = (float)(rnd() >> 40) / (1 << 24); d2[i] = u * u; }   // stands for the query's offset energies
    static int16_t acc[M] __attribute__((aligned(64))); static int16_t lut[64][16]; static uint64_t und[M / 64];
    int pick[64]; float gain[N]; uint16_t w[NBK];
    double tsel = 0, tlut = 0, tacc = 0, tchk = 0, best = 1e9; long und_total = 0;
    for (int rep = 0; rep < REP; rep++) {
        double t0 = now(), s0 = 0, s1 = 0, s2 = 0, s3 = 0; und_total = 0;
        for (int k = 0; k < K; k++) {
            const uint8_t *cc = cols + (size_t)k * csz; double a = now();
            for (int j = 0; j < N; j++) gain[j] = d2[(size_t)k * N + j] * lam[j];
            topk(gain, N, K2, pick);
            // order the picks so the first K1 are the best of them
            for (int i = 0; i < K1; i++) { int bi = i; for (int j = i + 1; j < K2; j++) if (gain[pick[j]] > gain[pick[bi]]) bi = j; int t = pick[i]; pick[i] = pick[bi]; pick[bi] = t; }
            for (int c = 0; c < K2; c++) for (int l = 0; l < M / 2; l += 64) _mm_prefetch((const char *)(cc + (size_t)pick[c] * (M / 2) + l), _MM_HINT_T0);
            double b = now();
            for (int c = 0; c < K2; c++) { float wt = 6.0f * gain[pick[c]] / (gain[pick[0]] + 1e-9f); for (int e = 0; e < 16; e++) lut[c][e] = (int16_t)(wt * ((e & 7) + 0.5f) * ((e & 8) ? -1 : 1)); }
            for (int bb = 0; bb < NBK; bb++) w[bb] = (uint16_t)(40 + 7 * bb);
            double c0 = now();
            memcpy(acc, base + (size_t)k * M, M * 2);
            accumulate(cc, pick, 0, K1, lut, acc);
            double c1 = now();
            und_total += check(acc, norms + (size_t)k * NBK * M, w, 5, 100, und);
            double c2 = now();
            accumulate(cc, pick, K1, K2, lut, acc);
            double c3 = now();
            for (int bb = 0; bb < NBK; bb++) w[bb] = (uint16_t)(20 + 3 * bb);
            und_total += check(acc, norms + (size_t)k * NBK * M, w, 9, 100, und);
            double c4 = now();
            s0 += b - a; s1 += c0 - b; s2 += (c1 - c0) + (c3 - c2); s3 += (c2 - c1) + (c4 - c3);
        }
        double tot = now() - t0;
        if (tot < best) { best = tot; tsel = s0; tlut = s1; tacc = s2; tchk = s3; }
    }
    double items = (double)K * M;
    printf("clusters %d x %d items, %d then %d more columns (%.1f B of code per item), %d block norms\n", K, M, K1, K2 - K1, K2 / 2.0, NBK);
    printf("ns per item: total %.2f | order selection and prefetch %.2f | tables %.2f | accumulate %.2f (%.3f per item-column) | two checks %.2f\n",
           1e9 * best / items, 1e9 * tsel / items, 1e9 * tlut / items, 1e9 * tacc / items, 1e9 * tacc / items / K2, 1e9 * tchk / items);
    printf("(timer overhead is inside the parts; undecided count on random data %ld)\n", und_total);
    return 0;
}
