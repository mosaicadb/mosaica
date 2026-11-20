# Integrating the visit order: plan

How the findings of `README.md` go into `rust/`: what each order costs at query time, how to make it cheap, how the
cluster space can be cut before the order is computed, and how each step is measured. The work lands on this
research line (`rust/`, `bench/annb/vexact-py`), not in Mosaica.

## What each order costs per query

`microbench/` times the work each order adds, alone, on one pinned core (Xeon at 2.1 GHz, AVX-512, built with
`target-cpu=native`), at LAION 1M's shape (K 500, d 512). Warm is the state in cache; cold evicts it before each
repetition. It is a harness with random data, not the index: phase 0 checks its first row in the real code.

| per-query work | warm | cold |
|---|---|---|
| q·m of every cluster, f64 query × f32 centre (as `dot64_32`) | 123-137 µs | 251 µs |
| the same, f32 × f16 AVX-512 GEMV | 8-10 µs | 29 µs |
| f′ (the spread of q·m over clusters) | ~0 | ~0 |
| H r4 projections, f16 / int8 directions (2 MB / 1 MB) | 66 / 37 µs | 119 / 88 µs |
| H r16 projections, f16 / int8 (8 MB / 4 MB) | 438 / 311 µs | 461 / 358 µs |
| H r16 for 64 / 128 candidate clusters only | 17 / 64 µs | 59 / 100 µs |
| shared basis, 32 / 64 directions | 13 / 26 µs | 15 / 30 µs |
| rad-aware model: τ by 40 bisection steps, K probabilities (unoptimised) | 79 µs | 88 µs |

For scale, the quick suite's filtered approximate k-NN on LAION (random-1) takes 0.53 ms at nprobe 16 and 0.59 ms at
nprobe 64: under a sparse visible set an extra cluster costs about 1 µs. Consequences:

- H r16 as stated is bound by memory bandwidth (about 18 GB/s) and would nearly double a filtered query for about
  +0.03 recall, while the 62 µs from nprobe 16 to 64 buys +0.24. What an order is worth is the clusters it saves
  times the cost of a cluster, which depends on the visible set's density; unfiltered, a cluster costs much more.
- If the harness is right about today's q·m, it is a quarter of a filtered query, and replacing it pays for H r4.
- f′ is free.

At scale the number of clusters grows as about √N (`tools/rules.py`: 2,000 items per cluster up to 2M, doubling for
each further 4× N): 6,250 clusters of 16,000 at 100M and 15,625 of 64,000 at 1B. There q·m over every cluster reads
8 MB of int8 centres per query at n = 512, H r4 over every cluster 32 MB and H r16 128 MB, so the spread can only be
computed over a pre-filtered set of clusters.

## Making the orders cheap

1. **One GEMV for the order.** Centres (int8 with a scale, as stored) and the clusters' directions in one matrix, the
   order's keys in f32. The exact f64 q·m only for the probed clusters, where the certified bounds (`cluster_q2`,
   `cut_params2`) read it.
2. **σ(q) without visiting every centre.** f′'s σ(q)² is qᵀCq for C the covariance of the centres: O(d·r) with a
   low-rank C, whatever K is.
3. **Cascade on the key.** With eigenvalues λ₁ ≥ λ₂ ≥ … ≥ s² (s² the isotropic rest), the first j projections bound
   the rank-r spread exactly: σ² lies between Σ_{i≤j} λ_i p_i² + s²(|q|² − Σ_{i≤j} p_i²) and the same with λ_{j+1}
   in place of s². With no projection, s²|q|² ≤ σ² ≤ λ₁|q|². Compute the cheap key everywhere and refine only the
   clusters whose upper key can still reach the cut: the same order as H everywhere, at the cost of the survivors.
4. **int8 directions** (a scale per direction): r16 −30%, r4 −45%.
5. **τ for the rad-aware model**: vectorised, started from max_c[μ_c + z·σ_c], a few secant steps, clusters that
   cannot contribute skipped. Target under 10 µs.
6. **Visible counts.** z(k, n) and the model's expected count take a cluster's visible count, not its size. More
   accurate under a filter, and it leans less on rows the viewer cannot see. Whole-cluster centres, radii and Σ_c
   still shape which visible items come back, as today.
7. **k.** The replays are at k = 10; the constants and z(k, ·) are checked at k = 100 offline before they are used
   there.

## Pre-filtering the cluster space

A hierarchy over centres is correct for exact search too; it fails there because a node's worst-case bound
(q·M + |q|·R) prunes nothing in hundreds of dimensions. The order needs no worst case, so the statistical key can
be used one level up.

- **The groups are already a level**: `fit.py` makes them by k-means over the normalised centres (at most 64; about
  8 clusters each at 1M, about 350 at 1B, which needs a middle level).
- **The node key must be invariant under the query shift** (`theory.md`, Theorem 11): a fixed radius term one level
  up repeats today's failure on text queries. A child's centre score is q·M_g + q·(m_c − M_g), whose spread over the
  children is √(qᵀC_g q), C_g the covariance of the child centres; the node key is q·M_g + z·√(qᵀC_g q) plus the
  largest child spread term, with a low-rank C_g as H does.
- **Expanding stops on the same global λ as probing** (the rad-aware model's probability), not at a fixed number of
  groups: text queries, whose centre scores are compressed, expand more groups without a separate setting.
- **Under a filter**, a node's visible count is the sum of its clusters'; nodes with none are skipped.
- **f′ as inner-product search**: its key is [q, β(q)]·[m_c, rad_c/σ(rad)] with β(q) = f′σ(q), an inner product with
  augmented centres, so any MIPS structure over the centres applies; H follows as the cascade's stage.

The risk is that a missed node loses all its clusters, and out-of-distribution queries are where node keys separate
least. Measured offline first (phase 2b).

## Into the index

- **At build, not at open.** `rad` and f are computed at open from pseudo-queries today. The directions, the
  eigenvalues, the model's table and the order's constants are computed by `rebuild` and stored, with a format
  version bump.
- **Writes** (not built yet in `rust/`): n_c and the sums behind the centre and rad merge on insert; directions stay
  as built until compaction (the order is a heuristic); the model's table is refitted at compaction.
- **API**: `knn_approx(q, k, stop)` with `stop` one of nprobe, a probability λ or a recall target; the order chosen at
  build. `fit_queries` stays as the override and refits the constants or the table on real queries. `Dial::fit`
  takes the order, so it follows it.

## Phases

Every phase is judged on recall against time per query, not against rows scanned: the order's own cost is counted.
A phase passes when it is at least as good as the baseline at every recall level, unfiltered and filtered.

0. **Baseline.** The quick suite's approximate k-NN on glove-100-angular, yahoo-minilm-384-normalized and
   laion-clip-512-normalized, unfiltered and under random-1 and random-25, nprobe 4 to 256, with the per-phase
   cycle counts (`Stats::cyc`, `P_PLAN` first). iNaturalist and BioCLIP are left out (disk; BioCLIP is not public).
   VIBE and ANN-Benchmarks need Apptainer, which this machine lacks.
1. **q·m as a GEMV, and f′.** Gate: `knn_approx` recall equals the replay's within 0.001, as `README.md` checked.
2. **H r4, then H r16 through the cascade.** Timed by phase; the survivors per stage reported.
   2b. **Hierarchy, offline.** In the replay harness: recall against rows scanned and against centres evaluated,
   and coverage (the share of the true neighbours, and of the flat order's top nprobe, inside the expanded groups),
   at the quick suite's K and at about 5,000 clusters (`--per-cluster 200`, more groups) to reach a 1B index's ratio
   of children to parent. Built only if coverage holds on LAION text queries.
3. **The rad-aware model's stopping and recall targets.**

## Progress

Runs on this machine (4 cores, timing on core 3, AVX-512), on the `order` suite (`suites.py`: glove-100-angular,
yahoo-minilm-384-normalized, laion-clip-512-normalized; 100 queries; approximate k-NN at k = 10, nprobe 4 to 256,
unfiltered and under random-1 and random-25). `results/order-baseline/` is the library as it was (suite
`order-default`), `results/order/` the current one with the settings `radius` and `spread`, run back to back.

**Measuring.** The machine's speed drifts by 10 to 40% between runs and within one (a run after a restart was faster
everywhere, including work nothing here changed). Between revisions of the library only back-to-back suite runs are
compared. Between orders, `scripts/ab_orders.py` opens one index per order in one process and gives each query to
every order in turn, and reports rows touched, which does not depend on the machine; `scripts/ab_curves.py`
interpolates time and rows to a recall. Differences below about 5% in a paired run, and 10% between runs, are not
findings.

**Phase 0: where the time goes** (`scripts/profile_phases.py`; `results/phases-0-baseline.txt`). Under a sparse
visible set the order was nearly the whole query: at random-1 and nprobe 4, 62 of 72 µs on glove, 79 of 94 on yahoo,
184 of 200 on LAION; unfiltered, 13 to 18%. Inside it, q.m of every cluster cost 37 µs (glove) to 137 µs (LAION).

**Phase 1.**
- `util::dot64_32` and `dot64` were vectorised two lanes wide by the compiler. Written for AVX-512 and AVX2 with the
  same operations in the same order (a multiply and an add per lane, not fused), they return the same bits 3.2×
  faster (q.m of every cluster 125 → 40 µs on LAION). The exact paths use them too.
- The order ranks clusters by int8 centres: `fit.py` stores each centre as int8 times max|m| / 127, which
  `Approx::new` recovers exactly from the f32 centres (0.25 MB on LAION against 1 MB), with an int8 GEMV
  (`kernel::gemv_i8`, AVX-512 and AVX2): 11 µs on LAION, within 1e-8 of the f64 products. q.m in f64, which the
  bounds take, is computed for the probed clusters only; the anchor stays the nearest of all (computed when a cluster
  on the index path first asks for it), since the nearest among the probed gave looser bounds.
- The `spread` order (f′ = 0.4, `OrderKind::Spread`) is the bindings' default; `radius` stays as an option, and
  `fit_queries` fits f for any order.

Back to back (`results/phase1-orders.txt`), with the same order as before (`radius`), a LAION query at random-1 takes
24 µs at nprobe 4 (153 before) and 32 at nprobe 16 (211); the order is 18 of them (`results/phases-1-int8-order.txt`).
Time to reach a recall, baseline / radius / spread, in µs:

| | recall 0.8 | 0.9 | 0.95 |
|---|---|---|---|
| LAION, unfiltered | 2088 / 2311 / 1396 | 4695 / 4038 / 1977 | 8352 / 8110 / 3964 |
| LAION, random-1 | 263 / 119 / 45 | 378 / 277 / 154 | 493 / 467 / 246 |
| LAION, random-25 | 2446 / 1954 / 1582 | 3939 / 4601 / 2338 | 7425 / 7334 / 4600 |
| glove, random-1 | 83 / 64 / 40 | 155 / 193 / 77 | 269 / 262 / 145 |
| yahoo, random-1 | 102 / 21 / 23 | 132 / 34 / 35 | 221 / 90 / 145 |

Unfiltered glove and yahoo, where the probed clusters' work dominates, differ by less than the drift. Paired
(`results/phase1-paired-radius-spread.txt`): on yahoo the two orders touch the same rows at the same recall and time;
on glove `spread` loses 0.008 to 0.015 recall at a fixed nprobe for 3 to 4% fewer rows (accepted); on LAION it gains
0.05 to 0.23 recall for 25 to 35% more rows and about 5% more time per nprobe.

**Phase 2: H** (`OrderKind::H { r, m }`, the bindings' orders `h4`, `h16`, `h4x8`).
- Each cluster's top r principal directions about its centre (int8, a scale each), their variances and the mean
  variance of the rest. The measurements below computed them exactly when the order was chosen (full covariance and
  eigendecomposition, faer). They are now computed by the build and stored (`rust/src/v2/order.rs`, store format 7;
  see the next section).
- z(k, n) takes the cluster's visible rows, and is negative under 2k of them (the bounds allow either sign).
- First version, the exact cut (the rank-0 bounds rest |q|² ≤ σ_c² ≤ λ₁|q|², `results/phase2-paired-h-exact-cut.txt`):
  unfiltered the bounds prune nothing (499 of 500 clusters computed on LAION; the order 113 to 317 µs for h4, 551 to
  1043 µs for h16). Even so h4 reaches recall 0.9 in 9% less time than `spread` on LAION and 0.95 in 25% less, and
  9 to 15% less under random-25; under random-1 it is 31 to 40% slower and h16 twice as slow.
- Second version: H's key only for the m·nprobe clusters first by the `spread` key (m = 4 unless the name gives it;
  x0 for every cluster). No longer exactly H's top nprobe, but at m = 2, 4 and 8 h4 needs the same rows to every
  recall as over every cluster on all three datasets (`results/phase2-paired-h-prefilter.txt`), so the cut costs no
  recall, and the order's cost now grows with nprobe, not K.
- Under random-1 H needed more rows than `spread` to every recall: with about 20 visible rows a cluster, z(10, n) is
  about 0, and H is `spread` with a weaker radius weight. H now takes the `spread` key for a query whose clusters
  hold under 8k visible rows on average (`H_MIN_ROWS_PER_K`; set between the measured points, about 2k rows a
  cluster under random-1 and 50k under random-25, not tuned between them). Under random-1 it then probes what
  `spread` probes, at the same time (`results/phase2-paired-h-sparse-switch.txt`).

Time to reach a recall against `spread`, paired (negative is faster):

| | recall | h4x2 | h4x4 | h16x4 |
|---|---|---|---|---|
| LAION, unfiltered | 0.8 / 0.9 / 0.95 | −11 / −15 / −26% | −12 / −14 / −25% | −21 / −22 / −30% |
| LAION, random-25 | 0.8 / 0.9 / 0.95 | −20 / −15 / −12% | −13 / −8 / −6% | −22 / −22 / −12% |
| yahoo, unfiltered | 0.8 / 0.9 / 0.95 | −3 / −9 / −9% | −3 / −9 / −8% | −3 / −13 / −13% |
| yahoo, random-25 | 0.8 / 0.9 / 0.95 | −9 / −13 / −21% | −12 / −14 / −20% | −14 / −19 / −24% |
| glove, unfiltered | 0.8 / 0.9 | −5 / −11% | −6 / −15% | −12 / −14% |
| glove, random-25 | 0.8 / 0.9 | −10 / −20% | −14 / −21% | −12 / −24% |
| random-1 (all three) | | ±5% (the `spread` order) | | ±6% |

h16x4 is the best order wherever the visible set is dense. Its state is 4 MB on LAION 1M (int8) and takes 6.3 s on
4 threads to compute, almost all of it the full 512 × 512 eigendecompositions.

**The order state at build** (`v2::order`). The build computes each cluster's state from the rows it encodes and
saves it beside the index; an open maps it and computes nothing (radius's f is still fitted at open). Per cluster:
its RMS radius over all its rows (f64), and 16 directions (int8 rows with an f32 scale each) with their variances
(f32), 16·n + 136 bytes (8.3 kB at n = 512, 4.2 MB on LAION 1M; 510 MB at 1B and n = 2048). An order with r ≤ 16
takes the first r; its rest is (rad² − Σ_{j≤r} λ_j) / (n₀ − r).
- The directions come from up to 4,096 rows spread evenly over the cluster, by subspace iteration on a block of 32
  started from random signs (5 products with the sample's covariance, Gram-Schmidt between them), then a 32 × 32
  Rayleigh-Ritz step (Jacobi). All of it in a fixed order with separate multiplies and adds, so the state is the same
  bits under AVX-512 and AVX2, built or recomputed from the saved rows.
- Against the exact directions (`order::tests::sampled_directions_against_the_exact_ones_on_a_real_fit`, 40 clusters
  of each fit, median / 90% / max): LAION (1,946 rows a cluster, all in the sample) variances 1 and 4 within
  0.0000 / 0.0000 / 0.007, variance 16 0.0008 / 0.007 / 0.025; the span of the first 16 misses 0.0003 / 0.001 /
  0.002 of their variance; σ_c(q) for centres as queries within 0.0009 / 0.0027 / 0.015 (16 directions) and
  0.0009 / 0.0025 / 0.054 (4). Yahoo and glove alike (σ_c within 0.007 at 90%, 0.09 at most). The largest
  principal angle of the first 16 is 0.06 to 0.7 rad at the median: their 16th and 17th variances are nearly equal,
  so the span is ill-defined, while the variance it captures is not. From fewer rows the error is the sample's:
  with 1,024 rows σ_c is within 0.007 to 0.022 at the median and 0.023 to 0.072 at 90%, variance 1 within 0.01 to
  0.03 at the median; 4,096 rows of a 64,000-row cluster at 1B should be about half that (the error goes as
  1/√rows).
- Cost: 0.025 s a LAION cluster on one thread (the exact one 0.066 s); at 1B and n = 2048 about 0.25 s a cluster,
  1.1 thread-hours for 15,625 clusters, where the exact covariances would take about 25 s a cluster (both
  extrapolated from LAION by their operation counts).
- The tables above were measured with the exact directions; the stored ones have not been run through the suite yet.

**Phase 3: the ablation, and the consolidated design.** Paired runs (k = 10, three datasets, five visible sets;
`results/ablation/` on branch `ccr-b37f5194-u55ji1`, `summary.txt`) set a calibrated process (exceedance tables per
spread tier, the plan at a price, its hand-overs; `hierarchy/policy.md` section 11) against simple arms built one
step at a time: S1 the `spread` order by nprobe with a measured cost model for each cluster's path, S2 an adaptive
stop (next key below the running k-th best), S3 a race against a scan of the visible rows, S4 the key μ + ψσ_c.
S4 with each cluster's stored directions beat the calibrated process almost everywhere and is the first design to
beat `spread` and h16x4 unfiltered. Time to recall 0.8 / 0.9 / 0.95 against `spread`: unfiltered LAION 0.59 / 0.67 /
0.59, yahoo 0.51 / 0.64 / 0.83, glove 0.76 / 0.70 / 0.68; random-25 0.12 to 0.43; region-25 0.36 to 0.62; region-1
0.26 to 0.58 (S1 better on LAION and glove); random-1 1.08 to 2.12 at 0.8 and 0.64 to 0.93 at 0.9. S2's f is not a
robust knob (recall 0.99 at f 0.5 on glove, 1.0 to 1.25 on LAION); ψ is: 4 gives 0.78 to 0.89 unfiltered, 5 gives
0.90 to 0.985, filtered sets more. The low-recall random-1 points were each arm's first cells, run right after
another arm's last (most expensive) ones: `spread` at nprobe 4 took 122 µs for 90 rows on LAION against 55 µs for
173 rows at nprobe 8. `scripts/ab_orders.py` now shuffles the cells per query.

The library now has one approximate k-NN and one approximate threshold query (`rust/src/v2/adaptive.rs`; bindings
`knn_approx(q, k, psi=4.5)`, `threshold_approx(q, t, psi=4.5)`): the key μ_c + ψσ_c(q) from the stored directions,
the clusters taken in decreasing μ_c + ψ·rad_c·√(qᵀC̄q / tr C̄) (C̄ the pooled within-cluster covariance, Σ_c n_c Σ_c / N
from the stored directions, as its top 16 directions and an isotropic rest, computed at open; no fitted constant)
with the next 8 priced (σ computed), the largest key among those read next, and the stop when their largest key is below the running k-th best (threshold:
the clusters whose key reaches t, after the support table, decided from the bounds before any σ), the race against a scan (k-NN; at most twice its modelled
cost) and the hand-over to a scan or the exact query (threshold), and each cluster's path by constants measured on
the machine and cached with the index. The calibrated process (its tables, group covariances, price and budget, and
the build's calibration), the H order, the radius order and `fit_queries` are gone. The k-NN stop is a heuristic:
a cluster not yet priced may still have a key above the k-th best (on synth4k 11 of 878 stops, against reading
every cluster for the same items in 685). The exact lazy stop, which prices a cluster whenever its bound beats the
best key priced, computed σ for every cluster on real data (the bound is about 5 times σ) and cost about 500 µs a
query on LAION (`results/consolidated-exact-stop/` on branch `ccr-b37f5194-u55ji1`); it was taken out. A stream in
the order of that bound (μ_c + ψ√(λ₁)|q|) instead ranks clusters by their largest variance: recall 0.07 at ψ 4.5 on
LAION, so the stream takes the radius scaled by the query's share of the pooled covariance. A part of q along
which no cluster varies leaves that scale unchanged, where rad_c|q|/√n₀ (the key first taken) grows with it. On
LAION the two keys read nearly alike (ψ 4.5 unfiltered: recall 0.919 at 59.8k rows either way): the top 16 pooled
directions hold 16% of the trace, so the scale stays within 0.97 to 1.11 of |q|/√n₀ over the 100 queries, where the
pooled covariance of rows sampled from every cluster puts it at 0.56 to 1.30 (median 0.91; the scale from every
cluster's stored directions, untruncated, correlates 0.98 with that but spans only 0.87 to 1.16, the clusters'
isotropic rests holding most of their variance). The `spread` order by nprobe
stays as `knn_nprobe`, the legacy comparison arm, until the owner's benchmark has run. The exact planner can take its
rule in rows from the measured constants (`Index(..., rule="measured")`); its default is unchanged.

The approximate threshold query under 1% visible sets took 1.04 to 1.29 times the exact query's time to recall 0.9
and 0.95 (`results/consolidated-final/`; yahoo random-1 0.90 and 0.93). On those sets the exact query scans the
visible rows before anything else (its planner's rule in rows: every cluster holding a visible row could be scored
directly, in sum), and the approximate one handed over too, to the same scan, in 75 to 100% of the queries at ψ 3 to
6 (yahoo: 6 to 100%), but only after its fixed work (centre scores and anchor search: 53, 31 and 20 µs modelled on
LAION, yahoo and glove random-1, against scans of 551, 275 and 207). It now is the exact query from the start on a
visible set the exact query scans first (`Plan::threshold_scans_first`, path "sparse"), decided from the visible
set alone: every query on random-1 and region-1 of the three datasets, none unfiltered or at 25%. Modelled saving
a query at ψ 3 / 4.5 / 6: LAION random-1 63 / 57 / 53 µs, region-1 26 / 30 / 32; glove 20 and 15 at each ψ; yahoo
random-1 −45 / 62 / 45, region-1 −61 / 3 / 13 (its ψ 3 probes were modelled cheaper than the scan, at recall 0.92
and 0.85). A test on the measured costs alone (hand over when the exact query costs no more than the approximate
one's fixed work and the reads it is sure of) cannot see these sets before the centre scores: the fixed work is a
tenth of the scan, and no read is sure without them. k-NN keeps its probe there (its fixed work is 2 to 2.5% of
the scan, and it reaches recall 0.9 reading a tenth to a third of the visible rows, in 0.5 to 1.1 times spread's
time); its existing test, a scan when the fixed work costs as much as one, is that floor test and fires on none of
these sets.

**The hierarchy: decided.** No level over the clusters: trees over the centres save little at these cluster counts
and only one level is ever useful (`hierarchy/replay.md`, `hierarchy/theory.md`), and re-clustering into more,
smaller clusters breaks robustness under visible sets aligned with the real clusters (31 to 47 groups touched
against 3). Under the clusters, sub-clusters were tried and are **shelved** (`subclusters.md`; experimental code on
branch `subclusters-exp`, not merged):
- Reading a sub-cluster costs the same per row as its whole cluster only when its rows are contiguous; a masked read
  (a label per row, rows kept in key order) costs 1.1 to 2.5 times per row, and contiguous key ranges are free to
  read but not coherent in space and gained nothing.
- Contiguous k-means sub-clusters (rows ordered by sub-cluster, then key) leave the exact answers unchanged but cost
  the exact queries 5 to 8% unfiltered and up to 17% (k-NN) or 12% (threshold) under random-25, from 16 binary
  searches and 16 short runs per cluster in place of one.
- With them the approximate k-NN took 0.69 to 0.88 of the current one's time unfiltered on glove and yahoo (0.80 to
  1.01 on LAION) and 0.65 to 0.94 under the 25% sets, but 0.93 to 1.24 under the 1% sets, where it split too many
  clusters for too little saving.
- So it fails both halves of the bar set for it (a small cost to the exact queries, and clear approximate gains
  without losses elsewhere). Worth revisiting if the approximate mode needs another 20 to 30%, with, cheapest first:
  the exact paths' kernel run once per run (to remove their cost), no splitting under sparse visible sets (to remove
  the 1% losses), and clusters of 8,000 rows, where the gain should be larger.

The owner's benchmark showed k-NN's time growing far faster than recall above ψ ≈ 5 (glove ψ 7 about 35 ms
against exact `knn2` about 17 ms; LAION ψ 6 recall 0.980 at 231k rows against nprobe 64's 0.977 at 132k). The order
was made independent of ψ (ψ moves only the stop): μ + z(1, v)σ, z(1, v) = Φ⁻¹(v/(v + 1)) of the cluster's v visible
rows, the stream μ + z(1, v)·rad·√(qᵀC̄q / tr C̄), the stop the window's largest μ + ψσ below the running k-th best,
ψ = ∞ the exact query. Two ways to read and hand over were then timed against `spread` (100 queries, 5 repetitions,
core 3; time to a recall, adaptive / spread, ψ = ∞ left out of the curve):
- *with a cost model* (`ad722d3`): each cluster read directly or on the index path by the measured constants, the
  index path priced at the running k-th best from the rows its sorted range leaves and their share above it (the
  timing flushing each cluster's vectors from the caches), and the scan race with a hand-over as soon as the reads
  projected cost more than scanning the rest. An earlier version also predicted the exact query's cost per query
  (`d3778d5`); pricing it cost about 1 ms a query on LAION and was taken out.
- *with no per-query cost model* (the current code): the exact query when its planner scans the visible rows first
  (`Plan::scan_first`), decided before anything is read; else each cluster read by the planner's rule in rows
  (directly when it holds at most `Plan::direct_max` visible rows, else on the index path); no race.

| recall 0.8 / 0.9 / 0.95 / 0.98 / 0.99 / 0.995 | cost model | no cost model |
|---|---|---|
| LAION none | 0.60 / 0.64 / 0.65 / 0.69 / 1.01 / 1.20 | 0.67 / 0.70 / 0.65 / 0.67 / 0.89 / 1.16 |
| LAION random-1 | 1.29 / 1.02 / 2.44 / 1.78 / 1.45 / 0.89 | 7.72 / 4.53 / 2.78 / 1.65 / 1.31 / 0.81 |
| LAION random-25 | 0.31 / 0.34 / 0.45 / 0.61 / 0.70 / 1.17 | 0.60 / 0.53 / 0.58 / 0.67 / 0.71 / 0.85 |
| yahoo none | 0.61 / 0.69 / 0.70 / 0.73 / - / - | 0.73 / 0.83 / 0.77 / 0.72 / - / - |
| yahoo random-1 | 1.42 / 1.13 / 0.95 / 0.92 / 1.16 / 1.12 | 8.10 / 5.50 / 3.45 / 2.03 / 1.54 / 1.09 |
| yahoo random-25 | 0.36 / 0.35 / 0.36 / 0.53 / - / - | 0.80 / 0.73 / 0.65 / 0.74 / - / - |
| glove none | 0.89 / 0.65 / 0.75 / 1.09 / 1.41 / 1.69 | 0.77 / 0.62 / 0.70 / 0.93 / 1.24 / 1.52 |
| glove random-1 | 1.59 / 1.24 / 1.65 / 1.41 / 0.96 / 0.84 | 4.18 / 2.50 / 1.64 / 1.15 / 0.77 / 0.66 |
| glove random-25 | 0.48 / 0.32 / 0.37 / 0.51 / 0.85 / 1.14 | 0.77 / 0.49 / 0.52 / 0.66 / 0.97 / 1.10 |

("-": spread's grid does not reach it within the run; a ratio at random-1 with no cost model is the exact query's
time against spread's at that recall.) Unfiltered the two are within the runs' noise (spread's own times moved 5 to
10% between them), the version without a cost model a little ahead on glove's top. It loses where its two rules
misjudge the probe: at 1% visible the exact planner scans first, so every query is the exact one (0.9 recall costs
the full scan, 4.5 to 5.5 times spread's time, where the probe had it at 1.0 to 1.1); at 25% the rule in rows sends
most clusters to the index path (500 visible rows of 2,000, above the rule's 436), which for the probe costs more
than reading them directly (LAION ψ 4: the same 30 clusters in 1.7 ms against 1.4; yahoo ψ 5 0.93 ms against 0.56),
group setups included. Both rules were made for the exact threshold query, whose index path prunes at its final
threshold.

**Counting the reads in dot products, checked before building it.** The plan was to choose each cluster's read path
by its work in d-dimensional dot products, from the structure and with no timing: a direct read nv; the index path a
group's setup (2n + Σ nb_j²/n + ndl + J + 3), the cluster's setup and tables, the rows of its sorted range through
the first level at its units/n each, and the f·nv rows expected above the k-th best through the rest and their stored
vectors. Checked against measured time (`study_count_model`, three datasets, unfiltered and random-25, 1,024 reads
each, one dot product the time of a warm direct read per row; `results/count-model-and-orders/`), it is off by far
more than 1.5x, so it was not built:
- The levels do not drop most rows early. Of the rows a sorted range leaves, 98%, 94% and 81% enter LAION's second,
  third and fourth levels (yahoo 80%, 52%, 26%; glove 66%, 15%), where f is about 1%: the work through the deeper
  levels is 100 to 500 times the prediction (LAION 819k dot products counted against 3k predicted), while the
  first level, the tables, the cluster setup and the stored vectors read are each predicted within 1.5x.
- Counted from what the reads did, the index path still costs more per counted dot product than a direct read does:
  1.3 to 1.7 times unfiltered, 2.5 to 3.8 times under random-25, where clusters are smaller and per-cluster work
  the counts do not hold (buffers, gathers, the levels' setup) weighs more.
- A direct read costs 1.9 to 6.8 times the warm dot product per row (cold rows; scattered ones under random-25), and
  a group's setup 1.6 to 5.2 times its count (glove's small n leaves fixed work dominant).

**What limits the orders at high recall: the order or the execution.** Rows to recall (`study_order_rows_to_recall`;
each order read whole, the same number of clusters for every query, a query reading nothing more once it holds its
true ten; 100 queries):

| rows to 0.95 / 0.99 / 0.995 | μ + z(1, v)σ | spread | tail, oracle t | tail, running t |
|---|---|---|---|---|
| LAION none | 31k / 49k / 57k | 39k / 62k / 66k | 35k / 51k / 62k | 36k / 58k / 65k |
| LAION random-25 | 10.1k / 15.9k / 18.1k | 11.8k / 22.2k / 23.6k | 9.7k / 15.3k / 17.4k | 10.6k / 19.9k / 21.4k |
| yahoo none | 12.9k / 23.4k / 24.8k | 15.0k / 27.6k / 29.2k | 14.8k / 24.4k / 26.6k | 14.9k / 26.8k / 28.2k |
| yahoo random-25 | 4.5k / 8.2k / 9.4k | 5.9k / 9.6k / 10.8k | 4.8k / 8.7k / 10.4k | 4.9k / 9.3k / 11.0k |
| glove none | 81k / 129k / 130k | 73k / 114k / 125k | 73k / 117k / 121k | 82k / 126k / 136k |
| glove random-25 | 24.6k / 36.4k / 37.7k | 24.1k / 36.1k / 36.7k | 22.9k / 33.2k / 35.6k | 26.6k / 36.5k / 38.1k |

The current order reads fewer rows than spread to 0.995 on LAION and yahoo (13 to 23% fewer) and 3 to 5% more on
glove; the oracle tail order (the chance a cluster holds a visible row above the true 10th best) beats spread
everywhere at 0.995 but gains little or nothing over the current order (and the tail order at the running k-th best
does worse than the current order). So spread's edge in time at 0.99 and above is not its order: it reads more rows
than ours for the same recall. It is execution, the cost per cluster and per row of the reads (legacy `knn2_fast`
against the probe's reads and its index path).

**A planner with few measured constants** (`results/measured-planner/`). Fitted to the study's single timed reads
(`step1-fit.txt`), direct(nv) = a + nv·row and index(c) = cl + c·cut_row (c the rows the sorted range leaves) predict
each read within a median of 1.02 to 1.26 and a 90th percentile of 1.5 to 2.6; timing the same read twice gives a
90th percentile of 1.1 to 1.6 (`step1-timing-noise.txt`), so most of the tail is the timing's. Constants fitted
unfiltered predict random-25's index reads 2.1 to 2.6 times too low: the per-row costs are timed at the visible
share. The planner (current code): k-NN the exact query only at ψ = ∞, no race; a cluster's σ only when it costs less
than reading the cluster directly; each read by the cheaper of direct(nv) and index(c) + a new group's setup, c at
the running k-th best, the window's new groups set up in one batch; the threshold query priced by the same model.
Its constants, each one thing the reads do, timed at open: `dir_us` (a direct read beyond its rows), `row_us` at
four visible shares (a row read directly), `cl_us` (a cluster opened on the index path), `cut_row_us` at four shares
(a row the sorted range leaves, through the levels), `cut_us` (computing the range), `grp_us` (a group's setup),
`sig_us` (a σ). Time to recall against spread (k-NN, ψ = ∞ left out of the curve), 0.8 / 0.9 / 0.95 / 0.98 / 0.99 /
0.995:

| | measured planner | cost model (ad722d3) | no cost model (0d1a36c) |
|---|---|---|---|
| LAION none | 0.72 / 0.71 / 0.66 / 0.70 / 1.02 / 1.28 | 0.60 / 0.64 / 0.65 / 0.69 / 1.01 / 1.20 | 0.67 / 0.70 / 0.65 / 0.67 / 0.89 / 1.16 |
| LAION random-1 | 0.92 / 0.97 / 0.97 / 0.95 / 1.12 / 0.89 | 1.29 / 1.02 / 2.44 / 1.78 / 1.45 / 0.89 | 7.72 / 4.53 / 2.78 / 1.65 / 1.31 / 0.81 |
| LAION random-25 | 0.29 / 0.31 / 0.44 / 0.57 / 0.75 / 1.00 | 0.31 / 0.34 / 0.45 / 0.61 / 0.70 / 1.17 | 0.60 / 0.53 / 0.58 / 0.67 / 0.71 / 0.85 |
| yahoo none | 0.86 / 0.82 / 0.78 / 0.75 / - / - | 0.61 / 0.69 / 0.70 / 0.73 / - / - | 0.73 / 0.83 / 0.77 / 0.72 / - / - |
| yahoo random-1 | 1.00 / 0.83 / 0.83 / 0.74 / 0.75 / 0.79 | 1.42 / 1.13 / 0.95 / 0.92 / 1.16 / 1.12 | 8.10 / 5.50 / 3.45 / 2.03 / 1.54 / 1.09 |
| yahoo random-25 | 0.29 / 0.32 / 0.36 / 0.50 / - / - | 0.36 / 0.35 / 0.36 / 0.53 / - / - | 0.80 / 0.73 / 0.65 / 0.74 / - / - |
| glove none | 0.79 / 0.59 / 0.65 / 1.00 / 1.34 / 1.63 | 0.89 / 0.65 / 0.75 / 1.09 / 1.41 / 1.69 | 0.77 / 0.62 / 0.70 / 0.93 / 1.24 / 1.52 |
| glove random-1 | 0.80 / 0.97 / 1.10 / 1.42 / 1.30 / 1.25 | 1.59 / 1.24 / 1.65 / 1.41 / 0.96 / 0.84 | 4.18 / 2.50 / 1.64 / 1.15 / 0.77 / 0.66 |
| glove random-25 | 0.42 / 0.29 / 0.37 / 0.58 / 0.94 / 1.10 | 0.48 / 0.32 / 0.37 / 0.51 / 0.85 / 1.14 | 0.77 / 0.49 / 0.52 / 0.66 / 0.97 / 1.10 |

The threshold query against the exact one (share 0.01, recall 0.9 / 0.95 / 0.99) did worse than with the earlier
constants under random-25 (LAION 0.21 / 0.29 / 0.51 against 0.15 / 0.22 / 0.44; yahoo 0.10 / 0.17 / 0.31 against
0.06 / 0.10 / 0.20; glove 0.34 / 0.41 / 0.50 against 0.14 / 0.20 / 0.30) and about as well unfiltered (LAION 0.37 /
0.50 / 0.78 against 0.32 / 0.46 / 0.80): it prices the index path at every visible row with costs timed at a
probe's k-th best, where its own threshold prunes differently.

The index read's residual is not flat in the gap g = (t − μ)/σ (`gap-and-pass-through.txt`): reads of near clusters
(g 1 to 3) take 1.3 to 1.9 times the fit, far ones (g ≥ 4) 0.95 to 1.14. The levels' pass-through follows the normal
model with the code's own half-widths: a row passes level l when its score reaches t − w, w = (dwa_l · pconst_l ·
s_i · ntab_l[code_i] + fixed + s_i · ewdet) · dial_l (dwa_l the query's norm, less α times the centre, in the group's
coordinates weighted by the level's distortion D_l: what level l leaves unresolved, per query and cluster; pconst_l
per group and level; s_i the row's scale from its key; ntab_l[code_i] the row's stored 4-bit residual-norm class,
escapes stored per row; fixed per query and cluster), so 1 − Φ((t − μ − w)/σ) predicts the share of the range's rows
entering each deeper level within 0.01 to 0.04 on every dataset and gap (glove 0.655 / 0.153 observed against 0.627 /
0.173; yahoo 0.797 / 0.517 against 0.791 / 0.513; LAION 0.982 / 0.942 against 0.977 / 0.931), and the cluster's
median w does as well as every row's.

Going past step 1 although its 90th-percentile errors exceeded 1.5x was justified by timing the same reads twice:
the timing alone spreads them about as widely (`results/measured-planner/step1-timing-noise.txt`).

**A pass-through term, checked before building it** (`results/pass-through-and-groups/`). The deterministic levels'
pass-through, 1 − Φ((t − μ − w_l)/σ) with the cluster's median half-width, as a regressor in the index read's fit
(`pass-through-fits.txt`): on glove it flattens the residual by gap (1.39 at g 1 to 2 and 0.83 at g ≥ 5 without it,
1.02 to 1.11 with a deep-level cost per share); on yahoo and LAION it does not (1.81 to 0.99 and 1.33 to 0.92 with
or without it). With the observed entries into every level as regressors (an oracle, `pass-through-ceiling.txt`)
the residual is flat on all three (1.02 to 1.19): the gap's cost on yahoo and LAION lies in the randomised levels
and the stored vectors, whose half-widths take another form (the uncoded blocks' norms and the coded ones' variance),
not in the deterministic levels. Even the exact deterministic pass-through leaves 1.65 (yahoo) and 1.32 (LAION) at
g 1 to 2. The cheap form of w_l (the query's norm orthogonal to the centre times the group's root-mean-square
distortion, in place of dwa_l, which needs the group's setup) is off from the exact one by a median of 1.0 to 1.46
and a 90th percentile of 1.1 to 4.2. So the term was not built: it would need the randomised levels' pass-through
too.

**Groups set up on first read** (the current code) against the window's new groups set up in one batch (989bab9),
the k-NN suite on both (`summary.txt`): within the runs' noise or better on LAION and yahoo (LAION unfiltered 0.63 to
1.15 of spread against 0.70 to 1.21), better under glove random-25 (0.25 to 0.92 against 0.29 to 1.06), worse at the
top of glove unfiltered (1.12 / 1.50 / 1.81 at 0.98 / 0.99 / 0.995 against 1.01 / 1.34 / 1.63). Kept, as the
simpler one. Nothing in k-NN compares a modelled total with a scan or the exact query any more; the threshold query
still hands over to the cheaper of a scan and the exact query when its sure reads (and the sigmas still to price)
cost as much, comparing modelled read costs on both sides, with no per-query fixed work on either.

**The measured gap curve** (`results/gap-curve/`). The index path's cost per row the range leaves is cut_row_us(share)
× h(g), g = (cut − μ_c)/σ_c (rad_c · s where the cluster has no sigma), h in seven bins of g (edges 1 to 6), 1 at the
median gap of the reads timed. At open each sampled cluster is read on the index path at a rotating cut (the query's
10th and 100th best, μ + {0, 2, 4, 6} σ); g is clamped at query time to the 1st to 99th percentile of the gaps
timed: 0 to 6.0 (glove), 0 to 13.7 (yahoo), 0 to 10.1 (LAION). The curves (`constants-*.txt`) fall from 2.3 to 3.5
at g < 1 to 0.44 to 0.55 at g ≥ 6. The residual of the timed reads by gap bin (`gap-study.txt`) is flat on glove (1.00
to 1.17) and rises from 1.05 to 1.38 (yahoo) and 1.01 to 1.35 (LAION). By share within a gap bin it ranges 0.96 to
1.93 with no pattern across shares, so share × h(g) separates within the timing's noise and no 2-D table was built.

The curve depends on which (cluster, cut) pairs are timed (`gap-study.txt`, three fits). Fitted on corpus rows as
pseudo-queries, on test queries, and on threshold queries at the prep thresholds, the cost per row at share 1 differs
by up to 2.19 (glove), 1.61 (yahoo) and 1.74 (LAION) between kinds in the same bin. The largest gaps are at g < 3,
where threshold queries cost 0.5 to 0.7 of the corpus rows' cost. The threshold kind also covers fewer gaps (−0.3 to
4.4 to 7.8). Corpus and test queries share a shape apart from the top bins, but corpus rows cost 1.2 to 1.5 times more
on glove and less on LAION (the corpus fit leaves 0.79 to 0.86 on glove's test-query reads and 1.09 to 2.00 on
LAION's). So g is not a sufficient statistic for the read's cost.

Region against random filters, at matched per-cluster share, current model, real queries on the prep sets:
direct/index residuals of 1.25/1.01 (random-1) against 1.29/0.87 (region-1) on glove, 1.88/1.84 against 1.77/1.65 on
yahoo and 1.39/1.80 against 1.08/2.12 on LAION; 1.05/0.78, 1.61/1.41 and 1.51/1.39 (random-25) against 1.12/0.84,
1.33/1.15 and 1.29/1.04 (region-25). Long visible runs are cheaper than the model says: yahoo region-25's residual
falls from 1.76/1.44 at runs under 1.5 rows to 0.93/0.68 at 16 or more, LAION's from 1.42/1.44 to 1.10/1.06. No
run-length term was built.

The threshold query's modelled-total hand-over (to a scan or the exact query) against the sparse planner hand-over
alone (`hand-over-kept/` and `hand-over-removed/`): removing it is within noise or better everywhere: glove random-25
0.23/0.27/0.34 of the exact time to recall 0.9/0.95/0.99 against 0.29/0.36/0.44, glove region-25 0.30/0.35/0.42
against 0.43/0.51/0.62, yahoo random-25 0.08/0.13/0.25 against 0.10/0.17/0.31, LAION within 0.01 to 0.06. It was
removed. A threshold query now hands over only when its visible set makes the exact query scan first. With psi
infinite it reads every open cluster without a sigma.

Re-timed (`summary.txt`). k-NN against 3bd18eb is within noise under random-1 and random-25. The unfiltered results
are mixed: yahoo is better at 0.8 and 0.9 (0.53/0.72 of spread against 0.74/0.79), glove is worse at 0.95 and 0.98
(0.94/1.37 against 0.71/1.12), and LAION is 0.02 to 0.08 worse. Region-1 is 0.41 to 0.73 of spread, region-25 0.41 to
1.12. The modelled k-NN cost per measured time moves toward 1 at small psi (0.38 to 0.55 at psi 2 before, 0.41 to
1.02 now). The threshold under random-25 and region-25 matches or beats 989bab9. Under random-1 and region-1 it is
1.00 to 1.03 of the exact query, against consolidated-final's 1.08 to 1.15 at 0.99. Unfiltered, it is worse than
989bab9 at 0.99 (glove 1 in 10,000: 0.72 against 0.55, LAION 1%: 0.93 against 0.78, yahoo 1%: 0.64 against 0.54) and
at 0.9 to 0.95 on glove and LAION, with or without the hand-over. This is consistent with the gap curve fitted on corpus
rows overpricing a threshold query's near clusters (above), but not yet tested.

**The gap curve dropped; the threshold priced at its cut** (`results/threshold-cut/`). The curve was dropped: it
differed by up to 2.2 times between query kinds, left yahoo's and LAION's residuals rising, and added about 108
lines. The threshold's regression had another cause: it priced each cluster's index path at every visible row
(`index(nv)` = cut_us + cl_us + cut_row_us(share) · nv against `direct` = dir_us + row_us(share) · nv), so the path
saved only (row_us − cut_row_us) · nv. Under a filter cut_row_us is above row_us at most shares (yahoo and LAION at
share 1/4 excepted, where clusters hold too few visible rows to cover cl_us), and unfiltered it pays only for
glove clusters over about 1,100 rows. The threshold query now prices it as k-NN does: at the rows the sorted range at
t leaves, found for the cluster being decided and only when the index path could be the cheaper, plus the group's
setup when the group is new. The group bought by foregone savings against the k-NN rule (bought when the read with
its setup is cheaper): the same decisions under random-25 (every cluster read directly in both), and within the
runs' noise elsewhere. The noise is large: yahoo random-25 with identical decisions in both gives 0.18 against 0.23
of the exact time at 0.99. Glove unfiltered is 0.54/0.65/0.77 against 0.55/0.67/0.80, glove region-25 0.39/0.45/0.53
against 0.35/0.41/0.49. The k-NN rule, the simpler, was kept. Against 989bab9 (the same procedure) the threshold
matches it unfiltered (glove 1 in 10,000 0.25/0.32/0.54 against 0.26/0.33/0.55; LAION 1% 0.38/0.51/0.78 against
0.37/0.50/0.78; yahoo 1% 0.20/0.30/0.51 against 0.22/0.34/0.54) and beats it under random-25 (glove 0.22/0.26/0.32
against 0.34/0.41/0.50; yahoo 0.07/0.13/0.23 against 0.10/0.17/0.31; LAION equal). It recovers 1e0061a's regression
(LAION 1% at 0.99: 0.78 against 0.93). Under random-1 and region-1 it is 1.01 to 1.03 of the exact query (the sparse
hand-over). Consolidated-final, an earlier build and session, still has the lower ratios at 0.9 unfiltered (glove 1
in 10,000: 0.13 against 0.25) and under random-25 and region-25 (glove random-25 0.14/0.20/0.30 against
0.22/0.26/0.32). k-NN, back to 3bd18eb's pricing, is within noise of 3bd18eb and better at glove unfiltered's top
(1.02/1.38/1.69 of spread at 0.98/0.99/0.995 against 1.12/1.50/1.81).

**Pushed for benchmarking (f528836), against the previous push (00965ba, consolidated-final).** k-NN time to recall
0.8/0.9/0.95/0.98/0.99/0.995 as a multiple of spread's, each from its own run (cross-run; the ratio to spread takes
out most, not all, of the machine's drift; "-" where the psi grid did not reach the recall):

| | 00965ba | f528836 |
|---|---|---|
| LAION none | .61/.72/.76/1.97/-/- | .70/.68/.68/.68/.90/1.20 |
| LAION random-1 | .96/.95/1.12/1.68/1.79/1.14 | 1.01/.99/1.00/.97/1.23/.96 |
| LAION random-25 | .27/.35/.49/1.17/-/- | .28/.31/.40/.56/.69/1.06 |
| LAION region-25 | .43/.57/.96/1.24/1.49/- | .46/.49/.58/.79/.99/.83 |
| glove none | .62/.55/.71/1.16/1.51/1.88 | .84/.60/.74/1.02/1.38/1.69 |
| glove random-25 | .34/.28/.24/.31/.37/.50 | .39/.25/.28/.46/.73/.82 |
| glove region-1 | .43/.55/.58/.56/.55/.55 | .65/.66/.78/.79/.82/.82 |
| glove region-25 | .39/.38/.38/.44/.52/.62 | .56/.48/.69/.79/.93/.80 |
| yahoo none | .59/.72/.92/.99/-/- | .78/.80/.76/.70/-/- |
| yahoo region-25 | .37/.38/.36/-/-/- | .61/.54/.46/.52/-/- |

The cost of high psi (the reason for this round) is fixed: LAION at 0.98 goes from 1.97 to 0.68 of spread, and the
filtered sets at 0.98 from 1.2-1.7 to 0.56-0.97. Known regressions against 00965ba: glove under filters (random-25
at 0.99 0.37 to 0.73, region-1 about 0.55 to about 0.8, region-25), recall 0.8 on glove and yahoo unfiltered and on
yahoo region-25, and the approximate threshold under 25% sets (glove random-25 at 0.9: 0.22 of exact against 0.14;
cross-run). Not yet diagnosed; 00965ba's order put psi in the key (mu + psi sigma), which may suit glove's small
clusters under filters.

**The glove regression, diagnosed** (`results/glove-regression/`, `results/order-rows/`). Run back to back in one
session (twice each), the regression is real under filters. At 0.95/0.98/0.99/0.995, as a multiple of spread's time:
- random-25: 0.20 to 0.49 (00965ba) against 0.28 to 0.81 (f528836);
- region-1: 0.56 to 0.71 against 0.76 to 0.81;
- region-25: 0.38 to 0.64 against 0.72 to 0.96.

Unfiltered, the regression at 0.8 is in the baseline, not in the probe. The two builds' spread times differ by about 15%
(936 and 884 µs against 783 and 748), while the adaptive times are within noise (535 to 599 against 576 to 580). Above
0.98, f528836 is ahead: 1.07 to 1.85 against 1.11 to 2.07.

Order and execution, separated by the rows touched and clusters visited to each recall:
- **random-25 and region-25.** It is the order. To the same recall, f528836 touches 16 to 36% more rows (random-25) and 49 to
  79% more (region-25, recall 0.8 to 0.99). Every read is direct in both, and the time per row is about
  the same (21 to 30 ns; f528836's first run was 39 ns at 0.99).
- **region-1.** It is the execution. f528836 touches no more rows but visits 2 to 4 times the clusters (129 to 303
  against 71 to 79 at 0.95 and above). 00965ba raced a scan of the visible rows and scanned the rest once the probe would
  cost more (75 to 100% of its queries at psi 3 and above). f528836 reads the rest cluster by cluster, each paying a
  sigma and the stream's bookkeeping. At about 0.9 µs a cluster, reading every cluster costs 636 µs against the exact
  query's 292 to 320.

The order effect is the z(1, v) factor: it favours clusters with many visible rows. In the offline replay
(`study_order_rows_to_recall`: every order read whole, the same number of clusters for every query, 100 queries,
k 10), mu + z(1, v) sigma needs 1.12 to 1.22 times the rows of mu + 4 sigma on glove unfiltered and random-25, and 1.41
to 1.53 times under region-25.

The replay's two questions (`tables.txt`; rows / those of mu + z(1, v) sigma):
- **The free pooled spread (rad s) against sigma.**
  - (ii) mu + z(1, v) rad s needs 0.99 to 1.14 times the rows of (i) on glove, 1.03 to 1.21 on yahoo and 1.06 to 1.46
    on LAION (1.19 to 1.25 unfiltered, 1.23 to 1.46 under region-25). The rad s ratios below are on the same base.
  - mu alone needs 1.06 to 1.22 times unfiltered on glove and yahoo, and 1.43 to 1.83 on LAION. It needs fewer rows
    under the region sets (0.67 to 0.83 under region-1), but by reading many more, smaller clusters.
  - So sigma earns its cost: rad s is not within a few percent anywhere but glove, and on LAION it is a poor proxy.
    The gap order ranked by rad s needs 1.65 to 1.80 times the rows of (i) on LAION at 0.98 and above, unfiltered.
- **The gap order (mu − t) / sigma.** The running 10th best re-keyed after every read is within 4% of the oracle t
  everywhere. Against (i):
  - glove 0.64 to 0.89;
  - LAION 0.64 to 0.89 under random-1 and the region sets (0.95 to 1.08 under random-25);
  - yahoo region-1 0.80 to 0.85;
  - but yahoo unfiltered 1.03 to 1.14 and LAION unfiltered 1.03 to 1.11.
  The static mu + 3 sigma (no t, no re-keying) does as well or better: 0.85 to 0.91 on glove unfiltered, 0.97 to 1.01
  on yahoo and LAION unfiltered, and 0.63 to 0.94 under the region sets of all three. It never exceeds 1.04 (yahoo and LAION
  random-25 at 0.9).
  The gap order and mu + 3 sigma read more, smaller clusters: glove region-1 to 0.9, 201 to 206 clusters against 36.

**Verdict.**
- **Rows:** mu + 3 sigma is the cheapest order that needs no t, or ties with the gap order, on every dataset and
  filter. The nv factor costs rows everywhere it changes the order.
- **Compute per cluster:** both need each cluster's sigma, an r-dimensional int8 product with the query (sig_us 0.5 µs
  on glove, 1.2 on yahoo, 1.9 on LAION), as (i) does. Ranking all clusters by it means pricing every cluster
  (K sigmas, 0.3 to 1 ms a query), unless a stream and window stand in as now.
- **What the replay does not measure:** the per-cluster read cost, which is what the nv factor saves (fewer, larger
  clusters); and the window. Its rows favour mu + 3 sigma, but the time depends on that per-cluster cost.
- **00965ba's failure at high psi** fits the replay. Its stream was mu + psi rad s, and on LAION rad s is a poor proxy
  (above). At psi 5 to 7 the stream ranked mostly by it, while mu + 5 sigma itself costs only 1.03 to 1.12 times the
  rows of (i).

**The orders stress-tested** (`results/order-stress/`; `study_order_stress`, rows only, 100 queries, the same
whole-cluster replay). Datasets:
- glove, yahoo and LAION;
- Gaussian and Student-t(3) mixtures at d 100 and 512 (100 clusters of 2,000, matched covariances);
- two held-out sets never used before: llama-128-ip (257k rows, inner product, not normalised) and a 200k-row sample of
  yandex-200 (Text2Image, out-of-distribution queries, cosine).

Truth is brute force over the stored vectors. The region sets follow the suites' definition: 300 centres, here random
rows. The orders, with ratios given to the best order in each row (`worst.txt`, `summary.txt`):
- mu + c sigma at c 2 to 5 (00965ba's order at psi = c);
- mu + z(1, v) sigma (i);
- mu;
- spread;
- the gap (mu − t) / sigma at the running k-th best (B);
- Gbar((t − mu) / sigma) v / (v + 16) (D).

Findings:
- **mu + 3 sigma.** Leaving the in-cluster bottom sets aside, it is within 1.10 of the best in 97% of cells. Its worst
  cells are 1.29 (glove k 1 at 0.995) and 1.23 (llama region-25, k 100, at 0.9).
- **mu + 2.5 sigma** is within 1.10 in 96% of cells; (i) in 52%; mu alone in 16%; spread in 35%.
- **Where mu + c sigma breaks: rows sampled from the low tail of each cluster.** Under the in-cluster bottom 1% and
  25% sets, mu + 3 sigma needs 1.4 to 7.1 times the best's rows (LAION bottom-1 7.1, glove 4.3). (i) needs 1.3 to 3.9.
  mu alone is near the best there (1.00 to 1.5).
- **B fixes that** (1.00 to 1.25). Elsewhere it loses up to 1.48 (t3-100, k 100 at 0.999), 1.08 to 1.23 under
  LAION's top sets and 1.36 at k 1. It is within 1.10 in 86% of cells.
- **D does not fix it** (2.7 to 13 times under bottom-1). Its per-row probability saturates near 1 there, and the v/(v + 16)
  factor then ranks by cluster size.
- **No order is within 10% of the best everywhere.** Without the adversarial bottom sets, mu + 2.5 to 3 sigma come
  closest (worst 1.23 to 1.31).
- **The other conditions are mild for mu + 3 sigma.** Its worst cells:
  - score-correlated top and direction sets 1.08;
  - the region/random mix 1.06;
  - out-of-distribution queries 1.05 to 1.14 (negated at most);
  - outliers 1.20 (LAION, at 0.9);
  - merged neighbouring clusters 1.08;
  - synthetic heavy tails 1.15 (t3, d 100, k 100, at 0.999).
  The synthetic clusters are well separated: one or two clusters reach recall 0.995 unfiltered, so the orders differ
  there only under region-25.
- **Best c against k.** The best c falls with k on glove (unfiltered 5, 4, 4, 3 for k 1, 10, 100, 1000) and LAION
  (3.5, 3, 3, 2.5), as c_pred = Phi^-1(1 − k_c / nbar) does (3.3 to 2.0 to 2.6). It does not on yahoo (2 to 3.5,
  unordered), and on the synthetic sets it stays at 2 to 3.5 while c_pred falls to 0.9 to 1.8. The rows are flat in c:
  mu + 3 sigma is within 1.08 of the best c in every block but llama k 100 (1.10 to 1.11) and the bottom sets.
- **mu + 3 sigma's advantage over mu alone does not grow with k.** It shrinks on glove (1.36, 1.26, 1.19, 1.10 times
  the rows for k 1 to 1000) and LAION (2.15, 1.60, 1.51, 1.35), and is flat on yahoo (1.10 to 1.20). Bruch et al.
  report it widening, but in a regime with no filters and no stopping rule, measured otherwise.

**The covariance sketches** (`sketch.txt`; mu + 3 sigma with sigma from each sketch, rows over the exact covariance's,
recall 0.9 to 0.995; errors over the clusters the exact order reads):
- **Ours** (16 int8 directions and a scalar rest) is within a few percent of exact on glove (0.95 to 1.03), yahoo (0.99
  to 1.07) and yandex (1.01 to 1.09). It is not on LAION (1.02 to 1.13) or llama (1.08 to 1.25), where its sigma's
  median error is 0.11 to 0.13 and 0.61 to 0.63.
- **Bruch et al.'s** diagonal plus t eigenpairs is worse than ours everywhere but llama. At t = 4 it needs 1.03 to 1.48
  times exact's rows (LAION 1.20 to 1.48); at t = 8, 1.01 to 1.40.
- **The hybrid** (our directions plus a diagonal residual) closes llama (0.98 to 1.07). It does not close LAION (1.09
  to 1.23, worse than ours, although its sigma's median error is lower, 0.055 against 0.12).
- **Is the residual axis-aligned?** For the residual beyond our directions, the diagonal's median relative error
  against the scalar rest's:
  - LAION 0.22 to 0.25 against 0.47 to 0.58 (p90 0.41 to 0.47 against 1.6 to 2.6);
  - llama 0.57 to 0.59 against 3.4;
  - yandex 0.19 to 0.23 against 0.24 to 0.30;
  - worse on glove (0.16 against 0.04);
  - equal on yahoo (0.5 each).
  So the residual is axis-aligned enough to matter on LAION and on the unnormalised inner-product set.
- **Storage per cluster and multiply-adds per sigma, at d dimensions:**

  | Sketch | Storage | Multiply-adds per sigma |
  |---|---|---|
  | Exact | 2 d^2 bytes (the triangle in f32) | d^2 |
  | Ours | 16 d + about 136 bytes | 16 d |
  | Bruch t | 4 d (t + 1) + 4 t bytes | d (t + 2) |
  | Hybrid | 20 d + 136 bytes | about 33 d (the residual needs q − U'Uq) |

  At d = 512: exact 512 KB; ours 8.3 KB; Bruch t = 4 10 KB and t = 8 18 KB; hybrid 10.4 KB.

**The self-correcting order** (`results/order-self-correcting/`; the same harness, every stress condition, 100
queries). The orders:
- **SC:** mu + (delta + 3 rho) sigma. Delta and rho are the mean and spread of z = (s − mu_c)/sigma_c over the rows
  scored in full (every visible row of each cluster read), with the prior 0 and 1 until two rows are, re-keyed after
  every read.
- **SC-probe:** SC with moments only from the rows a probe scores in full: the clusters read before k rows are held,
  and those of at most 129 visible rows.
- **E2:** mu + (3 + delta_e) sigma. Delta_e is the shift that makes the model's count of visible rows above each
  read's t_c match the observed count, by Newton's method from 0, held within −3 to 3.

All are compared against the other orders in rows to recall 0.9 to 0.999. Results (`compare.txt`, `detail.txt`):
- **SC against mu + 3 sigma, where mu + 3 sigma is within 1.10 of the best:** median 1.00 in every condition. The worst
  cells are 1.01 to 1.10, and 1.14 at LAION unfiltered k 1 at 0.9. SC-probe has the same medians, with worst cells
  of 1.13 to 1.14 (LAION k 1; the synthetic Gaussian d 100 under region-25), so the estimate mostly survives with
  moments only from direct reads.
- **Under the in-cluster bottom sets:** SC is at most 1.29 of the best (median 1.16), against mu + 3 sigma's 7.14
  (median 1.91), B's 1.25 and mu's 1.50.
- **Overall:** SC is within 1.10 of the best in 73 to 100% of cells per condition (73% under outliers, as mu + 3 sigma). Mu + 3 sigma is at 73 to 100%
  (80% under the score sets), B at 69 to 100%.
- **On the held-out sets** SC's worst cell is 1.07 against mu + 3 sigma's 1.23. On llama it learns rho 0.60 to 0.63,
  correcting the stored sketch's overestimated sigma there.
- **E2 is weaker:** worst 1.40 against mu + 3 sigma at k 1, and 1.81 under the bottom sets. Delta_e from a few counts
  swings early (−1.4 to 0.7 under benign sets).
- **The estimates under benign sets** (none, random, region, the mix, out-of-distribution queries, outliers, merged
  clusters, the synthetic and held-out sets; per-block medians): delta −0.06 to 0.04 and rho 1.00 (0.86 to 1.12), but
  0.60 to 0.63 on llama.
  - Bottom sets: delta −1.2 to −2.8, rho 0.31 to 0.53.
  - Top sets: delta 1.2 to 3.2, rho 0.32 to 0.63.
  - Direction sets: in between.
- **Against the success criteria:**
  - Within about 1.3 of the best under the bottom sets: met (1.29).
  - Within about 1.02 of mu + 3 sigma wherever that order was good: met in the median (1.00) but not in the worst cells
    (up to 1.10; 1.14 at LAION k 1).

**LAION's hybrid sketch, by query distribution** (`sketch-laion.txt`; mu + 3 sigma, rows over exact's at 0.9 to
0.995). With LAION's text queries the hybrid loses to ours (1.09 to 1.23 against 1.02 to 1.13). With 100 stored image
rows as queries (each left out of its own truth) it ties or wins in every cell (1.02 to 1.17 against 1.03 to 1.20). The
out-of-distribution explanation holds in direction.

The sigma errors do not explain it:
- **Text queries:** ours 0.11 to 0.13, the hybrid 0.053 to 0.057.
- **Image queries:** ours 0.068 to 0.073, the hybrid 0.053 to 0.057.
- **The residual diagonal along the text queries' residual directions** is the more accurate one: 0.22 to 0.25 against
  the scalar rest's 0.47 to 0.58 (p90 0.41 to 0.47 against 1.6 to 2.6). Along the image queries' it is 0.41 to 0.44
  against 0.49 to 0.52.

So the hybrid's loss on text queries comes with a smaller sigma error, not a larger one. The rows depend on how the
errors fall across the clusters ranked, which these medians do not measure.

**The self-correcting k-NN order, built** (5853926; `results/self-correcting-knn/`). Only the order changed; the
planner (read choice, constants, groups) is f528836's.
- **Keys:**
  - order mu + lambda sigma, lambda = delta + 3 rho (`ORDER_C`);
  - stop: the window's largest mu + (delta + psi rho) sigma below the running k-th best;
  - stream mu + lambda rad s.
  z(1, v) is gone.
- **Moments.** Delta and rho come from the clusters read directly while fewer than k items are held, each scored alone
  in the direct batch. The batch keeps the scores' sum and sum of squares (two adds and a multiply a row, only in that
  phase), converted once per cluster to z moments. The prior is 0 and 1 until two rows are in; lambda is frozen once k
  items are held.
- **Frozen against SC-probe** (the replay, `frozen.txt`; glove, yahoo, LAION and llama, k 1 and 10, none, random-25,
  bottom-1, bottom-25, top-25): rows within 0.97 to 1.03, median 1.00. So the nv ≤ 129 clusters' moments after the
  fill add nothing.
- **Work per query** (`overhead.txt`, 50 queries):
  - stream re-keys: unfiltered 0 to 0.12, random-25 1, region-1 2.4 to 3.5 on average (at most 9);
  - each re-key a heapify of the clusters left in the stream (fewer than 592);
  - the window's eight keys re-computed at each read.
- **The threshold query does not use this order.** It reads by the bounds and mu + psi sigma at its own t.
- **Timed against 00965ba's library (7536990) and f528836, back to back** (`timing/summary.txt`; 50 queries,
  2 repetitions, psi 2 to 7 in steps of 1, so the ratios interpolate coarsely). Time to recall 0.8/0.9/0.95/0.98/0.99
  over spread's:

  | Set | 7536990 | f528836 | Self-correcting |
  |---|---|---|---|
  | glove none | 1.42/.82/1.92/2.32/2.61 | 1.58/.88/1.26/1.95/1.82 | 1.44/.81/1.41/1.58/1.38 |
  | glove random-25 | 1.08/.78/.56/1.09/1.23 | 1.12/.85/.60/.94/1.40 | .96/.74/.55/1.14/1.27 |
  | glove region-1 | .65/.67/.62/.56/.54 | .75/.84/.79/.73/.75 | .94/1.00/.92/.83/.81 |
  | LAION none | .63/.92/1.25/1.43/2.09 | .72/.97/.92/1.03/1.06 | .70/.92/.83/1.01/1.08 |
  | LAION random-25 | .40/.41/.61/1.13/- | .38/.38/.61/.51/.85 | .47/.37/.54/.56/.64 |
  | LAION region-1 | .52/.64/.66/.60/.58 | .50/.68/.65/.57/.55 | .53/.63/.64/.59/- |

  Glove region-1 is where the new order loses. Delta comes out at −0.6 there, which lowers both the order's lambda
  and the stop. To the same recall it reads more clusters than f528836 (306 clusters for 0.94 at psi 5, against 204 for
  0.99 at psi 3). 7536990's race to a scan remains the fastest under region-1.

**The scan hand-over** (46b41d3; `results/scan-handover/`). The probe hands the rest to a scan once its work W reaches
the visible rows left R. W is the rows scored from stored vectors plus r = 16 for each sigma. Every cluster left is
then read directly, in row order and batched (path "race"), and the answer is exact. There is no modelled cost.

**First version: W counted rows touched.** That counts every row in an index-path range, so the hand-over fired at
psi 5 in half of glove's unfiltered queries and slowed them (`timing-touched/`).

**Counting rows scored, timed against 7536990 and 5853926 back to back** (`timing/summary.txt`; 50 queries, 2
repetitions, psi 2 to 7). Time to recall 0.8/0.9/0.95/0.98/0.99 over spread's:

| Set | 7536990 | 5853926 | Hand-over |
|---|---|---|---|
| glove none | 1.57/.86/1.87/2.23/2.57 | 1.46/.80/1.44/1.63/1.43 | 1.43/.82/1.52/1.72/1.53 |
| glove random-1 | 1.11/.88/.61/1.23/.97 | 1.20/1.62/1.62/1.54/1.35 | 1.13/1.89/1.92/1.83/1.24 |
| glove region-1 | .64/.65/.62/.57/.56 | 1.00/1.10/1.00/.89/.86 | .79/.86/.81/.74/.72 |
| glove random-25 | .96/.68/.48/.98/1.10 | .98/.76/.54/1.15/1.27 | 1.10/.86/.63/1.54/1.73 |
| LAION none | .69/1.01/1.40/1.62/2.28 | .72/1.10/1.01/1.10/1.10 | .71/1.08/.89/1.02/.98 |
| LAION random-1 | 1.34/1.18/1.34/1.10/1.37 | 1.18/1.44/1.14/1.37/1.48 | 1.06/1.28/1.09/1.65/1.45 |
| LAION region-1 | .50/.64/.68/.61/.58 | .52/.64/.65/.59/- | .49/.64/.70/.59/- |
| LAION random-25 | .41/.41/.69/1.25/- | .47/.39/.58/.61/.68 | .42/.36/.51/.53/.57 |

Where it fires (`races.txt`):
- the 1% sets from psi 3 to 4 up (most queries by psi 5 to 6);
- unfiltered only at psi 6 to 7 (glove, 31 to 38 of 50 queries; LAION 4 of 50 at psi 7);
- random-25 from psi 5 on glove, psi 6 on LAION.

It helps glove region-1 (0.72 to 0.86 of spread, against 0.86 to 1.10) and the top of random-1 (0.995: 1.04 against
1.28). It costs glove random-25 at 0.98 to 0.99 (1.54 to 1.73, against 1.15 to 1.27). There the probe's long queries
already read most of the visible rows, so the hand-over doubles them, as its bound allows. It also costs glove
random-1 at 0.9 to 0.98. Glove region-1 still trails 7536990 (0.55 to 0.65), whose race priced the probe's per-cluster
overhead (a sigma, the order, a direct read's fixed cost).

**The shrunk estimate (B) was not built.** The replay (`replay/`, the stress harness with random-1, region-1 and
region-25 at k 10) shows the frozen self-correcting order needing no more rows than mu + 3 sigma on the 1% and region
sets. Glove region-1 at 0.9/0.99: 2,953/4,197 rows against 3,042/4,174; LAION 2,114/3,283 against 2,116/3,213. So the
order does not misfire on region in rows. The time it loses there is per-cluster work (about 200 small clusters read
one by one), which the estimate's reliability cannot change.

**Glove unfiltered at 0.98 to 0.99** (5853926 against spread, same session; `profiles/`):
- At recall 0.99 the probe touches 477k rows over 227 clusters (37 read directly, 190 on the index path) in 12.1 ms.
  Spread touches about 425k rows over about 210 clusters in about 8.4 ms (interpolated between nprobe 128 and 256).
- So 12% more rows and about 28% more time a row (25 against 20 ns).
- The profiles (medians) place the per-row gap:
  - Direct reads: whole clusters of about 2,000 rows scored from their stored vectors at about 30 ns a row, against
    the index path's about 20 ns. The measured constants price direct reads as cheaper there, often when the group
    is new.
  - The probe's per-cluster work: pricing the index path (cut_rows) 0.3 ms, groups 0.2 ms, sigmas 0.13 ms and the
    order 0.1 ms, about 7% at psi 5.
- The rest of the mean gap is the long tail of queries the stop lets run (mean 227 clusters against a median of
  about 155).
- No change: the cause is spread over the planner's read choice (fixed this round) and the stop's tail, with no
  single small fix.

**The scan hand-over was reverted** (71aa524; the PIN back at 5853926). It cost glove random-25 at 0.98 to 0.99 the
full twice its bound allows, and helped only part of the 1% sets.

**The stop without the shift (a) and small clusters straight into the batch (b), through the real code**
(`results/probe-variants/`; `study_probe`, the index's measured constants, 50 queries, k 10, psi 2 to 7).

(a) is the stop as the window's largest mu + psi rho sigma, with delta kept in the order only.
- It saves 4 to 12% of clusters on glove and LAION region-1 at recall 0.9 to 0.95.
- It breaks the bottom-set fix: under each cluster's bottom quarter it reads 156 clusters where 5853926 reads 6 (glove),
  and 47 where it reads 3 (LAION), at recall 1.0 in both.
- Dropped.

(b) sends a cluster of at most r = 16 visible rows to the direct batch, with no sigma, window slot or pricing. It is
read in stream order unless its stop key with rad s is below the k-th best.
- It first lost half the recall, because the moments counted a direct read whose rows were still in the batch (sums
  of 0). Moments are now taken only after a flush shows fewer than k items held.
- With that fixed, against 5853926 (clusters to recall 0.9/0.95/0.99, small ones included):

  | Set | 5853926 | (b) |
  |---|---|---|
  | glove region-1 | 281/318/360 | 197/269/327 |
  | LAION region-1 | 150/196/234 | 112/156/196 |
  | glove random-1 | 143/224/374 | 78/204/379 |
  | LAION random-1 | 89/135/324 | 49/99/236 |

- Rows fall alike, and the bottom sets are unchanged.
- Reading the small clusters whatever their stop key (b') is close: better on glove random-1 at 0.99, worse on
  region-1.
- (b) is built (3930562).

**Timed** (`results/probe-variants/timing/`): four builds back to back, 50 queries, 2 repetitions, psi 2 to 7. Time to
recall 0.8/0.9/0.95/0.98/0.99 over spread's:

| Set | 7536990 | f528836 | 5853926 | 3930562 |
|---|---|---|---|---|
| glove none | 1.57/.85/1.93/2.29/2.62 | 1.55/.89/1.18/1.72/1.57 | 1.44/.76/1.43/1.64/1.46 | 1.54/.87/1.56/1.83/1.61 |
| glove random-1 | .97/.77/.52/1.10/.92 | 1.38/1.08/1.41/1.77/1.29 | 1.09/1.48/1.61/1.60/1.35 | 1.12/.86/1.34/1.48/1.22 |
| glove region-1 | .65/.68/.63/.58/.57 | .71/.82/.78/.72/.76 | .79/1.02/.97/.85/.83 | .59/.59/.66/.65/.65 |
| glove random-25 | 1.14/.81/.60/1.08/1.21 | 1.58/1.24/.94/1.09/1.46 | .92/.70/.52/1.07/1.19 | 1.15/.85/.60/1.12/1.23 |
| LAION none | .69/.96/1.31/1.55/2.19 | .69/.98/.96/1.06/1.10 | .78/1.04/.88/1.03/1.04 | .71/.92/.86/.97/1.01 |
| LAION random-1 | 1.36/1.15/1.23/1.04/1.39 | 1.38/1.14/1.37/1.04/1.06 | 1.17/1.44/1.21/1.51/1.46 | 1.23/1.06/1.19/.97/1.06 |
| LAION region-1 | .53/.69/.73/.64/.60 | .45/.63/.62/.58/.57 | .51/.61/.66/.64/- | .46/.47/.52/.50/.53 |
| LAION random-25 | .41/.42/.71/1.22/- | .40/.43/.68/.55/.84 | .49/.39/.56/.58/.63 | .47/.37/.57/.56/.60 |

- The small-cluster batch brings glove region-1 level with 7536990 (0.59 to 0.66 against 0.57 to 0.68), puts LAION
  region-1 ahead of all three (0.46 to 0.53), and improves the 1% random sets on both datasets.
- Glove random-1 still trails 7536990 at 0.9 to 0.95.
- Unfiltered and random-25 are as 5853926, within the runs' noise.

**Next.**
- The order question, decided by the owner from the replay above: a fixed-multiplier order (mu + 3 sigma) against
  mu + z(1, v) sigma, and on sparse sets a hand-over to the scan once the probe's reads would cost more (as 00965ba's
  race did). Held this round: no library change.
- The legacy arm's removal once the benchmark no longer needs it.
- Inserts leave the directions as built until compaction.
