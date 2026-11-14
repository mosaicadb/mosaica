# The randomised levels' bounds: cheaper to evaluate, and tighter

Branch `research/rand-bounds`, rebased onto `research/rand-levels` at b429c5352 (the review fixes). Measured on the
Ryzen 9 5900X (WSL2, AVX2), one thread pinned to core 10, builds on cores 0–2, fixed plan constants
(`results/bounds/plan_fixed.txt`), s3 configuration (`--int --gtab --flags repack`), the 1M BioCLIP data-only dual
index with four randomised levels (`vexact-rand-data/c1m_r4`, the same as `c1m/dual --rlv 56,96,160,256`). Scripts:
`results/bounds/run.sh`, `e2e.sh`, `check.sh`, `cmp.py`; studies in `results/bounds/study/`.

## Summary

Threshold queries are 4–5% faster at answers of 10–75% and 2–4% faster at 0.01–1%, with 1–2% fewer bytes and 2–3%
fewer items reaching their full vectors. Two changes are on by default:

- **Cheaper evaluation (Part 1):** the scalars the randomised levels decode are cheaper, and constant terms are
  computed once. This takes 2.5–3.5 ms out of the bounds at large answers.
- **A tighter constant (Part 2):** each (group, level) now uses a constant from the exact law of the estimator's
  error instead of the sub-Gaussian c = 9.33. It is 7.41 where one block is coded (the first randomised level) and
  9.09 where four are. The union bound is unchanged.

The other candidates were measured and left off. None of the bound refinements studied would remove more than about
3% of full-vector reads, and the quantiser is already at the Lloyd–Max optimum for its cells. A deeper last level
cuts bytes by 16–27% but costs 1–26% in time, because a level's per-cluster cost is larger than the full vectors it
saves. Only the caller's tolerance d moves the full-vector share a lot: d = 0.01 halves the bytes and saves 17–19% at
large answers.

## Part 1: cheaper evaluation, same bounds (commit ffb96672c)

- **Constant terms once.** At the first randomised level, two values are formed per item and stored in place of s,
  mr and adet. They are the estimate's constant c0 = qm + α·mr + s·adet and the half-width's fixed part
  h0 = fixed + s·(edet + ewdet) + 16u·(|qm + α·mr| + |s·adet|). Later levels load the two values; s, mr and adet
  are no longer compacted after the deterministic levels.
- **The variance term at the rise.** Each block's (|y|·tan·(1 + 1e-6)·nd)² / (nb − 1) is formed when its kappa and
  tan are decoded, and stored in place of |y|·tan. A level then adds one value per coded block. The kappa rounding
  bound and the estimate's 16u share of |kappa·aj| are one product, (dmx_j + 16u)·|kappa·aj|.
- **Scalars by permutes, not gathers.** The breakdown showed that the scalar decode took 12.2–13.1 ms of 23 ms. Most
  of it was AVX2 gathers from the 256-entry |y| and kappa tables. Each table entry is now defined as the f32 product
  of two 16-entry tables, one indexed by each nibble of the code: ytab[c] = yhl[1][c >> 4] · yhl[0][c & 15]. The
  encoder chooses codes against these same values, so decoding is two permutes and a multiply. 4-bit kappa codes use
  a single permute of the table's first 16 entries. The scalar decode falls from 12.7 to 9.9 ms at item 50%, and
  from 5.1 to 3.8 ms at text 1%.
- Kappa·aj is still formed once per block per level and is not stored. On four levels every coded block rises at
  almost every level, so a stored product would have to be recomputed at each rise.

Measured against the rebased start: −3% at item 10–75%, −4% at text 1–75%, and −1 to −2% elsewhere. Bytes are
identical. The bounds and decisions phase falls from 23.1 to 20.2 ms at item 50%, and from 9.0 to 7.7 ms at text 1%
(`bd_base`, `bd_p1b`).

**Slack.** The estimate is computed with the same f32 operations in the same order as before, so it is bit for bit
the same. The half-width is a sum of non-negative terms. Its relative rounding error is at most the largest number
of roundings that any one term passes through:

- **The constant part:** 2 roundings inside h0, then up to 8 accumulations (two per coded block), the final fused
  multiply-add and the multiply by 1 + 1e-6. That is about 12u.
- **The random term:** about 6u inside each variance term, 3u for their sum, half of that through the square root,
  then the constant's rounding and the last two operations. That is about 8.5u.

Both are below the factor's 16u ((1.0 + 1e-6) as f32 is exactly 1 + 16u). The old order had about 13u on its longest chain, so the new one is no worse, and the
slacks are unchanged. The new constants carry their own 1 + 1e-6.

## Part 2: tighter bounds

### What the bound is made of

A study dump of every randomised interval was taken from a 50k verify run of 10 item and 10 text queries, 9.7M
records (`VEXACT_DUMP`, `study/bounds_study.py`). It gives the mean half-width at each randomised level and what
makes it up:

| Level | half-width | random term | Cauchy–Schwarz on uncoded blocks | other deterministic | variance share by block |
|---|---|---|---|---|---|
| 56 B | 0.080 | 0.018 | 0.062 | 0.0000 | 1 / 0 / 0 / 0 |
| 96 B | 0.037 | 0.037 | 0 | 0.0001 | .25 / .41 / .32 / .01 |
| 160 B | 0.016 | 0.016 | 0 | 0.0001 | .23 / .28 / .45 / .04 |
| 256 B | 0.0067 | 0.0066 | 0 | 0.0001 | .08 / .14 / .65 / .13 |

At the first randomised level, the deterministic bound on the three uncoded blocks dominates. From 96 B on, the
random term is all of the half-width. At 256 B it comes mostly from block 2 (256 positions at 2 bits).

### Each candidate, simulated on the dump (items entering 96 / 160 / 256 B, and reaching full vectors)

Every alternative below is at most as wide as the current bound, so replaying the dump's decisions with it is exact.

| Bound | item: enter 96 / 160 / 256 B, full | text: enter 96 / 160 / 256 B, full |
|---|---|---|
| current (c = 9.33) | 1,229,526 / 696,541 / 313,978, 121,750 | 1,944,345 / 1,171,822 / 564,035, 242,047 |
| exact tail where one block is coded | −3.6% / 0 / 0, 0 | −2.3% / 0 / 0, 0 |
| one-sided union, c = 9.256 | −0.1% / −0.7% / −0.8%, −0.8% | −0.1% / −0.6% / −0.7%, −0.8% |
| Theorem B, self-normalised (one-sided c) | −1.5% / −4.4% / −1.5%, −0.9% | −0.9% / −4.0% / −1.4%, −0.8% |
| Chernoff, exact moment generating function per item | random term −5.3 / −4.9 / −4.3% at 96 / 160 / 256 B | (same) |

- **The exact tail.** A coordinate U of a uniform unit vector in R^d has P[U ≥ s] = ½·I_{1−s²}((d−1)/2, ½). At the
  same per-side probability, the constant per block falls by a factor of 0.80 (d = 95), 0.83 (127), 0.89 (255) and
  0.90 (287).
- **Theorem B.** Its gain is small because κ² = c²/Σ_j (d_j / tan²_j) is 0.09 at 96 B and below 0.001 at 256 B.
  The precise blocks pin the sum, so the Lagrange bound on Σ_j T_j gains almost nothing. It would also need a
  per-item W in the state, so it is not built.
- **The one-sided union** is valid for threshold decisions: an item's wrong decision needs the deviation on the side
  its true score sets. It would double k-NN's failure bound, which uses both ends of the same interval, for 0.8%.
  Not built.

### Built: the constant per (group, level) from the exact law (commit c25333fa9, on by default)

The per-item Chernoff bound needs four table reads per item per level. Its gain has almost no spread across items
(5% ± 1%). So the bound is taken over all block shares instead, which gives one constant per set of coded blocks at
no per-item cost (`src/v2/tail.rs`).

- **One coded block:** c = s*·√d, where P[U ≥ s*] = e^{−c²/2}, the per-side probability of the bound it replaces.
  At nb = 96 this gives c = 7.413, the scipy value to 4 digits.
- **Several coded blocks:** with shares w_j = a_j²/(d_j σ²), P[X ≥ h] ≤ exp(−λh + Σ_j ψ_{d_j}(λ a_j)). Here ψ_d is
  the exact log moment generating function, tabulated by Simpson's rule and read by linear interpolation, which is
  an upper bound because ψ is convex. λ may depend on the a_j, because the law of u is the same given A. The
  constant must bound max_w min_λ, and that is at most min_λ max_w. For a fixed λ the sum over blocks is separable,
  so each share is rounded up to a multiple of 1/1000 (ψ is increasing) and the maximum is found by dynamic
  programming over the blocks.
- **The result** is c = 9.086 for the four 1M blocks, against 9.073 for max_w min_λ on a grid. A test checks the
  single-block value against the incomplete beta function, and ψ against the sub-Gaussian bound.

The proof is the existing one with the moment generating function evaluated exactly instead of bounded, so no new
argument is needed. `--csg-plain` restores c everywhere.

At 50k, verify counts: items entering 96 B fall 2.7%, 160 B 2.2%, 256 B 2.5%, full vectors 2.6%. At 1M, against
Part 1 alone: −2% at item 10–75% and item 0.01–0.1%, −1% at text (`e_p1c*` against `e_p2a*`).

`--design-exact` (off) also feeds the constants into the rate design. It then codes three blocks at 56 B instead of
one, and items entering 96 B fall 2%. But it is 8–24% slower at 1M, because the first level decodes more scalars.

### The quantiser

The randomised code is a nested uniform binary expansion with a scale chosen per item from six values.
`study/quant_study.py` searched the ratio of successive deltas at every depth on Gaussian blocks of 128, for the
schedules in use (rises at 1–2, 2–3, 2–3–6, 1–4, 2–4 and 3–4–7 bits). Halving is optimal in every case. At one rise
of 2 bits the code's tan² is 0.133, which is the Lloyd–Max value for a Gaussian. The loss against one-rate codes,
for example 0.179 against 0.137 at 2 bits when the same block must also serve 3 and 6 bits, is the price of nesting
under a single scale. Unequal deltas do not recover it. Nothing to build.

### The last level's depth (1M, against the final default)

| Schedule | bytes per item | full share | time |
|---|---|---|---|
| 56 / 96 / 160 / 256 (default) | 301 (item 50%), 353 (text 50%) | 9.6%, 11.9% | — |
| 56 / 96 / 160 / 320 | −16%, −15% | 5.5%, 7.3% | +4 to +26% (item 0.01% is +26%) |
| 56 / 96 / 160 / 256 / 384 | −27%, −26% | 3.6%, 4.9% | +1 to +9% |

The deeper levels cut full-vector reads by 42–63%, but every answer size is slower. A level costs a table per
(cluster, level) and a decode and bound per survivor. At the deep end, about 10 survivors per cluster share each
table, so the per-entrant cost is close to the 104 ns of a full vector. This is the schedule for a disk-resident
store, where bytes are the cost. `bail=8` or `bail=24` (sending a cluster's last survivors to full vectors instead of
building a table) was 0–7% faster than five levels without bail, measured within one process, which is still
slower than four levels.

### The tolerance d (the caller's parameter; report only, default exact)

`tol=D` (plan flag, off) takes an item in once its lower bound reaches t − D. The out-decision stays at t, so
R ⊆ S(t − D). At 50k, 10 + 10 queries:

| d | full-vector reads | wrong-in against S(t) |
|---|---|---|
| 0 | 709k | 0 |
| 0.001 | −9% | 0 |
| 0.003 | −26% | 4,108 |
| 0.01 | −78% | 276,798 |

At 1M:

| d | item 50% | text 50% | item 75% | text 1% |
|---|---|---|---|---|
| 0.001 | −2%, bytes −6%, full 9.6 → 8.5% | −0%, bytes −5% | −2% | −2% |
| 0.003 | −8%, bytes −19%, full → 6.2% | −5%, bytes −15% | −6% | +8% (noise at 40 ms) |
| 0.01 | −19%, bytes −50%, full → 1.1% | −17%, bytes −45%, full → 2.7% | −19% | −3% |

Small answers barely move, because their cost is in items far below the threshold, not near it.

## Correctness after each step (50k, 200 item and 200 text queries, 7 answer sizes, filtered sets, k-NN, float64 oracle; `check.sh`)

| Step | Wrong in / out | det / rand violations | Decisions not implied | k-NN | Oracle |
|---|---|---|---|---|---|
| Start (b429c5352, earlier run on the same index) | 0 / 0 | 0 / 0 | 0 | 400 / 400 | 0 wrong |
| Part 1, constants once and variance at the rise | 0 / 0 | 0 / 0 (44.8M checks) | 0 of 10.6M | 400 / 400 | 0 wrong |
| Part 1, permute decode and the tol flag | 0 / 0 | 0 / 0 | 0 | 400 / 400 | 0 wrong |
| Constants per (group, level) | 0 / 0 | 0 / 0 (44.1M checks) | 0 | 400 / 400 | 0 wrong |
| The same with `--design-exact` | 0 / 0 | 0 / 0 | 0 | 400 / 400 | 0 wrong |
| Final (the tol fix; default exact) | 0 / 0 | 0 / 0 | 0 | 400 / 400 | 0 wrong |

The largest randomised error stays at 5.47 sd. The fifth level with bail=24 also passed (80 queries, 0 wrong).

## Before and after at 1M

The baseline is the rebased start, b429c5352. Each figure is the median over 3 runs of the median over 20 queries,
and each side was run three times as separate processes, interleaved with the other work. The table gives the mean
of the three. Bytes are per item of N and include full vectors.

| Answer | item ms before | after | Δ | item B before | after | item full before | after | text ms before | after | Δ | text B before | after | text full before | after |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0.01% | 2.1 | 2.1 | +1% | 3.0 | 2.9 | 0.11% | 0.11% | 18.2 | 17.9 | −2% | 20.9 | 20.8 | 0.55% | 0.55% |
| 0.1% | 3.0 | 3.0 | −2% | 6.0 | 5.9 | 0.24% | 0.24% | 22.5 | 22.1 | −2% | 26.8 | 26.5 | 0.41% | 0.41% |
| 1% | 7.4 | 7.2 | −2% | 19.3 | 19.1 | 0.73% | 0.73% | 39.4 | 37.7 | −4% | 71.5 | 69.9 | 0.89% | 0.87% |
| 10% | 54.5 | 51.6 | −5% | 145.6 | 142.5 | 3.32% | 3.24% | 61.1 | 58.1 | −5% | 203.2 | 199.1 | 5.59% | 5.46% |
| 50% | 75.2 | 71.3 | −5% | 307.2 | 301.1 | 9.88% | 9.63% | 80.2 | 76.3 | −5% | 359.5 | 352.6 | 12.16% | 11.86% |
| 75% | 70.2 | 66.6 | −5% | 264.9 | 259.5 | 8.00% | 7.80% | 76.2 | 72.5 | −5% | 313.7 | 307.5 | 9.95% | 9.71% |

"Full" is the share of all items whose full vector is read.

## On by default

- Part 1: c0 and h0 in place of s, mr and adet; the variance term formed at the rise; |y| and kappa decoded by
  permutes.
- Constants per (group, level) from the exact law. `--csg-plain` turns them off.

Off: `tol=`, `--design-exact`, deeper schedules (`--rlv`), and the study dump (`VEXACT_DUMP`).

## Unproved ideas

None of the measured candidates needed a new proof. One argument that would, for the record:

- **Union only over the levels an item reaches.** The union runs over N × L (item, level) events, but most items
  stop at the first randomised level. Counting only the levels each item reaches would lower c. But which levels an
  item reaches depends on the seed, through the earlier randomised levels. The union over a seed-dependent set is
  not covered by the current proof (the M of `PROOFS.md` must be seed-independent). **Unproved, not built.**

The union's N is a design constant (10⁹) and is not the index's size. At N = 10⁶ the constant would be
8.56 instead of 9.33, but that changes what the guarantee promises, not the proof.

## A crash

One run crashed with a segmentation fault: a fifth level at 384 B with `bail=8` and `bail=24` variants, in the
timing loop for text queries. The same queries had already completed in that process's bytes pass. Three reruns of
the same configuration did not crash (`x_bail_base` on the start binary, `x_bail_new`, `x_crash2`). An
AddressSanitizer build (stable with `RUSTC_BOOTSTRAP=1`, `-Zsanitizer=address`) found nothing on the configuration,
at 50k and at 1M. The level loop's state is created fresh for every query, so a heap over-read is the likely cause.
It would have to land in memory that ASan does not watch: the 128-byte slack after the code arrays, or the memory
map of the full vectors. Four further runs of the crashing configuration were also clean, three of them with guard
pages and bounds asserts. **The cause is unknown, and may be hardware.** Both bail and five levels are off by
default.

## The next bottleneck

The breakdown of the final build at item 50% (ms, profiled, `bd_p2c`; a busier machine than the earlier
breakdowns):

- level decode 18.2
- bounds and decisions 21.2, of which scalars 10.8
- full vectors 10.9
- tables 6.1
- re-pack 4.7
- compaction 4.5

The scalars are now mostly the tan permute and the per-half bookkeeping of the rises. Decode is still the largest
phase, and it is bound by the bytes read. Full vectors cost 104 ns per item, so a deeper level does not repay its
per-cluster table cost in memory. Further gains have to come from one of:

- fewer bytes per decision at the levels: the 56 B level's deterministic bound on uncoded blocks is three quarters
  of its half-width;
- cheaper tables for clusters with few survivors;
- the caller's tolerance.
