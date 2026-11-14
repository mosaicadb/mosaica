# The randomised levels' constants: certified worst case, harmful side, the index's own N, the tan levels

Branch `research/constants`, on `research/rand-bounds` at d443f66e7 plus `reports/bounds3` (the maths report and,
in `reports/bounds3/referee/`, the referee's scripts and verdicts). It builds the owner's decisions on that report:
the harmful side only for threshold and k-NN queries, with the k-NN loop changed so that this is sound, and N taken
from the index's own size. The referee found nothing unsound; its conditions are built in (below).

Scripts: `results/const/check.sh` (50k correctness), `run.sh` (1M timing), `cmp.py` (the table). The indexes are the
same 50k and 1M BioCLIP dual indexes as before (`vexact-rand-data/c50k_r4`, `c1m_r4`), copied with their own
`meta.json` to `vexact-const-data/` so that certificates could be added.

## What each step changed

1. **Constants (781d33b94).** For a level with several coded blocks the constant is the smallest of three valid
   bounds:
   - a certificate of the exact worst case over block shares (R3);
   - the convex-order closed form (R4), in `tail.rs` with one-dimensional numerics;
   - today's Chernoff value.

   `tools/certify.py` holds cert2.py's grid certificate for the index's block layout at one ln delta, and writes the
   certified X to `meta.json` (`CERTS`). `fit.py` runs it at the end of a fit, within `--cert-time` (600 s by
   default). If it cannot certify in that time, the build falls back to R4 and Chernoff. A certificate is used at
   any budget at least as loose as its own, and for any subset of its blocks; certificates are never interpolated
   in ln delta. Single-block levels keep the exact tail. Tests check the section 3 numbers: 8.885 / 8.815 (R4,
   two-sided and one-sided), 9.086 / 9.018 (Chernoff), 7.373 / 6.927 (one block). The certifier reproduces 8.704
   (one-sided, N = 1e9) and finds 8.775 two-sided, against the report's 8.774. The difference is its starting point,
   which is the one-sphere prediction plus 0.008, 0.001 higher.
2. **Harmful side only (bcfd84462).** delta = eps / (N L) per (item, level) for threshold and k-NN queries. The
   k-NN selection is now `select_knn`:
   - a candidate is dropped when at least k other candidates have a lower bound above its upper bound;
   - it is certified when at most k − 1 others have an upper bound at or above its lower bound;
   - an exact score is stored unclamped.

   Its test transcribes knn_onesided.py. With only the harmful side valid it gives 0 wrong of 4,000, and 0 of
   400,000 in a one-off run. With the old counts temporarily restored the same test gives 589 wrong of 4,000; the
   Python gives 554.
3. **N from the index (7795133b4).** N is the number of rows rounded up to a power of two, over all rows and not
   any viewer's count; `--nguar N` overrides it. eps comes from meta `EPS` (fit.py writes 1e-9). The power is
   `Meta2::npow`, and every `ThrOut2` and `KnnOut2` carries it.
   - **No result cache.** The prototype has none. A server that caches results must drop them when npow rises; a
     fall is safe.
   - **The doc comment on npow** states the referee's other conditions: no mixed constants inside one result, and
     a one-sided result is valid only for its own (query, threshold). Reusing intervals for another threshold or
     for k-NN needs the two-sided budget.
   - **Entities.** The build refuses two rows of one entity, because k-NN counts rows.
   - **Certifier default.** `certify.py` now certifies at this budget by default; each power needs its own
     certificate.
   - **The rate design** still uses the two-sided N = 1e9 constant from meta `CSG`, so the stored code is unchanged.
4. **The stored scale (695a4480b).** The source is the 15-value tan(phi) table. Measured at the encoder (50k, every
   item, stored over true):

   | | tan² | \|y\|² |
   |---|---|---|
   | block 0 | +6.5–8.8% | +0.4–0.5% |
   | block 1 | +2.8–6.2% | +0.4–0.5% |
   | blocks 2 and 3 | +1.9–5.2% | +0.4–0.5% |

   The values were equal-mass quantiles. They are now the round-up quantiser with the least mean square ratio of
   stored to true value, fitted by dynamic programming over 1,024 candidates, with the top still at the 0.998
   quantile. Escapes are therefore unchanged. Same bits, same decode, no code bytes. tan² excess becomes:

   | | before | after |
   |---|---|---|
   | block 0 | +6.5–8.8% | +4.2–5.0% |
   | block 1 | +2.8–6.2% | +2.0–4.1% |
   | blocks 2 and 3 | +1.9–5.2% | +1.4–3.6% |

   Soundness does not depend on the fit, because the encoder still rounds up against the stored table. The build
   now asserts, for every item, level and block, that the decoded \|y\| and tan(phi) are at least the true values.

   - **Not built: an 8-bit tan for blocks 0 and 1.** It would remove most of the remaining excess. Each rise of
     those blocks needs one more 4-bit scalar row: +0.5 B at 56 B and +1 B at 96, 160 and 256 B, which is 1–2.5%
     of each level's bytes. Against that it would save about 1% of the sd at the first three randomised levels,
     roughly 0.07 in c, or about 0.8% of full reads. That is not worth the bytes.
   - **Not from the tan code.** The rest of the 2.45% comes from \|y\| (0.2% of sd) and from bounding
     \|Delta_perp\| by \|Delta\| (about 1 / (2 nb), 0.5% for block 0), which no stored code can fix.

Not done: R5 (per-block constants), the eps split, Theorem B.

## Constants used per level

| | 56 B (block 0) | 96 / 160 / 256 B, blocks 0–2 (1M only) | 96 / 160 / 256 B, all four |
|---|---|---|---|
| start, two-sided, N = 1e9 | 7.413 | 8.933 (Chernoff) | 9.086 (Chernoff) |
| step 1, two-sided, N = 1e9 | 7.413 | 8.748 (R4) | 8.775 (certificate) |
| step 2, one-sided, N = 1e9 | 7.373 | ≤ 8.704 (certificate) | 8.704 (certificate) |
| step 3, 50k: N = 2^16 | 6.724 | — | 7.639 (certificate) |
| step 3, 1M: N = 2^20 | 6.931 | 7.963 (certificate) | 7.963 (certificate) |

Step 4 does not change the constants. At 1M the three-block set takes the four-block certificate, which covers its
subsets. A certificate for the three blocks alone would be lower: the report's exact value is 8.568 one-sided at
1e9. It is not built, because the design certifies one layout per index.

## Correctness after every step (50k, 200 item + 200 text queries, 7 answer sizes, filtered sets, three plans)

Every step gives:

- wrong in / out 0 / 0;
- deterministic and randomised interval violations 0 / 0, over 42.2–44.1M checks;
- deterministic decisions not implied: 0, of 10.6M;
- k-NN: 400 / 400 exact;
- float64 oracle: 0 entities wrong, k-NN 0 of 80 differ.

The largest randomised error stays at 5.47 sd. At the final 50k constant of 7.64 that leaves 2.17 sd of margin.

Items entering each level against the start (50k verify counts, both plans):

| Step | 160 B | 256 B | full vector |
|---|---|---|---|
| 1 certified | −2.9% | −3.2% | −3.3% |
| 2 one-sided | −3.5% | −4.0% | −4.1% |
| 3 own N (2^16) | −13.6% | −15.2% | −15.6% |
| 4 tan levels | −14.1% | −15.9% | −16.2% |

These are the report's predictions: rows C, D and N, −3.4%, −4.1% and −12.3% of exact reads.

## Before and after at 1M

The cores were shared: timing pinned to core 7, the process on cores 6–7, another agent's benchmark on 8–11. Each
binary was run as three separate processes, interleaved round by round. The first round is dropped: its runs were
up to 50% slow while other jobs ran. The table gives the mean of the remaining rounds' per-cell medians (2 runs each
for start / 1 / 2, 3 for steps 3 and 4). Run-to-run noise is about ±5%, so only bytes and full share are exact.

| Answer | item ms start | 1 | 2 | 3 | 4 (final) | item B start → final | item full | text ms start | 1 | 2 | 3 | 4 (final) | text B start → final | text full |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0.01% | 2.36 | +7% | −3% | +7% | +5% | 2.9 → 2.9 | 0.11 → 0.11% | 19.2 | −1% | −2% | 0% | +5% | 20.8 → 20.6 | 0.55 → 0.55% |
| 0.1% | 3.31 | +3% | −4% | −2% | +6% | 5.9 → 5.8 | 0.24 → 0.24% | 23.2 | +3% | −2% | +1% | +2% | 26.5 → 26.1 | 0.41 → 0.40% |
| 1% | 7.97 | +1% | −6% | −6% | +1% | 19.1 → 18.6 | 0.73 → 0.71% | 38.5 | +2% | −2% | +1% | +3% | 69.9 → 67.0 | 0.87 → 0.78% |
| 10% | 53.6 | 0% | −5% | −2% | −1% | 142.5 → 133.2 | 3.24 → 2.85% | 61.0 | −1% | −4% | −4% | 0% | 199.1 → 184.5 | 5.46 → 4.80% |
| 50% | 74.9 | −1% | −4% | −4% | −3% | 301.1 → 276.2 | 9.63 → 8.43% | 80.0 | +1% | −4% | −4% | +1% | 352.6 → 322.8 | 11.86 → 10.38% |
| 75% | 69.8 | +1% | −2% | −2% | −1% | 259.5 → 238.9 | 7.80 → 6.82% | 75.5 | +1% | −3% | −2% | 0% | 307.5 → 282.2 | 9.71 → 8.49% |

Bytes are per item of N and include full vectors. "Full" is the share of items whose full vector is read.

### By step at 1M

| Step | bytes (10–75%) | full share | time |
|---|---|---|---|
| 1 | −1.5 to −2% | −3% | within noise |
| 2 | −0.3 to −0.5% | −0.7% | within noise |
| 3 | −4 to −5.5% | −8 to −9% | −2 to −4% at 10–75% (from step 2 to step 3 the two are level) |
| 4 | −0.4 to −0.5% | −0.7% | no gain measured |

- **Overall**, start to final: bytes −6.5 to −8.5% and full vectors −12 to −13% at answers of 10–75%; at 0.01–1%,
  bytes 0 to −4%.
- **Time** at 10–75% is −1 to −3% for item queries and 0 to +1% for text, within the ±5% noise. Step 3's reduction
  should show up in time, as Part 2 of REPORT_BOUNDS did (2.6% of full reads gave 1–2% of time). These runs cannot
  resolve it. A quiet machine is needed to measure the time gain.
- **Step 4 shows no time gain.** Its query code is the same as step 3's, apart from the table values, so its small
  apparent slowdown (+1 to +5%, also seen in reverse order in an extra pair of runs) is noise or code layout.

## What is on by default

- Constants: the smallest of certificate, R4 and Chernoff for several blocks; the exact tail for one.
- One-sided budget, eps / (N L), with N the row count rounded up to a power of two, for threshold and k-NN queries.
- The changed k-NN counts (`select_knn`); one row per entity enforced at build.
- The optimal tan levels, and the decoded-scale check at build.
- `fit.py` writes `EPS` and a certificate (`--cert-time`). An index without one uses R4 and Chernoff.

Flags: `--nguar N` puts N in place of the power of two. `--csg-plain` gives the sub-Gaussian constant at the same
budget.
