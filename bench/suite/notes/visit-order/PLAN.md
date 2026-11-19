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
against exact `knn2` about 17 ms; LAION ψ 6 recall 0.980 at 231k rows against nprobe 64's 0.977 at 132k). Three
changes. (1) ψ moves only the stop: the order is μ + z(1, S)σ, z(1, S) = Φ⁻¹(S/(S + 1)) of the cluster's S rows
(3.3 at 2,000; all its rows, not its visible ones, as the multiplier is a property of the budget, `hierarchy/
theory.md` 3.4), the stream μ + z(1, S)·rad·√(qᵀC̄q / tr C̄), and the stop the window's largest μ + ψσ below the
running k-th best; a smaller ψ reads the first of what a larger one does. (2) The index path is priced at the
running k-th best t from the rows its sorted range at t leaves and the share of them expected above t,
Ḡ((t − μ)/σ), with the measured cost cl + size·S + (vis + und·share)·rows (`und_us` fitted on clusters spread over
the centre-score ranking, not only the first 16); the sorted range itself drops almost nothing on glove (it keeps
91 to 99% of a cluster's rows at the true 10th best, the far clusters included), so the saving is the levels'.
The timing now flushes the cluster's stored vectors from every cache level before each read (it had evicted the
core's L2 only, and the rows came from L3): the measured row cost about doubled (glove 0.019 → 0.033 µs, LAION
0.055 → 0.116), as the owner's timing implied (glove ψ 7 modelled 21 ms, timed 35). (3) The probe races the exact
query too: knn2's cost as its planner would run it, priced by the measured constants at the running k-th best,
bought once the probe has spent what pricing it costs; past the cheaper of that and the scan, it hands over (path
"exact"), so the whole is at most about twice the cheaper. ψ = ∞ is `knn2`.

By explain_approx counts (30 queries, k 10; rows touched, the index path's included; clusters read, direct /
index): glove unfiltered ψ 3.5 / 4 / 4.5 / 5: 0.807 at 48k (18.5: 3.7 / 14.8), 0.947 at 166k (69.5), 0.987 at
362k (160: 15 / 145; 20k rows read from stored vectors), 0.997 at 644k (301); ψ 6 hands 6 of 30 to the exact
query, ψ 7 24 of 30. Before: ψ 4.5 0.987 at 284k (137, all direct), ψ 5 0.997 at 528k; nprobe 128 0.970 at 257k,
256 1.000 at 513k. LAION unfiltered ψ 4.5 / 5 / 5.5 / 6 / 7: 0.907 at 52k (24.7), 0.960 at 100k (48), 0.980 at
160k (77.6: 3.1 / 74.5), 0.993 at 268k (132), 1.000 at 555k (278); before ψ 6 0.980 at 231k (127), ψ 7 0.993 at
451k (244); nprobe 64 0.977 at 132k, 128 1.000 at 257k. Random-25: glove ψ 3.5 / 4 0.890 at 29k, 0.990 at 75k
(nprobe 256 0.987 at 128k); LAION ψ 4.5 / 5 / 6 0.933 at 27k, 0.973 at 48k, 0.993 at 115k (nprobe 64 0.967 at 33k,
256 0.997 at 126k), all read directly. So the decoupled order cuts LAION's rows at 0.98 by 31% but still reads
more than nprobe at the top, and reads more than the coupled order on glove at 0.987 (362k against 284k), where a
larger multiplier ordered better; most of glove's rows now go to the index path.

**Next.**
- High recall: above ψ ≈ 5 the time grew far faster than recall in the owner's benchmark, and at the top the
  approximate k-NN was slower than the legacy arm and than exact k-NN. Being fixed and timed: the order made
  independent of ψ (ψ moves only the stop), the index path priced from the gap to the running k-th best, and a race
  against the exact query (at most about twice its cost; ψ = ∞ is the exact query).
- The legacy arm's removal once the benchmark no longer needs it.
- Inserts leave the directions as built until compaction.
