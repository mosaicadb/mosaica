# Threshold queries at n = 768: local-basis stream against a flat global-rotation code

2026-11-14. BioCLIP-2, 200k sample: a random 150k as the corpus, 100 held-out items as queries (from the other
50k; no text queries were available, so none were run). Answer shares 0.01 / 0.1 / 1 / 10 / 30 % of the corpus
(15 to 45,000 matches; threshold = midpoint of the k-th and (k+1)-th exact score per query). Mixed rule: at each
level certify in when the lower bound is >= t (t - d with a tolerance), exclude when the upper bound is < t, pass on
otherwise; after the last level read the rest exactly. One constant on both sides, the plain all-items one-sided
union over N = 1e9 items and five randomised levels, eps = 1e-9. Counts, not estimates; every run counts wrong
decisions against brute force. Full tables: `logs/tables.md`. Run on cores 8-9.

## Codes

- **Local basis, PCA** (`lb_run.py` = `combined/run.py` plus `lb_run.patch`): the fit is vexact-bench's
  `rust/tools/fit.py`, run unchanged (`runfit.py`): K = 150 clusters, G = 9 groups (sizes 97 to 50,910 items) with a
  PCA basis each, data-only H25 profile. `adapt.py` converts the fit to the index format of `combined/build.py`, with
  H25 in place of both profiles. The stream is the combined design's: level 0 (3 B), escape-code deterministic levels
  at 4 / 8 / 16 / 32 B, sub-bit randomised levels at 48 / 96 / 192 / 384 / 576 B over four rotated blocks, scalars
  charged (stored cumulative 3 / 8 / 13 / 22 / 39 / 65 / 117 / 217 / 413 / 609 B).
- **Local basis, geometric-mean metric** (`adapt_gm.py`): the same stream with each group's PCA basis replaced by the
  dual pair of `groups/scripts/measure.py` (research/exact-vector-search 2c528d7b0): Sigma_Q from 5,000 corpus items as
  pseudo-queries against their 4 nearest centres, one global matrix, ridge 0.1 of its mean eigenvalue; item side
  Aq v l^-1/4, query side Aq^-1 v l^1/4. `lb_run.py` takes the query-side matrix as `VQ`.
- **Flat global rotation** (`flat.py` = `thr768.py` plus `flat.patch`): one Haar rotation, K = 200 centres, Theorem
  B's interval, levels: signs of the first 256 and 512 rotated coordinates (32, 64 B; added so the flat code has
  levels at 32 and 64 B), then 1 / 2 / 4 / 6 bits (96 / 192 / 384 / 576 B). Constant 8.72 at every level.

## Undecided per true match after each level (d = 0), and exact reads per match

Local PCA, columns at stored bytes 39 / 65 / 117 / 217 / 413 / 609 (code bytes 32 / 48 / 96 / 192 / 384 / 576):

| Share | 32 | 48 | 96 | 192 | 384 | 576 = exact reads |
|---|---|---|---|---|---|---|
| 0.01% | 160 | 69.2 | 6.48 | 1.29 | 0.275 | 0.125 |
| 0.1% | 47.8 | 23.9 | 3.50 | 0.832 | 0.157 | 0.064 |
| 1% | 15.4 | 9.80 | 2.03 | 0.508 | 0.091 | 0.033 |
| 10% | 4.17 | 3.31 | 1.05 | 0.287 | 0.049 | 0.016 |
| 30% | 1.93 | 1.66 | 0.713 | 0.217 | 0.036 | 0.011 |

Local, geometric-mean metric (same columns):

| Share | 32 | 48 | 96 | 192 | 384 | 576 = exact reads |
|---|---|---|---|---|---|---|
| 0.01% | 222 | 108 | 8.89 | 1.57 | 0.335 | 0.135 |
| 0.1% | 53.2 | 30.5 | 4.44 | 1.01 | 0.184 | 0.072 |
| 1% | 14.1 | 9.76 | 2.31 | 0.599 | 0.105 | 0.037 |
| 10% | 3.55 | 2.82 | 1.01 | 0.287 | 0.049 | 0.016 |
| 30% | 1.68 | 1.42 | 0.594 | 0.169 | 0.028 | 0.009 |

Flat (code bytes, no scalars charged):

| Share | 32 | 64 | 96 | 192 | 384 | 576 = exact reads |
|---|---|---|---|---|---|---|
| 0.01% | 553 | 120 | 32.6 | 6.29 | 1.31 | 0.432 |
| 0.1% | 117 | 37.6 | 13.9 | 3.78 | 0.888 | 0.266 |
| 1% | 25.4 | 12.6 | 6.34 | 2.28 | 0.592 | 0.176 |
| 10% | 5.17 | 3.56 | 2.34 | 1.13 | 0.329 | 0.098 |
| 30% | 2.37 | 1.78 | 1.31 | 0.685 | 0.203 | 0.061 |

## Mean code bytes per item and exact reads per match, with tolerance

Each cell: code B per item / exact full-vector reads per true match. Total bytes per item = code + 1,536 x reads
x matches / N.

| Share | Code | d = 0 | 0.002 | 0.005 | 0.01 |
|---|---|---|---|---|---|
| 0.01% | local PCA | 10.2 / 0.125 | 10.2 / 0 | 10.2 / 0 | 10.2 / 0 |
| | local GM | 9.8 / 0.135 | 9.8 / 0 | 9.8 / 0 | 9.8 / 0 |
| | flat | 34.6 / 0.432 | 34.6 / 0.115 | 34.6 / 0.009 | 34.5 / 0 |
| 0.1% | local PCA | 15.3 / 0.064 | 15.2 / 0 | 15.1 / 0 | 15.0 / 0 |
| | local GM | 14.6 / 0.072 | 14.5 / 0 | 14.4 / 0 | 14.2 / 0 |
| | flat | 39.2 / 0.266 | 39.1 / 0.075 | 38.9 / 0.010 | 38.7 / 0 |
| 1% | local PCA | 29.7 / 0.033 | 29.0 / 0 | 28.5 / 0 | 27.7 / 0 |
| | local GM | 27.9 / 0.037 | 27.2 / 0 | 26.7 / 0 | 25.8 / 0 |
| | flat | 55.7 / 0.176 | 55.1 / 0.061 | 54.3 / 0.011 | 52.9 / 0 |
| 10% | local PCA | 70.6 / 0.016 | 67.7 / 0 | 65.0 / 0 | 61.5 / 0 |
| | local GM | 64.0 / 0.016 | 61.2 / 0 | 58.3 / 0 | 54.7 / 0 |
| | flat | 110.4 / 0.098 | 107.7 / 0.043 | 103.9 / 0.010 | 98.5 / 0 |
| 30% | local PCA | 109.4 / 0.011 | 105.8 / 0 | 101.8 / 0 | 96.3 / 0 |
| | local GM | 93.9 / 0.009 | 90.3 / 0 | 86.3 / 0 | 81.0 / 0 |
| | flat | 160.9 / 0.061 | 157.9 / 0.034 | 153.4 / 0.008 | 145.7 / 0 |

Exact-read bytes per item at d = 0 (1,536 x reads / N): local 0.02 / 0.10 / 0.51 / 2.5 / 5.2 B; flat 0.07 / 0.41 /
2.7 / 15.1 / 27.9 B. Per-cluster bytes (centres) add 0.77 B per item to the local stream.

Wrong decisions against brute force: **0** in every row of every code (5 shares x 4 tolerances x 100 queries x
150k items each), and 0 interval violations at every level of the local stream (largest randomised error 5.9 sd).

## What the numbers say

- **The local-basis stream wins at every share and every byte budget.** At equal stored bytes it leaves 3-5x fewer
  undecided per match at 96-576 B for small answers and 2-6x fewer for large ones, needs 3.5-5.5x fewer exact
  reads per match (0.125 against 0.43 at 0.01%, 0.011 against 0.061 at 30%), and reads 1.5-3.4x fewer code bytes
  per item. The flat code is closest at large answers and early levels (30%, 32 B: 1.9 against 2.4).
- **Code bytes, not exact reads, are the cost.** At d = 0 exact reads are 0.1-5% of the local stream's bytes and
  1-15% of the flat code's. A tolerance removes the exact reads (local: all of them from d = 0.002; flat: at
  d = 0.01) but cuts code bytes only 0-12%, and by least for small answers, where the bulk sits far below t. This
  agrees with groups/REPORT.md 3.5-3.6: per-match exact reads concentrate at t, the code bytes in the bulk.
- **Geometric-mean metric:** fewer undecided through 16 B, more from 32 B to 576 B (the randomised levels were
  designed for the PCA profile), net code bytes 4-14% lower, rising with the share; exact reads up 0-15% at small
  shares. Worth carrying into the kernel; the randomised levels' design should be refitted for it.

## What was simplified

- Not the Rust binary: it does not report per-level counts per match or tolerances, its answer shares are fixed
  at 0.01-5%, and its source could not be edited. The numpy stream uses the combined design's levels (4 / 8 / 16 /
  32 deterministic, five randomised), not v2's (6 deterministic to 32 B, 14 randomised to 256 B).
- Local constant: an exact Chernoff bound over the four rotated blocks at a one-sided tail of eps / (N x 5) per
  event, the stream's analogue of 8.72 (sub-Gaussian equivalent 9.28); `run.py` used the two-sided tail.
- No cluster cut, support table or sorted range in either code; the "annulus" rows of `logs/tables.md` add the
  bound from the stored a (C1) to level 0, a 15-20% cut at small shares.
- Flat: quantiser scale fitted globally (unproved under the referee's ruling), the 2 / 4 / 6-bit codes are separate
  quantisations charged as increments, no scalars charged (a few B per level read).
- 150k items, so K = 150 and G = 9; one group has 97 items. Shares of a 150k sample, with constants as at 1e9.

## Files

`split.py`, `runfit.py`, `shim.py` (scipy.special.gammaln stand-in), `adapt.py`, `adapt_gm.py`, `lb_run.py` +
`lb_run.patch`, `flat.py` + `flat.patch` (against `thr768_orig.py`, with `consts2.py`, `consts3.py`), `tables.py`,
`logs/`. Large files in the session scratchpad (`thr/`).

## Text queries (2026-11-14, second run)

The same run with real BioCLIP-2 text embeddings as the queries. Corpus, fit (`fit/`, data-only), `adapt.py`,
`adapt_gm.py`, `lb_run.py`, `flat.py` and `tables.py` are unchanged; only `queries.npy` and `thr.npy` differ
(`text_split.py`, split.py's threshold rule). No text embedding enters any fit. Driver `run_text.sh`, query sets
`text_sets.py`, side-by-side tables `text_compare.py` (full output `logs/text_compare.md`, per-set tables
`logs/text_*/tables.md`). Cores 8-9.

**Embeddings** (`../textq/`): `fetch_text_tower.py` (vexact-bench agent2, unchanged) range-reads the 495 MB text tower;
`encode.py` encodes the 369 test prompts of agent2's `text_q.json` and the 2,206 training prompts of `text_train.json`
(encoded, not used here) with open_clip 4.2.2 on CPU torch 2.14.1. Data in `vexact-data/bioclip2-text/`.

**The prompts were built for a different sample.** agent2 built them from files 38 / 88 / 246 / 444 of
TreeOfLife-200M-Embeddings; `bioclip2-200k` is files 463 / 573 / 476 / 462 (`fetch_meta200k.py` re-read those row groups:
emb equals g0-g3 row for row, 952 species, g0 is 50,000 images of one species). **None of the 239 species-labelled
prompts names a species in this sample**, and 1 of the 60 higher-taxon labels occurs in it, so the label check over
the test prompts is empty and every "names" query below is a species absent from the corpus. Encoder check instead
(`check_insample.py`): "a photo of <Genus species>" for the 361 species with >= 20 images in the sample: the nearest
image carries the label for **79.8%**, 67.7% of the 10 nearest do (56% / 89% / 99% for species with < 100 / 100-999 /
>= 1,000 images). A seeded 100 of these prompts are run as an extra set, "in-sample species".

Mean thresholds at 0.01 / 0.1 / 1 / 10 / 30 %: items 0.910 / 0.886 / 0.850 / 0.778 / 0.694; all text 0.617 / 0.592 /
0.566 / 0.535 / 0.514; free text 0.488 / 0.466 / 0.442 / 0.415 / 0.397; in-sample species 0.768 / 0.742 / 0.706 / 0.639 /
0.553.

### Code bytes per item / exact reads per match (d = 0), and the flat-to-local-PCA ratios

| Share | Queries | local PCA | local GM | flat | flat / PCA code B | flat / PCA reads |
|---|---|---|---|---|---|---|
| 0.01% | items (100) | 10.2 / 0.125 | 9.8 / 0.135 | 34.6 / 0.432 | 3.39 | 3.46 |
| | text, all (369) | 78.1 / 0.330 | 80.9 / 0.377 | 79.5 / 0.890 | 1.02 | 2.70 |
| | in-sample species (100) | 25.4 / 0.193 | 24.9 / 0.222 | 44.0 / 0.740 | 1.73 | 3.83 |
| 0.1% | items | 15.3 / 0.064 | 14.6 / 0.072 | 39.2 / 0.266 | 2.56 | 4.16 |
| | text, all | 101.0 / 0.214 | 103.3 / 0.248 | 111.2 / 0.695 | 1.10 | 3.25 |
| | in-sample species | 34.0 / 0.121 | 32.9 / 0.144 | 53.0 / 0.534 | 1.56 | 4.41 |
| 1% | items | 29.7 / 0.033 | 27.9 / 0.037 | 55.7 / 0.176 | 1.88 | 5.33 |
| | text, all | 139.0 / 0.161 | 140.6 / 0.187 | 171.0 / 0.586 | 1.23 | 3.64 |
| | in-sample species | 53.0 / 0.073 | 50.7 / 0.087 | 75.8 / 0.352 | 1.43 | 4.82 |
| 10% | items | 70.6 / 0.016 | 64.0 / 0.016 | 110.4 / 0.098 | 1.56 | 6.12 |
| | text, all | 217.1 / 0.100 | 218.3 / 0.115 | 285.1 / 0.375 | 1.31 | 3.75 |
| | in-sample species | 100.9 / 0.029 | 96.4 / 0.033 | 137.3 / 0.147 | 1.36 | 5.07 |
| 30% | items | 109.4 / 0.011 | 93.9 / 0.009 | 160.9 / 0.061 | 1.47 | 5.55 |
| | text, all | 271.3 / 0.060 | 272.4 / 0.068 | 358.4 / 0.230 | 1.32 | 3.83 |
| | in-sample species | 162.5 / 0.019 | 153.6 / 0.020 | 220.2 / 0.087 | 1.36 | 4.58 |

By kind (flat / local PCA code bytes at 0.01 / 0.1 / 1 / 10 / 30 %): names (259, species not in the corpus) 1.03 /
1.10 / 1.23 / 1.33 / 1.35; higher taxa (60) 1.01 / 1.11 / 1.24 / 1.33 / 1.33; free descriptions (50) 0.99 / 1.09 /
1.20 / 1.23 / 1.21. Exact-read ratios 2.4-4.1 in every kind. Local PCA code bytes, names / higher / free at 0.01%:
73.0 / 79.7 / 102.9; at 30%: 260 / 277 / 322.

A tolerance removes the exact reads more slowly than for items: the local stream leaves 0.07-0.20 per match at
d = 0.002 for 0.01% answers of out-of-corpus text (items: 0), at most 0.005 at d = 0.005 and none at d = 0.01; the
in-sample species behave like items. Flat leaves 0.001-0.035 per match at d = 0.01. Code bytes fall 0-13%, as for items.

### Undecided per true match after each level (d = 0), text against items

Code-byte columns (stored bytes for the local stream as in the first section). Exact reads = the last column.

| Share | Queries | PCA 32 | 96 | 192 | 384 | reads | GM 96 | 192 | reads | flat 32 | 96 | 192 | 384 | reads |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0.01% | items | 160 | 6.48 | 1.29 | 0.275 | 0.125 | 8.89 | 1.57 | 0.135 | 553 | 32.6 | 6.29 | 1.31 | 0.432 |
| | text, all | 6,170 | 305 | 15.7 | 1.08 | 0.330 | 345 | 16.1 | 0.377 | 7,240 | 1,120 | 72.1 | 3.67 | 0.890 |
| | in-sample sp. | 1,250 | 41.3 | 4.35 | 0.560 | 0.193 | 46.5 | 4.71 | 0.222 | 2,090 | 232 | 33.8 | 2.81 | 0.740 |
| 0.1% | items | 47.8 | 3.50 | 0.832 | 0.157 | 0.064 | 4.44 | 1.01 | 0.072 | 117 | 13.9 | 3.78 | 0.888 | 0.266 |
| | text, all | 770 | 91.1 | 8.07 | 0.746 | 0.214 | 98.1 | 8.08 | 0.248 | 854 | 270 | 35.5 | 2.80 | 0.695 |
| | in-sample sp. | 194 | 13.5 | 2.71 | 0.393 | 0.121 | 14.8 | 2.86 | 0.144 | 280 | 54.9 | 12.5 | 2.01 | 0.534 |
| 1% | items | 15.4 | 2.03 | 0.508 | 0.091 | 0.033 | 2.31 | 0.599 | 0.037 | 25.4 | 6.34 | 2.28 | 0.592 | 0.176 |
| | text, all | 89.9 | 25.1 | 4.49 | 0.570 | 0.161 | 25.9 | 4.46 | 0.187 | 94.6 | 52.5 | 15.0 | 2.18 | 0.586 |
| | in-sample sp. | 31.9 | 4.95 | 1.43 | 0.245 | 0.073 | 5.18 | 1.49 | 0.087 | 41.1 | 12.6 | 4.80 | 1.20 | 0.352 |
| 10% | items | 4.17 | 1.05 | 0.287 | 0.049 | 0.016 | 1.01 | 0.287 | 0.016 | 5.17 | 2.34 | 1.13 | 0.329 | 0.098 |
| | text, all | 9.82 | 5.64 | 2.05 | 0.347 | 0.100 | 5.68 | 2.02 | 0.115 | 9.94 | 8.18 | 4.58 | 1.28 | 0.375 |
| | in-sample sp. | 5.76 | 1.68 | 0.569 | 0.097 | 0.029 | 1.65 | 0.569 | 0.033 | 6.65 | 3.05 | 1.60 | 0.492 | 0.147 |
| 30% | items | 1.93 | 0.713 | 0.217 | 0.036 | 0.011 | 0.594 | 0.169 | 0.009 | 2.37 | 1.31 | 0.685 | 0.203 | 0.061 |
| | text, all | 3.33 | 2.53 | 1.14 | 0.207 | 0.060 | 2.53 | 1.12 | 0.068 | 3.33 | 3.13 | 2.23 | 0.764 | 0.230 |
| | in-sample sp. | 2.90 | 1.16 | 0.384 | 0.064 | 0.019 | 1.04 | 0.338 | 0.020 | 3.14 | 2.00 | 1.01 | 0.292 | 0.087 |

Wrong decisions against brute force: **0** in every row of every code and set (5 shares x 4 tolerances, 369 + 259 +
60 + 50 + 100 queries), and 0 interval violations at every level of the local stream.

### What the text numbers say

- **Local PCA still wins, by much less in code bytes.** Over all 369 text queries flat needs 1.02-1.32x the code
  bytes of local PCA (items 1.47-3.39x), level with it at 0.01%; exact reads stay 2.7-3.8x lower with local PCA
  (items 3.5-6.1x). The 0.01% row is a tie in code bytes (78.1 against 79.5 B). Species actually in the corpus sit between: 1.36-1.73x code bytes, 3.8-5.1x reads.
- **Text queries cost 2.5-7.7x the code bytes of item queries** with local PCA (78 against 10 B at 0.01%, 271 against
  109 B at 30%), 2.2-3.1x with flat. At 32 B the local stream leaves 6,170 undecided per match at 0.01%, 62% of the
  corpus: the first 32 B decide almost nothing for an out-of-corpus text query. Free descriptions are worst
  (103 B at 0.01%), in-sample species best (25 B).
- **The geometric-mean metric loses for text:** 0.4-3.6% more code bytes and 13-16% more exact reads than PCA across
  the 369 queries (items: 4-14% fewer code bytes). For in-sample species it saves 2-5% of code bytes and costs 7-21%
  more exact reads. Its Sigma_Q
  is fitted on corpus items as pseudo-queries, so it does not describe text.
- Caveat: no test prompt's species is in this corpus, and the in-sample set is 100 queries of one prompt form.
  The flat code carries no scalar charge, the local stream does (0.77 B per item per cluster not included in either).
