# ANN-Benchmarks: glove-100-angular and nytimes-256-angular

The fast-scan index (`rust/`, `--v2`) on an AMD Ryzen 9 5900X under WSL2 (AVX2, 6 cores / 12 threads visible), against
hnswlib, faiss HNSW and faiss IVF-PQ fast scan with a re-rank, each over the parameter grid ANN-Benchmarks uses for it.
k = 10, all 10,000 test queries, one query at a time on one thread, best of two repetitions. Recall@10 follows
ANN-Benchmarks: a returned item counts if its angular distance is within the k-th true distance + 1e-3.

`run.sh` is the whole pipeline for one dataset. Raw rows are in `results/<dataset>_ours.csv` and
`results/<dataset>_baselines.csv`, build times in `results/<dataset>_builds.csv`, all tables in `results/tables.md`.

## Setup

- **Our index.** Built from the training vectors alone by `rust/tools/fit.py` (glove: 1,184 clusters, 64 groups,
  n = 100 padded to 128; nytimes: 290 clusters, 36 groups), encoded when opened (11 s at 1.2M on 12 threads).
  *Exact* is `knn2` (seed, threshold sweep, selection) through the planner. *Flat* is brute force over the same stored
  f16 vectors on the same thread. *Approx* probes the `nprobe` clusters nearest the query, scores their items by the
  32-byte interval midpoint and re-ranks the best 100 or 1,000 from full vectors.
- **Baselines.** ANN-Benchmarks' wrappers and grids: hnswlib and faiss HNSW with M in {4 … 96}, efConstruction 500, ef
  in {10 … 800}; faiss IVF`nlist`,PQ(d/2)x4fs with nlist in {512, 1204, 2048, 4096}, nprobe in {1 … 200}, k_reorder
  in {0, 10, 100, 1000}. Built with 8 threads, queried with 1.
- **Filters.** Random visible sets of 50 / 10 / 1 / 0.1% of the entities (seed 7). Ground truth for recall is float
  brute force over the visible set. Filtered baselines use one build each: hnswlib M-16 (filter callback), faiss HNSW
  M-16 (`IDSelectorBitmap`), faiss IVF-PQ nlist 2048 (selector in the IVF search, then the re-rank).
- **Timing conditions.** Two single-thread timing jobs ran side by side on separate physical cores (ours on CPU 0,
  baselines on CPU 4), while other agents' experiments ran on CPUs 8-11. A solo re-run of 1,000 glove queries was
  10-17% faster for our exact and flat paths and 25-35% faster for hnswlib, so every method is somewhat slowed and the
  baselines slightly more than ours.

## Exact k-NN (one thread)

| Dataset | Filter | Ours exact QPS | Flat over stored values QPS | Recall@10 (ANN rule) | Queries differing from brute force: sets / scores |
|---|---|---|---|---|---|
| glove | none | 29.9 | 75.5 | 1.0000 | 0 / 0 |
| glove | 50% | 49.7 | 50.0 | 1.0000 | 0 / 0 |
| glove | 10% | 173 | 211 | 1.0000 | 0 / 0 |
| glove | 1% | 1,355 | 4,505 | 1.0000 | 0 / 0 |
| glove | 0.1% | 2,443 | 30,917 | 1.0000 | 0 / 0 |
| nytimes | none | 44.6 | 110 | 0.9991 | 9 / 0 |
| nytimes | 50% | 134 | 146 | 0.9991 | 0 / 0 |
| nytimes | 10% | 499 | 866 | 0.9991 | 0 / 0 |
| nytimes | 1% | 1,490 | 12,560 | 0.9991 | 0 / 0 |
| nytimes | 0.1% | 2,097 | 73,726 | 0.9991 | 0 / 0 |

Exactness against brute force over our stored (f16) values: no query returned a different list of scores. The nine
differing sets on nytimes are its nine all-zero test queries, where every item ties at score 0. Those nine queries
also make recall 0.9991 there: ANN-Benchmarks' distance to a zero query is undefined (NaN), so nothing counts. Against
the published ground truth, glove recall is exactly 1; the f16 storage never cost a counted neighbour.

## Unfiltered: best QPS at a recall target

| Dataset | Method | ≥ 0.90 | ≥ 0.95 | ≥ 0.99 |
|---|---|---|---|---|
| glove | ours approx | 232 | 116 | 57 |
| glove | hnswlib | 3,004 | 1,536 | 611 |
| glove | faiss HNSW | 2,668 | 1,501 | 605 |
| glove | faiss IVF-PQ fs + re-rank | 4,642 | 2,696 | 1,145 |
| glove | ours exact (recall 1) | 29.9 | | |
| nytimes | ours approx | 199 | 109 | – (max 0.985 at 56) |
| nytimes | hnswlib | 4,221 | 1,727 | 374 |
| nytimes | faiss HNSW | 3,707 | 1,534 | 360 |
| nytimes | faiss IVF-PQ fs + re-rank | 4,247 | 2,496 | – (max 0.967) |
| nytimes | ours exact (recall 1) | 44.6 | | |

Plots: `results/glove-100-angular.png`, `results/nytimes-256-angular.png`.

## Filtered: highest recall each baseline reaches, and its QPS

| Dataset | Visible | Ours exact (recall 1) | hnswlib M-16 | faiss HNSW M-16 | faiss IVF-PQ nlist 2048 |
|---|---|---|---|---|---|
| glove | 50% | 49.7 | 0.952 at 579 | 0.950 at 659 | 0.969 at 596 |
| glove | 10% | 173 | 0.988 at 142 | 0.918 at 684 | 0.951 at 637 |
| glove | 1% | 1,355 | 0.999 at 17 | 0.790 at 686 | 0.895 at 2,032 |
| glove | 0.1% | 2,443 | 1.000 at 3 | 0.439 at 700 | 0.742 at 3,300 |
| nytimes | 50% | 134 | 0.965 at 431 | 0.963 at 506 | 0.911 at 1,931 |
| nytimes | 10% | 499 | 0.990 at 96 | 0.910 at 503 | 0.862 at 1,904 |
| nytimes | 1% | 1,490 | 1.000 at 16 | 0.686 at 502 | 0.702 at 4,277 |
| nytimes | 0.1% | 2,097 | 1.000 at 8 | 0.299 at 493 | 0.448 at 5,704 |

hnswlib's filtered rows use the first 500 test queries and ef in {10, 40, 120, 400}: its filter is a Python callback
per visited node, which dominates its time at low selectivity (the full grid would have taken hours). Full rows,
including the QPS at each recall target, are in `results/tables.md`.

## What the numbers say

- **Exact is correct everywhere and slow unfiltered.** On both datasets the exact path touches 95-99% of the items
  (it reads 28-34 MB per query, 9-23% of the f16 data) and is 2.5x slower than a plain scan of the same stored vectors
  on the same thread. GloVe and NYTimes give the bounds little cluster structure to use, as the numpy work found for
  GloVe (`knn/REPORT.md`).
- **Filtered, exact keeps recall 1 where the baselines lose it.** At 1% and 0.1% visible it is 80-800x faster than
  hnswlib (the only baseline that reaches 0.99 there) and 2-3.5x faster than faiss HNSW, which tops out at
  0.30-0.79; IVF-PQ is faster still but stops at 0.45-0.90. A scan of the visible set beats our exact path by 3-35x at
  1% and 0.1%. The k-NN path always runs its seed and sweep;
  it has no rule that sends a small visible set straight to a scan, as the threshold planner has.
- **Approximate mode** is an IVF with a coarse estimate and is 10-20x behind the graph indexes and IVF-PQ at equal
  recall: per probed item it builds tables and runs six levels, where faiss scans 4-bit PQ codes.

## Deviations from the protocol

- faiss HNSW on nytimes is built without the 239 all-zero training vectors (ids mapped back at query time). With
  them, every query met hubs at distance 1 and recall was 0.02. hnswlib and IVF-PQ keep them.
- Builds ran before the timed runs; build times in `results/*_builds.csv` were taken with other jobs running.
