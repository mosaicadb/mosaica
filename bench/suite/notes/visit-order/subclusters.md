# Sub-clusters: what they cost to read, what layout (c) costs the exact queries, and what they buy the approximate k-NN

2026-11-19. Experiment on branch `subclusters-exp` (from `00965ba`), not merged; nothing here changes the library's
default behaviour. Code: `rust/src/v2/sub.rs` (new), the layout-(c) build option and the exact paths over runs in
`rust/src/v2/{mod,q,store}.rs`, the bindings' experimental methods in `bench/annb/vexact-py/src/lib.rs`. Scripts and raw
outputs: `subclusters/` (`results/<dataset>/`). Everything timed on core 3 of this machine (Xeon at 2.1 GHz, AVX-512,
`target-cpu=native`); builds on cores 0 to 2.

## Summary

Against the owner's rule (small or minimal impact on the exact and threshold queries, and clear approximate wins of
about 20% somewhere meaningful without losses elsewhere): **no, not as built.**

1. **Reading a sub-cluster** costs what a cluster of its rows costs when it is contiguous (layouts (b) and (c)): the
   same per row, plus the read's fixed part (1.5 us directly, 14 to 22 us on the index path, once per cluster). The
   best sub-cluster (7 to 8% of the rows) costs 0.10 to 0.13 of its cluster unfiltered, the best four (26 to 31%) 0.35
   to 0.42. **Masked (a)** pays a pass over the cluster's labels: 0.15 to 0.23 and 0.57 to 0.88. Under 1% sets a
   sub-cluster holds 2 to 4 visible rows and costs a third of its cluster.
2. **Layout (c) costs the exact queries 2 to 8% unfiltered (k-NN 5 to 8%), 3 to 17% under 25% sets (k-NN under
   random-25 9 to 17%), and nothing under 1% sets**, paired, with identical answers on all three datasets and the
   synthetic fixture. Its state is 17 offsets a cluster; the sorted range is searched in each of 16 runs and the
   kernel gathers the scattered candidates.
3. **The approximate k-NN with sub-clusters on layout (c)** (split by the measured costs, the sub-clusters whose key
   reaches the k-th best read in one read, their own spread) takes 0.65 to 0.82 of the library's time to recall 0.8 to
   0.95 on glove and yahoo unfiltered and on every dataset under 25% sets, 0.88 on LAION unfiltered; but 1.02 to 1.24
   under 1% sets on LAION (1 to 1.1 on glove and yahoo), and up to 1.07 at recall 0.99 unfiltered. Layouts (a) and (b)
   give no clear win. The policy as specified (sub-clusters read best-first, one read each) is no better than reading
   a cluster's chosen sub-clusters at once, and much worse at 0.99 and under 1% sets.

So the approximate win exists and is about the bar (20 to 35%) on dense visible sets, but only with layout (c), whose
cost to the exact queries is not minimal (up to 17% on random-25 k-NN), and it still loses under sparse visible sets
on LAION.

## Method

**Sub-clusters.** 16 per cluster, by k-means on each cluster's stored f16 rows (`sub::kmeans`: k-means++ seeding from
a seed per cluster, then at most 20 Lloyd iterations; the replay's faiss k-means used 10). It runs on the rows in key
order, so the masked layout (computed when the default index is opened) and layout (c) (computed by the build) hold
the same sub-clusters, entity for entity (`every_layout_holds_the_same_subclusters`). Per sub-cluster the approximate
query reads an int8 centre with a scale, its rows, its RMS radius about that centre, and (for the second spread tried)
the variances of its rows along its cluster's 16 stored directions and an isotropic rest.

**Layouts.**
- (a) **masked**: the default index; a label per row (4 bits would do for 16; held as a byte). A sub-cluster read
  passes over the cluster's rows (or its visible rows, or the rows of its sorted range on the index path) keeping those
  of its label (`KnnReader::visit_sel` with a mask).
- (b) **contiguous key ranges**: the default index; sub-cluster j is rows [j S/16, (j+1) S/16) of the cluster's key
  order. Nothing is reordered and no k-means runs; the sub-clusters are not coherent in space.
- (c) **contiguous k-means runs**: `BuildOptions::sub = 16` (`vexact --sub 16`, the bindings' `sub_build=16`) orders
  each cluster's rows by (sub-cluster, key); `Index2::subruns` holds 17 row offsets a cluster.

**Datasets and fits.** The suite's glove-100-angular (1.18M rows, K 592, n 128), yahoo-minilm-384-normalized (0.68M,
K 339, n 384) and laion-clip-512-normalized (1.0M, K 500, n 512, text queries), their fits from
`/home/user/suite-data/work` (symlinked; `subclusters/scratch.sh`), each built twice in `/home/user/scratch-sub` (the
default layout and layout (c); `build.py`). 100 queries, k = 10, the five visible sets of the suite (none, random-1,
random-25, region-1, region-25).

**Pairing.** Each script opens every index it compares in one process and gives each query to every cell in an order
shuffled per query and repetition (as `../scripts/ab_orders.py`); the first round measures recall, rows and answers and
is not timed. Times are the median over queries of each query's median. The approximate queries' cost model
(`adaptive::Measured`) is timed once per dataset on the default layout and copied into layout (c)'s cache, so both
layouts choose paths and races by the same constants and differ only in what their reads cost. Two copies of the same
index differ by up to about 10 to 15% in time on some cells (the `adaptive-c` arm below is the library's k-NN,
unchanged, on the layout-(c) copy: 0.83 to 1.10 of `adaptive` on the default copy); differences under about 10% are
not findings.

## 1. What reading a sub-cluster costs, by layout

`read_costs.py`: for each query, its 16 clusters of largest centre score holding a visible row; at the running k-th
best of a k-NN over them (the 10th best of their rows), each cluster read whole, its best sub-cluster (by sub-centre
score) alone, and its best four one after another, each directly and on the index path, each timing the fastest of 3
after reading twice the core's L2 (`sub::read_costs`). Below, each read at its cheaper path as a share of the whole
cluster's at its cheaper path (mean over 1,600 samples); the rows are the visible rows read.

| | rows: whole / best one / best four | (a) masked: one / four | (b) key ranges: one / four | (c) runs: one / four |
|---|---|---|---|---|
| glove, none | 2,606 / 210 / 795 | 0.23 / 0.88 | 0.11 / 0.41 | 0.13 / 0.42 |
| glove, random-25 | 651 / 53 / 200 | 0.18 / 0.54 | 0.11 / 0.34 | 0.12 / 0.38 |
| glove, region-25 | 613 / 42 / 176 | 0.19 / 0.62 | 0.11 / 0.35 | 0.13 / 0.39 |
| glove, random-1 | 26 / 3 / 9 | 0.43 / 0.87 | 0.30 / 0.66 | 0.34 / 0.69 |
| glove, region-1 | 22 / 2 / 8 | 0.47 / 0.75 | 0.35 / 0.56 | 0.37 / 0.59 |
| yahoo, none | 2,038 / 132 / 531 | 0.16 / 0.58 | 0.11 / 0.38 | 0.10 / 0.35 |
| yahoo, random-25 | 509 / 33 / 133 | 0.11 / 0.33 | 0.09 / 0.27 | 0.08 / 0.25 |
| yahoo, region-25 | 515 / 35 / 139 | 0.14 / 0.43 | 0.10 / 0.31 | 0.10 / 0.30 |
| yahoo, random-1 | 20 / 2 / 7 | 0.37 / 0.76 | 0.29 / 0.65 | 0.30 / 0.62 |
| yahoo, region-1 | 56 / 4 / 17 | 0.24 / 0.49 | 0.19 / 0.38 | 0.18 / 0.36 |
| LAION, none | 2,293 / 160 / 646 | 0.15 / 0.57 | 0.10 / 0.35 | 0.10 / 0.36 |
| LAION, random-25 | 575 / 42 / 167 | 0.09 / 0.32 | 0.07 / 0.23 | 0.08 / 0.25 |
| LAION, region-25 | 502 / 33 / 144 | 0.11 / 0.39 | 0.08 / 0.26 | 0.09 / 0.29 |
| LAION, random-1 | 23 / 2 / 8 | 0.34 / 0.72 | 0.27 / 0.60 | 0.28 / 0.58 |
| LAION, region-1 | 35 / 4 / 14 | 0.28 / 0.55 | 0.23 / 0.45 | 0.24 / 0.46 |

Least-squares fits of each read's time as fixed + per row (unfiltered; microseconds + nanoseconds a row):

| | whole, direct | whole, index | one, direct (a / b / c) | one, index (a / b / c) |
|---|---|---|---|---|
| glove | 1.2 + 16 | 16.5 + 16 (c: 20.4 + 19) | 3.0 + 32 / 1.3 + 20 / 1.7 + 18 | 18.5 + 43 / 13.8 + 20 / 14.7 + 22 |
| yahoo | 0 + 45 | 19.5 + 25 (c: 19.2 + 30) | 2.6 + 62 / 1.3 + 46 / 1.3 + 45 | 22.0 + 98 / 15.1 + 40 / 16.7 + 47 |
| LAION | 0 + 61 | 27.2 + 33 (c: 31.7 + 36) | 3.4 + 69 / 1.6 + 58 / 1.5 + 58 | 32.1 + 91 / 21.1 + 35 / 22.1 + 42 |

- **A contiguous sub-cluster (b, c) costs what a cluster of its rows costs**: the same per row as a whole read, plus
  the read's fixed part, about 1.5 us directly and 14 to 22 us on the index path (the cluster's per-query setup, which
  `visit_sel` keeps for the cluster's next sub-cluster). (b) and (c) are within noise of each other; (c) is 5 to 15%
  dearer on the index path (the candidates are scattered over 16 runs, so the kernel takes them by gather, below).
- **A masked sub-cluster (a) costs a pass over its cluster**: directly, 1.1 to 2 times the per-row cost (most on glove,
  whose rows are short: the label pass over 2,600 rows is a large part of reading 210 of them); on the index path 2 to
  2.5 times (the label test over the sorted range, and candidates by gather). The best four masked sub-clusters, 26 to
  31% of the rows, cost 0.57 to 0.88 of the whole cluster unfiltered, against 0.35 to 0.42 contiguous.
- **Under a 1% visible set a sub-cluster holds 2 to 4 visible rows**, and a read is mostly its fixed part: the best
  one costs 0.27 to 0.47 of the whole cluster, the best four 0.56 to 0.87. There is little to save there.
- Expanding a cluster (counting its sub-clusters' visible rows and scoring their 16 int8 centres, with the caches
  evicted, `SubState::e_us`) costs 0.55 (glove), 1.1 (yahoo) and 1.4 to 1.8 us (LAION): about one direct read's fixed
  part, or 10 to 30 rows' worth.

## 2. What layout (c) costs the exact queries

**Every place that assumes one key-sorted run per cluster**, and what layout (c) does there:
- *The sorted range* (`q::sorted_range2`; threshold queries, the exact k-NN's sweep, the adaptive threshold query's
  reads, `KnnReader::visit` on the index path). Split into `range_ivs` (the key intervals from the cut, once per
  cluster) and `range_in` (the two binary searches over a key-sorted row range); a cluster with runs gets
  `Pre::Runs`: the cut once and 16 pairs of binary searches, the rows each run's range takes in or leaves out, and the
  candidates of all 16 undecided runs (ascending) in one call of the kernel. `KnnReader::visit` reads a layout-(c)
  cluster through `visit_sel` over its runs, taking the largest keys over all runs when it fills the k best.
- *The level kernel* (`run_cluster`) takes any ascending candidate list; with 16 runs the candidates span most of the
  cluster, so it gathers them instead of loading them in place: part of the cost below.
- *The norm sort* sorts each cluster by residual norm instead of by key; it is not combined with layout (c) (the build
  refuses both; the default build is not norm-sorted).
- *The seed* (`seed2`) reads whole clusters at its level and ranks by interval midpoint; it does not depend on the order
  (exact ties among midpoints may be broken otherwise, which changes the seed's threshold, not the answer).
- *The support table* (each cluster's extremes against every anchor) and the per-cluster records are maxima and minima
  over the rows: unchanged.
- *Compaction* (`Index2::compact`): a run keeps the rows of it that are kept, in order; its offsets are remapped.
- *The selection* (`select_knn`) works on intervals and exact scores, in no order.
- *The order state* (the clusters' directions, from up to 4,096 rows spread over the cluster) and the encoder's
  calibration (a sample spread over the rows) are computed on the rows in key order, so they are the default layout's
  bits; the codes of each row are its own.
- *Disk mode* (vectors on file, `plan2_priced`): spans are summed over the runs; candidates in 16 runs need more read
  extents. Not timed here.

**Correctness.** `sub::tests::layout_c_answers_as_the_default_layout` (synth4k, 9 queries, 7 visible sets, every plan,
and both layouts compacted to each set's rows): k-NN at k 1, 10 and 100 gives the same entities and scores, threshold
queries at six thresholds the same entities, with every decided row's bounds checked against its vector (`verify`, no
violation). On the real data every answer of `ab_exact.py` below was identical on both layouts (0 differences in
3 datasets x 5 visible sets x 100 queries x every query kind). The default layout's exact paths are bit for bit as at
`00965ba`: `e2e::the_exact_paths_are_unchanged` passes.

**Extra state.** `subruns`: 17 offsets of 4 bytes a cluster (34 kB at 1M; about 1 MB at 1B and 15,625 clusters), a
build option, and the sub-cluster labels at build time. An insert goes to its nearest sub-centre's run, at its key's
place (the default layout places it at its key's place in the cluster).

**Time** (`ab_exact.py`, 5 repetitions; the paired ratio (c) / default, median over queries, with its quartiles):

| | k-NN, k 10 | threshold 0.01% | threshold 1% | threshold 10% |
|---|---|---|---|---|
| glove, none | 1.081 [1.05, 1.12] | 1.079 | 1.054 | 1.037 |
| glove, random-25 | 1.167 [1.13, 1.22] | | 1.116 | |
| glove, region-25 | 1.093 [1.06, 1.13] | | 1.066 | |
| glove, random-1 | 1.002 | | 1.002 | |
| glove, region-1 | 0.992 | | 0.979 | |
| yahoo, none | 1.058 [1.03, 1.09] | 1.052 | 1.026 | 1.016 |
| yahoo, random-25 | 1.141 [1.10, 1.20] | | 1.067 | |
| yahoo, region-25 | 1.051 [1.02, 1.10] | | 1.045 | |
| yahoo, random-1 | 0.983 | | 0.984 | |
| yahoo, region-1 | 0.955 | | 0.959 | |
| LAION, none | 1.051 [1.03, 1.08] | 1.053 | 1.023 | 1.017 |
| LAION, random-25 | 1.091 [1.06, 1.11] | | 1.064 | |
| LAION, region-25 | 1.043 [1.02, 1.07] | | 1.030 | |
| LAION, random-1 | 0.995 | | 0.999 | |
| LAION, region-1 | 0.970 | | 0.966 | |

(Under a visible set the prep has one threshold, at 1% of the visible rows. Default-layout times, unfiltered k-NN:
19.7 ms glove, 16.0 ms yahoo, 40.0 ms LAION; random-25 16.5, 13.4 and 38.2 ms; region-25 12.1, 10.3, 25.2 ms;
the 1% sets 0.33 to 0.90 ms.)

- **Layout (c) costs the exact queries 2 to 8% unfiltered, 3 to 17% under a 25% visible set (worst on random-25, and
  on k-NN), and nothing under a 1% set** (the planner scans those visible rows before cutting anything). The cost falls
  with the answer's size (the share of the time spent in sorted ranges and levels falls). Its source is the index-path
  read of a cluster: in section 1 it costs 10 to 20% more on layout (c) (about 4 us more fixed for the 16 pairs of
  binary searches, and the gathered candidates).
- Not tried: running the kernel once per run with the cluster's setup kept, which would load each run's candidates in
  place; it would trade the gather for 16 kernel calls a cluster.

## 3. What sub-clusters buy the approximate k-NN, in time

**The query** (`sub::knn_sub`, the bindings' `knn_sub`). The library's adaptive k-NN (`Adaptive::knn`: the stream by
the isotropic key, 8 clusters priced ahead, the stop when the window's largest key is below the running k-th best, the
race against a scan) with one change: a cluster taken from the window is **split** when the part of its whole read a
split could save (its cheaper path's cost less the least fixed cost of any read, `dir_us` or `cl_us`) exceeds what
expanding it costs (`e_us`, measured at open): the theory's rule (`hierarchy/theory.md` 3.2: a node holding no more
than its expansion costs is read whole), with the measured costs and no new constant. A split cluster's sub-clusters
holding a visible row get the key mu_s + psi sigma_s (mu_s the int8 sub-centre's score), and those whose key reaches the
running k-th best are read:
- `bestfirst` (as asked): they join a heap beside the window; the probe reads the largest key of the window and the
  heap, one sub-cluster a read, and stops when both are below the k-th best;
- `filter`: all of them at once, in one read of the cluster (`Sel` with their ranges, or their labels under a mask);
  the others never. If that is every sub-cluster, the cluster is read whole;
- `unsplit` (tried on glove only, 15 queries, 1 repetition): `bestfirst`, unless reading them one by one would cost
  as much as the whole cluster. Early in a query every sub-cluster's key reaches the k-th best, so it rarely splits: a
  quarter of the clusters read unfiltered, 3% under random-25, none under random-1; its times fell between
  `adaptive`'s and `bestfirst`'s. Not run further.

Each read (one sub-cluster or a set) is priced as a cluster read of its visible rows (the index path's per-cluster
part once per cluster, its per-row part over the rows it passes); the race scans every row not read. Tests:
`knn_sub_is_exact_at_large_psi` (every layout, mode and spread; with every cluster split by force too; and raced after
splits, exact), `subcluster_reads_find_what_whole_reads_find`.

**The spread of a sub-cluster.** `scaled`: sigma_s = sigma_c(q) rad_s / rad_c, the cluster's spread from its stored
directions scaled by the radius ratio: nothing beyond the radius is stored. `own`: the sub-cluster's own variances
along its cluster's 16 directions and its rest, sigma_s^2 = sum_d lam_sd p_d^2 + rest_s (|q|^2 - sum_d p_d^2), with the
query's projections p already computed for sigma_c: 16 more floats a sub-cluster (64 B), 16 multiply-adds.
What I tried before (glove, 15 queries): the expansion cost taken as 16 int8 products at the warm `i8_us` (0.08 to
0.3 us) and compared with the whole read's cost, fixed part included: every cluster under random-1 was expanded (20
visible rows cost 2.3 us to read, nearly all of it the read's fixed part, which a split cannot save), and the query
took 1.5 to 1.9 times `adaptive`'s time there, about 2 us a cluster in cold misses on the sub-centres and their
statistics. The rule now leaves the fixed part out and times the expansion cold (`e_us`).

**Arms** (`ab_sub.py`, 3 repetitions, psi 2.5 to 7 (LAION to 8), nprobe 4 to 512): `spread` (the legacy arm),
`adaptive` (the library's), `adaptive-c` (the library's on the layout-(c) copy), and `knn_sub` as
`sub-<layout>[-own]-<mode>`. Time to recall by linear interpolation over the grid (`../scripts/ab_curves.py`), as a
ratio of `adaptive`'s; `-` where the grid's largest psi does not reach it.

**Time to recall against `adaptive` (the library's k-NN on the default layout)**, at recall 0.8 / 0.9 / 0.95 / 0.99
(below 1 is faster; full tables with microseconds in `subclusters/results/summary_vs_adaptive.txt`, against
`adaptive-c` in `summary_vs_adaptive_c.txt`, raw points in `results/<dataset>/ab_sub.txt`):

*The three layouts under one policy* (`filter`, `scaled` spread):

| | (a) masked | (b) key ranges | (c) runs |
|---|---|---|---|
| glove, none | 0.98 / 1.00 / 1.09 / 1.08 | 0.98 / 0.99 / 1.00 / 1.07 | 0.80 / 0.80 / 0.87 / 0.88 |
| glove, random-25 | 1.21 / 1.09 / 0.93 / 0.89 | 1.13 / 1.07 / 0.97 / 1.02 | 0.93 / 0.83 / 0.72 / 0.71 |
| glove, region-25 | 1.17 / 1.07 / 1.00 / 0.98 | 1.04 / 1.04 / 1.01 / 1.05 | 0.79 / 0.81 / 0.78 / 0.73 |
| yahoo, none | 0.96 / 1.01 / 1.09 / - | 1.02 / 1.07 / 1.06 / - | 0.83 / 0.94 / 0.89 / - |
| yahoo, random-25 | 0.96 / 0.99 / 0.98 / - | 1.02 / 1.07 / 1.06 / 0.98 | 0.87 / 0.85 / 0.85 / - |
| yahoo, region-25 | 0.97 / 0.98 / 0.99 / - | 1.07 / 1.04 / 1.09 / - | 0.87 / 0.88 / 0.89 / - |
| LAION, none | 1.08 / 1.13 / 1.04 / 1.13 | 1.13 / 1.10 / 1.08 / 1.05 | 1.01 / 0.92 / 0.88 / 1.06 |
| LAION, random-25 | 0.99 / 0.84 / 0.84 / 0.82 | 1.11 / 0.98 / 0.98 / 0.88 | 0.81 / 0.72 / 0.64 / 0.80 |
| LAION, region-25 | 0.90 / 0.83 / 0.86 / 0.93 | 0.99 / 0.99 / 0.97 / 1.00 | 0.80 / 0.74 / 0.78 / 0.88 |

*Layout (c), the policies and spreads*, with the legacy arm and the library's k-NN on the layout-(c) copy:

| | `filter`, `scaled` | `filter`, `own` | `bestfirst`, `scaled` | `spread` (legacy) | `adaptive-c` |
|---|---|---|---|---|---|
| glove, none | 0.80 / 0.80 / 0.87 / 0.88 | 0.69 / 0.75 / 0.82 / 0.86 | 0.80 / 0.89 / 0.91 / 1.12 | 1.48 / 1.76 / 1.33 / 0.58 | 1.03 / 1.03 / 0.98 / 0.96 |
| glove, random-25 | 0.93 / 0.83 / 0.72 / 0.71 | 0.77 / 0.76 / 0.70 / 0.74 | 0.98 / 0.97 / 0.86 / 1.11 | 3.18 / 3.58 / 3.72 / 1.88 | 0.96 / 0.92 / 0.88 / 0.84 |
| glove, region-25 | 0.79 / 0.81 / 0.78 / 0.73 | 0.71 / 0.72 / 0.69 / 0.73 | 0.91 / 0.78 / 0.70 / 1.63 | 2.64 / 2.32 / 2.19 / 1.45 | 0.93 / 0.86 / 0.83 / 0.88 |
| yahoo, none | 0.83 / 0.94 / 0.89 / - | 0.69 / 0.77 / 0.77 / - | 0.62 / 0.77 / 0.91 / - | 1.68 / 1.46 / 1.19 / - | 1.05 / 1.10 / 1.10 / - |
| yahoo, random-25 | 0.87 / 0.85 / 0.85 / - | 0.80 / 0.71 / 0.76 / - | 0.87 / 0.89 / 0.92 / - | 3.37 / 3.48 / 3.13 / 2.20 | 0.96 / 1.01 / 0.99 / 1.00 |
| yahoo, region-25 | 0.87 / 0.88 / 0.89 / - | 0.81 / 0.82 / 0.94 / - | 0.87 / 0.93 / 0.98 / - | 2.75 / 2.83 / 2.97 / - | 0.95 / 0.96 / 1.00 / - |
| LAION, none | 1.01 / 0.92 / 0.88 / 1.06 | 1.01 / 0.88 / 0.88 / 1.07 | 0.75 / 1.01 / 1.09 / 1.88 | 1.70 / 1.46 / 1.31 / 0.36 | 1.20 / 1.10 / 0.99 / 1.04 |
| LAION, random-25 | 0.81 / 0.72 / 0.64 / 0.80 | 0.74 / 0.72 / 0.65 / 0.83 | 0.76 / 0.72 / 0.67 / 1.27 | 4.04 / 3.31 / 2.15 / 0.65 | 0.95 / 0.96 / 0.96 / 0.97 |
| LAION, region-25 | 0.80 / 0.74 / 0.78 / 0.88 | 0.81 / 0.80 / 0.76 / 0.92 | 0.76 / 0.76 / 0.77 / 1.08 | 2.21 / 1.64 / 1.06 / 0.71 | 0.97 / 1.04 / 0.90 / 1.00 |

`adaptive`'s own time to recall 0.9, us: glove 1,290 (none), 737 (random-25), 1,336 (region-25), 134 (random-1), 352
(region-1); yahoo 641, 261, 480, 80, 261; LAION 2,137, 1,054, 3,193, 200, 579.

*The 1% visible sets.* The first run (all arms, `results/<dataset>/ab_sub.txt`), and a second one after `knn_sub`
kept its priced clusters' projections in flat arrays instead of a hash map with a vector each
(`results/<dataset>/ab_sub_sparse.txt`, its own `adaptive` arm; the 25% and unfiltered rows above are from the first
code, which only did more fixed work per priced cluster):

| | first run: (a) / (c) `filter` / (c) `filter` `own` / (c) `bestfirst` | second run: (a) / (c) `filter` / (c) `filter` `own` |
|---|---|---|
| glove, random-1 | 1.22 1.26 1.27 1.06 / 1.12 1.07 1.05 1.01 / 1.15 1.12 1.11 1.05 / 1.08 2.66 3.20 1.06 | 1.13 1.08 1.06 1.05 / 1.11 1.09 1.09 1.01 / 1.05 1.03 1.02 1.02 |
| glove, region-1 | 1.21 1.10 1.09 1.07 / 1.07 1.04 1.04 1.02 / 1.14 1.06 1.05 1.01 / 2.46 1.36 1.08 1.05 | 1.10 1.01 1.02 1.05 / 1.14 0.96 0.95 1.00 / 0.93 0.94 0.97 1.00 |
| yahoo, random-1 | 1.17 1.18 1.21 1.18 / 1.11 1.12 1.08 1.10 / 1.08 1.18 1.12 1.21 / 1.08 1.18 1.19 2.07 | 1.10 1.07 1.08 1.03 / 0.95 0.98 0.99 0.99 / 0.97 0.97 1.00 1.04 |
| yahoo, region-1 | 1.13 1.11 1.11 1.10 / 1.00 1.01 1.02 1.04 / 1.03 1.02 1.10 1.06 / 1.00 1.00 1.34 1.08 | 1.04 1.05 1.08 1.07 / 0.96 0.99 1.03 1.01 / 0.95 0.98 1.06 1.02 |
| LAION, random-1 | 1.24 1.25 1.16 1.10 / 1.19 1.08 1.17 1.05 / 1.22 1.14 1.18 1.06 / 1.29 1.17 1.13 1.04 | 1.07 1.06 1.16 1.06 / 1.07 1.06 1.17 1.06 / 1.13 1.09 1.24 1.06 |
| LAION, region-1 | 1.09 1.18 1.17 1.08 / 1.01 1.10 1.09 1.02 / 1.04 1.11 1.09 1.03 / 1.01 1.01 1.05 1.02 | 1.09 1.15 1.12 1.06 / 1.06 1.10 1.07 1.02 / 1.04 1.08 1.05 1.03 |

(Each cell: recall 0.8, 0.9, 0.95, 0.99. `adaptive-c` in the second run: 0.94 to 1.02.)


**Findings.**
- **Only layout (c) wins, and only where the visible set leaves clusters dense.** With every sub-cluster read
  contiguous and coherent, `knn_sub` (`filter`, `own`) reaches recall 0.8 to 0.95 in 0.69 to 0.82 of `adaptive`'s
  time on glove and yahoo unfiltered, 0.65 to 0.80 under random-25 on all three, 0.69 to 0.94 under region-25, and
  0.88 on LAION unfiltered at 0.9 and 0.95 (1.01 at 0.8; against the same copy of the index, `adaptive-c`, 0.85 / 0.80
  / 0.89). At 0.99 it is 0.74 to 0.92 under 25% sets and 0.86 to 1.07 unfiltered.
- **Masked sub-clusters (a) lose most of it** (0.82 to 1.21; a clear win only on LAION random-25 and region-25 at 0.9
  and 0.95, 0.82 to 0.86): each read passes over the cluster's labels, which section 1 measures at about the cost of the rows saved.
  **Key ranges (b) gain nothing** (0.97 to 1.13 at 0.8 to 0.95): they are not coherent in space, so the keys of their parts barely
  separate them.
- **The sub-cluster's own variances (`own`) beat the scaled cluster spread** by 5 to 18% of the time on glove and
  yahoo (rows: 3 to 27% fewer at the same recall), and tie on LAION. 16 floats more a sub-cluster.
- **`bestfirst`, the policy as specified, is about as good as `filter` at 0.8 to 0.95 on dense sets (better on yahoo
  and LAION unfiltered at 0.8: 0.62 and 0.75), and far worse at 0.99 (1.08 to 1.88) and under 1% sets (up to 3.2
  times)**: each
  sub-cluster read pays its own fixed part (about 1.5 us directly, as much as 25 to 100 rows), and at a high psi most
  of a split cluster's sub-clusters are read one at a time. `filter` reads the same sub-clusters a cluster at a time.
  It does not read them best-first across clusters: a sub-cluster whose key is below the k-th best when its cluster is
  opened is never read, one above it is read then.
- **Under 1% visible sets it is a loss**: 1.02 to 1.24 on LAION, about 1 to 1.1 on glove and yahoo after the second
  run's change, with `adaptive-c` at 0.94 to 1.02 (so not the copy). Clusters hold 20 to 60 visible rows there; on
  LAION random-1 at psi 4 the rule expands about a third of the clusters it takes (36 of 102: a cluster of 23 visible
  rows has a variable cost of 1.3 us against the expansion's measured 1.35 us, so the larger ones pass), half of those
  turn out to need every sub-cluster, and the rest save a few rows for an expansion's 1.4 us. The theory's rule is a necessary condition only; its full form weighs the
  rows a split is expected to skip, which `knn_sub` does not estimate.
- Against the legacy arm (`spread`): `spread` takes 1.1 to 4 times `adaptive`'s time at 0.8 to 0.95 (most under the
  25% sets) and so more than every (c) arm with `filter`; at 0.99 it is faster unfiltered and on LAION's random sets
  (0.36 to 0.71 of `adaptive`), and `knn_sub` does not change that (0.86 to 1.07 of `adaptive` there).

## The state each layout needs

| | layout state | approximate state | exact paths | build |
|---|---|---|---|---|
| (a) masked | a label per row: 0.5 byte (16 sub-clusters); 0.5 MB at 1M, 0.5 GB at 1B | per sub-cluster: int8 centre (n B) and scale, rows, radius (`scaled`), + 64 B of variances (`own`): 16 (n + 16) B a cluster (`scaled`), 4.8 MB on LAION 1M (n 512); about the order state's size | untouched | k-means per cluster (6.5 s glove, 11 s yahoo, 24 s LAION at open on one core; at build, in parallel) |
| (b) key ranges | none (16 equal splits) | as (a) | untouched | none |
| (c) runs | 17 offsets a cluster (34 kB at 1M) and the build option | as (a) | sorted range per run (above) | k-means per cluster, rows reordered |

## Recommendation

**Do not adopt sub-clusters now.** Of the three layouts only (c) gives the approximate wins, and it fails both halves of
the rule as built: it slows the exact k-NN by 5 to 8% unfiltered and 9 to 17% under random-25 (threshold queries 2 to
12%), and the approximate query with it loses 2 to 24% under 1% visible sets on LAION. Masked sub-clusters (a), the
layout that leaves the exact paths alone, do not pay for their label pass at 1M (wins of 14 to 18% on LAION's 25% sets
only, losses up to 27% elsewhere). Key ranges (b) do not help.

What would change the answer, in order of cost:
- **The exact cost of (c)**: run the level kernel on each run's candidates loaded in place, with the cluster's setup
  kept between runs (not tried), and measure whether the 4 to 17% falls within noise. If it does not, (c) stays out.
- **The 1% losses**: the split rule as implemented is the theory's necessary condition; the full rule (expand only
  when the rows it expects to skip are worth more than the expansion) needs an estimate of the skipped rows, for
  instance from the sub-clusters' visible counts and the stream key's margin over the k-th best. Alternatively, no
  sub-clusters for a query whose clusters average few visible rows, as H took the spread key below 8k rows.
- **Scale**: the theory expects the gain to grow with the cluster size (setup amortised; `hierarchy/theory.md` 4.2).
  Run the same arms at `--per-cluster 8000` on one dataset before deciding sub-clusters are not worth it at 1B.

If sub-clusters are taken up later, take them as layout (c), read by `filter` with the `own` spread, not best-first.

## Deviations and limits

- The default layout's index (and the library's `adaptive` arm) is this branch's build, not the suite's saved index:
  the library's fingerprint changed, so both layouts were built here from the same fits.
- The approximate threshold query with sub-clusters was not built (time).
- 100 queries; the grids interpolate between psi steps of 0.5 (recall jumps of up to 0.2 between them on glove, which
  the interpolation smooths). Recall 0.99 was not reached by psi 7 on yahoo unfiltered and region-25 by any arm.
- `e_us` is timed with the caches evicted; a query that has just read a neighbouring cluster finds part of the
  sub-centres in the shared cache.
- The measured cost model sends almost every read on glove and yahoo to the direct path (their measured index path
  costs more than direct reads at their row lengths), so the index-path cost of sub-cluster reads matters on LAION only.
- Tests run on the default build only (AVX-512 here; AVX2 not built): `cargo test --release --lib`, 111 passed
  (`subclusters/results/tests.txt`), among them `e2e::the_exact_paths_are_unchanged` and the four `sub::tests`.
- The 25% and unfiltered rows of section 3 were timed before `knn_sub` dropped its hash map of priced clusters (the
  second run of the 1% sets); that change only removes fixed work, so those rows understate `knn_sub` slightly.
- LAION's ab_sub run was stopped by a time limit after three visible sets; region-1 and region-25 were run again in a
  second process with the same arms and appended to `results/laion-clip-512-normalized/ab_sub.txt`.
- `ab_curves.py` labels the interpolated points `x` against the first arm (`spread`); `summary.py` takes the ratios
  against `adaptive` (or `adaptive-c`).
