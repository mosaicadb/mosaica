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

At scale the number of clusters grows as about √N (`fit.py`: 2,000 items per cluster up to 2M, doubling for each 4×
N): about 7k clusters at 100M and 22k at 1B. There q·m over every cluster reads 11 MB of int8 centres per query, H r4
over every cluster 90 MB and H r16 360 MB, so the spread can only be computed over a pre-filtered set of clusters.

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

Runs on this machine (4 cores, timing on core 3), the `order` suite (`suites.py`: glove-100-angular,
yahoo-minilm-384-normalized, laion-clip-512-normalized; 100 queries; approximate k-NN at k = 10, nprobe 4 to 256,
unfiltered and under random-1 and random-25). `results/order-baseline/` is phase 0, `results/order/` phase 1 (the
settings `radius` and `spread` on one build).

**Phase 0: where the time goes** (`scripts/profile_phases.py`; `results/phases-0-baseline.txt`). Under a sparse
visible set the order is nearly the whole query: at random-1 and nprobe 4, 62 of 72 µs on glove, 79 of 94 on yahoo,
184 of 200 on LAION. Unfiltered it is 13 to 18% at nprobe 4. Inside it, q.m of every cluster cost 37 µs (glove) to
137 µs (LAION), the sort 7 µs, the query's preparation 2 µs.

**Phase 1.**
- `util::dot64_32` (and `dot64`) were vectorised two lanes wide by the compiler. Written for AVX-512 with the same
  operations in the same order (a multiply and an add per lane, not fused), the result is bitwise the same and 3.2×
  faster: q.m of every cluster 125 → 40 µs on LAION, 37 → 13 on glove. It serves the exact paths too
  (`results/phases-1-dot.txt`).
- The order ranks every cluster by f32 products (`q::dot32`, 18 µs on LAION) and computes q.m in f64, which the
  bounds take, for the probed clusters only. The anchor stays the nearest of all (computed when a cluster on the index
  path first asks for it): the nearest among the probed clusters gave looser bounds and cost about 15% unfiltered.
- The `spread` order (f′ = 0.4, `OrderKind::Spread`) is the bindings' default; `radius` (today's, fitted) stays as an
  option, and `fit_queries` fits f for either.

With `radius` (the same order as before), a query at random-1 and nprobe 4 now takes 36 µs on LAION (was 198), 27
on glove (70) and 32 on yahoo (102); the order is 26, 15 and 19 µs of those (`results/phases-1-f32-order.txt`, which
profiles `spread`). Time to reach a recall (`results/phase1-orders.txt`, by `scripts/compare_runs.py`), baseline /
radius / spread, in µs:

| | recall 0.8 | 0.9 | 0.95 |
|---|---|---|---|
| LAION, unfiltered | 2961 / 2776 / 1457 | 5588 / 5121 / 2318 | 10248 / 9363 / 4701 |
| LAION, random-1 | 363 / 188 / 88 | 527 / 367 / 188 | 699 / 545 / 317 |
| LAION, random-25 | 2805 / 2480 / 1279 | 5710 / 5205 / 2561 | 10910 / 10154 / 5475 |
| glove, unfiltered | 789 / 785 / 793 | 1819 / 1632 / 1771 | 3145 / 2980 / 3396 |
| glove, random-1 | 99 / 53 / 53 | 150 / 106 / 111 | 226 / 186 / 208 |
| yahoo, unfiltered | 672 / 698 / 649 | 995 / 1015 / 1045 | 1498 / 1517 / 1514 |
| yahoo, random-1 | 132 / 32 / 32 | 157 / 53 / 51 | 228 / 114 / 115 |

`radius` returns the same recall as the baseline in every cell (the same order). On LAION's text queries `spread`
halves the time to every recall, on top of the faster order; on glove it loses about 0.008 recall at a fixed nprobe
(as the replays found, within 0.01) and is up to 14% slower to recall 0.95 unfiltered, faster under random-25; on
yahoo they tie. Single medians differ by up to about 10% between runs of the same work at these sizes, so
differences below that are not findings.

**Next.** Unfiltered, the order is now 3 to 6% of a query and the probed clusters' own work is the rest, so H's
extra cost (phase 2) has to buy recall at a fixed nprobe; under sparse filters the order is still about two thirds of
a query (26 of 36 µs on LAION), where only cheaper ranking helps: the f16 or int8 centres (9 µs in `microbench/`) and
then the hierarchy of phase 2b.
