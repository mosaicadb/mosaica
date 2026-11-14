# Exact k-NN cascade: numpy confirmation on all 10,000 test queries

The design (level-0 report, sections 1, 2 and 5) is a cascade per query at k = 10 with every cluster scanned:
seed tau from the 30 best 1-bit estimates, then a 1-bit, 2-bit and 4-bit level, each dropping an item whose
upper bound is below tau, then an exact score for what remains. This run checks it on glove-100-angular
(N = 1,183,514, n = 100, K = 4,096) and nytimes-256-angular (N = 290,000, n = 256, K = 1,024), all 10,000 test
queries of each, with the saved plain k-means structures (`probe/K*.npz`, `C0`/`a0`).

## How it was run

- `build.py`: residuals to the saved centres, one sign-fixed Haar rotation (seed 7), odd-grid codes as in
  `probe/expB.py`, but with the scale chosen per item from its own rotated vector (the referee's repair: a scale
  fitted on other rotated items makes the code depend on the rotation through them). Codes: 1 bit = sign;
  2 bit = sign + one magnitude bit at the item's best 2-bit scale s2; 4 bit **nested** = scale 4·s2, whose top
  magnitude bit is the 2-bit bit (asserted); 4 bit **independent** = the item's best 4-bit scale s4;
  **tied4** = 2 bit at s4/4, nested under the independent 4-bit code (the other way to tie the scales).
- `run.py`: every item's three upper bounds are computed. Clusters are scanned in decreasing q.c_j, items in id
  order; tau is the k-th best exact score seen, starting from the seeds and rising as each item that passes all
  three levels is scored. Counts are exact: tau's steps come from a sequential pass over the items that pass
  the seed threshold.
- Bounds. **thmB**: centre-corrected estimate of (q - alpha c_j).r with alpha.e_c at every level, self-normalised
  width, R = |r| w_j; t_hi = R when t >= R, otherwise min(R, root), and the item is kept when the discriminant
  is negative (the referee's repairs). **plain**: q.c_j + f<obar,Pq> + c sd. A slack of 1e-5 is added to every
  bound for float32 rounding: items equal to their centre have width 0, and without it 92 nytimes neighbours
  at duplicate items "failed" by rounding in the first run (no wrong answers).
- Bytes per query: visible items x (n/8 + level-1 scalars) + L1 survivors x (n/8 + scalars) + L2 survivors x
  (n/4, or 3n/8 for a non-nested 4-bit code, + scalars) + full reads x 2n (f16). Scalars thmB 4 / 3 / 3 B,
  plain 3 / 1 / 1 B, as the layout section specifies; the scalars were used unquantised. Centres and the query
  rotation are not counted; the 1-bit pass that finds the seeds is counted once.
- One thread per worker on cores 10-11. `summarise.py` makes the tables (`logs/summary_*.txt`); `zcheck.py`
  recomputes extreme z values in float64; `all.sh` / `piece.sh` ran the pieces; per-query results in `out/`.

c_kL = 5.99 (n = 100) / 6.37 (n = 256): k = 10 x 3 levels. c_N = 7.48 / 8.36: exact tail, union over N x 5.

## glove-100-angular, 10,000 queries

Survivors are non-seed items passing each level; full = 30 seeds + L3 survivors. Every row: **0 results
differing from brute force, 0 true neighbours with a failed bound at any level.**

| configuration | L1 mean | p90 | L2 mean | p90 | L3 mean | p90 | full mean | p90 | MB mean | p90 |
|---|---|---|---|---|---|---|---|---|---|---|
| thmB c_kL nested | 112.0k | 265k | 3,252 | 9,289 | 103.5 | 261 | 133.5 | 291 | 21.38 | 23.96 |
| thmB c_kL independent | 112.0k | 265k | 3,252 | 9,289 | 50.1 | 119 | 80.1 | 149 | 21.41 | 24.04 |
| thmB c_kL tied4 | 112.0k | 265k | 5,903 | 16,684 | 50.1 | 119 | 80.1 | 149 | 21.44 | 24.13 |
| thmB c_N nested | 246.5k | 518k | 8,740 | 24,805 | 187.3 | 486 | 217.3 | 516 | 23.64 | 28.35 |
| thmB 9.35 nested | 462.6k | 823k | 25,088 | 69,386 | 382.6 | 1,030 | 412.6 | 1,060 | 27.48 | 34.42 |
| plain c_kL nested | 243.2k | 456k | 7,521 | 19,508 | 187.6 | 432 | 217.6 | 462 | 21.87 | 25.10 |
| plain c_N nested | 493.5k | 777k | 21,223 | 53,406 | 361.8 | 864 | 391.8 | 894 | 25.64 | 30.40 |
| plain 9.35 nested | 808.0k | 1,043k | 62,130 | 146,071 | 794.5 | 1,970 | 824.5 | 2,000 | 31.03 | 36.61 |
| 10% visible, thmB c_kL nested | 24.5k | 45.5k | 1,415 | 3,374 | 68.0 | 155 | 98.0 | 185 | 2.39 | 2.79 |
| 1% visible, thmB c_kL nested | 4,862 | 6,935 | 577 | 1,083 | 41.2 | 84 | 71.2 | 114 | 0.30 | 0.36 |

Visible items 118,359 and 11,837 on average. The seed tau equalled the true k-th score in 11% of queries.
The brute-force top 10 matches the dataset's ground truth (overlap 1.00000).

## nytimes-256-angular, 10,000 queries

Nine test queries are zero vectors: every score ties at 0, so they score all 290,000 items (correctly), adding
about 260 to every unfiltered L3 and full mean. Rows marked "w/o zero" leave them out (9,991 queries).
Every row, with or without them: **0 differing results, 0 true neighbours with a failed bound.**

| configuration | L1 mean | p90 | L2 mean | p90 | L3 mean | p90 | full mean | p90 | MB mean | p90 |
|---|---|---|---|---|---|---|---|---|---|---|
| thmB c_kL nested, all | 54.7k | 157k | 2,651 | 9,347 | 324.3 | 230 | 354.3 | 260 | 12.71 | 16.69 |
| thmB c_kL nested, w/o zero | 54.4k | 156k | 2,393 | 9,296 | 63.4 | 229 | 93.4 | 259 | 12.55 | 16.68 |
| thmB c_kL independent, w/o zero | 54.4k | 156k | 2,393 | 9,296 | 29.9 | 104 | 59.9 | 134 | 12.61 | 16.91 |
| thmB c_kL tied4, w/o zero | 54.4k | 156k | 4,142 | 15,766 | 29.9 | 104 | 59.9 | 134 | 12.65 | 17.05 |
| thmB c_N nested, w/o zero | 110.3k | 247k | 8,067 | 30,237 | 141.8 | 530 | 171.8 | 560 | 14.93 | 21.38 |
| thmB 9.35 nested, w/o zero | 136.4k | 268k | 13,307 | 48,496 | 208.0 | 794 | 238.0 | 824 | 16.23 | 23.49 |
| plain c_kL nested, w/o zero | 65.3k | 173k | 2,915 | 11,109 | 72.8 | 262 | 102.8 | 292 | 12.55 | 16.74 |
| plain c_N nested, w/o zero | 130.5k | 258k | 9,944 | 36,154 | 166.1 | 613 | 196.1 | 643 | 15.20 | 21.35 |
| plain 9.35 nested, w/o zero | 160.8k | 275k | 16,411 | 57,385 | 246.0 | 927 | 276.0 | 957 | 16.66 | 23.46 |
| 10% visible, all | 14.5k | 22.0k | 1,632 | 3,529 | 99.8 | 184 | 129.8 | 214 | 1.73 | 2.16 |
| 1% visible, all | 2,361 | 2,605 | 651 | 908 | 60.5 | 99 | 90.5 | 129 | 0.28 | 0.32 |

Visible items 29,002 and 2,901. Seed tau = true k-th score in 50% of queries. Overlap with the dataset's ground
truth is 0.985 because of the zero queries and ties among duplicate items; the cascade is checked against brute
force on the same float32 scores.

## Pass marks (L1 / L2 / L3 survivor means)

| | mark | nested (s4 = 4 s2) | independent 4-bit | tied4 (s2 = s4/4) |
|---|---|---|---|---|
| glove | <= 180k / 9k / 100 | 112k / 3.25k / **104: fail** | 112k / 3.25k / 50: pass | 112k / 5.9k / 50: pass |
| nytimes (w/o zero) | <= 70k / 5k / 50 | 54.4k / 2.39k / **63: fail** | 54.4k / 2.39k / 30: pass | 54.4k / 4.14k / 30: pass |

Exactness passes everywhere: 0 differing results and 0 neighbour bound failures in 2 x 10,000 queries x 10
configurations.

## Cost of nesting

Tying the 4-bit scale to the 2-bit one (s4 = 4 s2) gives a mean tan(phi) at 4 bits of 0.131 (glove) and 0.139
(nytimes) against 0.097 and 0.104 with the item's own 4-bit scale, and doubles L3 survivors (104 vs 50; 63 vs
30). Tying the other way (s2 = s4/4) costs at level 2 instead: tan(phi) 0.397 vs 0.360 (glove), 0.404 vs 0.365
(nytimes), and 1.7-1.8x the L2 survivors. In bytes the three layouts are within 1%, because the level-1 pass
is about 90% of the bytes (glove: 1.18M x 16.5 B = 19.5 MB of 21.4 MB). Nesting costs full reads, not bytes;
tied4 is the better nested layout.

## Each reduction's contribution (glove, nested; nytimes w/o zero queries in brackets)

| step | L1 | L2 | L3 | MB |
|---|---|---|---|---|
| plain, 9.35 (today) | 808k (161k) | 62.1k (16.4k) | 795 (246) | 31.0 (16.7) |
| exact tail: plain, c_N | 493k (131k) | 21.2k (9.9k) | 362 (166) | 25.6 (15.2) |
| union over k x L: plain, c_kL | 243k (65k) | 7.5k (2.9k) | 188 (73) | 21.9 (12.6) |
| Theorem B: thmB, c_kL | 112k (54k) | 3.25k (2.39k) | 104 (63) | 21.4 (12.6) |

Theorem B at c_kL halves glove's L1 and L2 survivors; on nytimes it gains 1.2x (w about 0.93 there). In bytes it
is worth 2% (glove) and 0% (nytimes), since level 1 reads every item regardless.

## The law of z

z = (true - estimate)/(sd |D_perp|), centre-corrected, over all items for the first 1,000 queries: 1.18e9 pairs
per code on glove, 2.89e8 on nytimes. Two-sided tail, measured against the exact sphere law:

| code | |z|>3 (law) | |z|>4 (law) | |z|>5 count (law) | max |z| |
|---|---|---|---|---|
| glove 1 bit | 2.296e-3 (2.300e-3) | 3.35e-5 (3.38e-5) | 115 (118) | 5.72 |
| glove 2 bit | 2.297e-3 | 3.37e-5 | 116 (118) | 5.84 |
| glove 2 bit tied4 | 2.296e-3 | 3.39e-5 | 117 (118) | 5.79 |
| glove 4 bit nested | 2.311e-3 | 3.43e-5 | 101 (118) | 5.51 |
| glove 4 bit independent | 2.298e-3 | 3.40e-5 | 126 (118) | 5.51 |
| nytimes 1 bit | 2.537e-3 (2.544e-3) | 4.99e-5 (5.06e-5) | 107 (91) | 5.68 |
| nytimes 2 bit | 2.542e-3 | 5.11e-5 | 109 (91) | 5.67 |
| nytimes 2 bit tied4 | 2.538e-3 | 5.02e-5 | 92 (91) | 5.71 |
| nytimes 4 bit nested | 2.529e-3 | 5.04e-5 | 75 (91) | 5.68 |
| nytimes 4 bit independent | 2.531e-3 | 5.06e-5 | 111 (91) | 5.79 |

(nytimes |z|>5 counts and maxima are the float64 recomputation in `logs/zcheck_nytimes.txt`.) The law holds at
3 and 4 sd to about 1% on every code. At 5 sd nytimes runs 18-22% above the law on three codes, about 2 Poisson
sd; pairs share items across queries, so the counts are not independent. The float32 pass on nytimes also gave
a few |z| up to 488 where |D_perp| is near zero; recomputing every pair with float32 |z| > 4.5 in float64 leaves
a maximum of 5.79, so those were rounding. Glove's nested 4-bit code runs 0.5% (at 3 sd) and 1.4% (at 4 sd)
above the law, the only code that does; small, and the counts are correlated, but the one place to look again.

## What disagrees with the design report

1. L3 survivors with nested codes are 104 (glove) and 63 (nytimes), against the report's 56-81 and 34-42, so
   the third pass mark fails with s4 = 4 s2. The report's tables used independent scales, which give 50 and 30
   here. "Nesting costs accuracy only" is right in bytes but doubles full reads; tie the 2-bit scale to the
   4-bit one instead (tied4 passes every mark).
2. Bytes: 21.4 MB on glove against the report's 16.9 + 3.6 = 20.5 MB; 12.6 MB on nytimes against 11.4 MB.
   L1 survivors are fewer than reported (112k vs 149k), but e_c at level 1 adds 1 B per item and level 1 is 90%
   of the bytes.
3. The means hide a long tail: p90 L1 survivors are 265k (glove) and 156k (nytimes), above the 180k and 70k
   marks; p90 L2 9.3k on both. The marks hold for means only. On nytimes it is the queries far from every
   centre that the report's risks section names.
4. Theorem B's gain at c_kL is 2.2x (glove) and 1.2x (nytimes) in L1 and L2 survivors, inside the report's
   1.2-2.2x.
5. Filtered: at 1% visible, level 1 passes 41% (glove) and 81% (nytimes) of visible items, so the bytes are the
   level-1 pass over the visible set; full reads stay under 100 on average.
6. The interrupted first run used one scale fitted on 20,000 rotated items, no t_hi = R cap, a clamped negative
   discriminant and no slack. On the same queries its counts were within 4% of this run (glove first 2,400:
   113.6k / 3,390 / 109 vs 113.6k / 3,312 / 105; nytimes all: identical L1 to 0.1%, L2 within 1%), with 0
   differing results. Every figure above is from the repaired run.

Structures in `/home/joe/code/vexact-data/<name>/cascade/` (about 3 GB for glove) can be deleted.
