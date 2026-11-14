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
