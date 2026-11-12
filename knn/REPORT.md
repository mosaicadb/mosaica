# k-NN on the exact-threshold index

Saved by the coordinating session from the agent's returned text; condensed. Every number is the
agent's. Scripts, tables and baseline rows are in this directory (`prep.py`, `sim.py`, `post.py`,
`approx.py`, `model.py`, `ann.py`, `build_g.py`, `gtcheck.py`; `post_*.txt`, `approx_*.txt`,
`ann_*.jsonl`, `log_*.txt`).

## Summary

- **Exact k-NN works with zero violations and costs about a threshold query at the k-th best
  score** (5–22% above an oracle that knows that score). 1M BioCLIP, n = 768, excluding bases:

  | Queries | k = 1 | k = 10 | k = 100 |
  |---|---|---|---|
  | Item | 1.76 MB (0.115%) | 2.64 MB (0.172%) | 4.73 MB (0.308%) |
  | Text | 24.8 MB (1.62%) | 29.9 MB (1.95%) | 38.3 MB (2.49%) |

  Bases add 69–150 MB per query at 1M.
- **Cost is linear in N:** exact k = 10 touches 10.6% of items (item queries) and 59% (text).
  HNSW computes 2,210 distances for recall 0.994 (items) / 3,568 for 0.947 (text) and grows
  roughly with log N.
- **Bytes at 1M are competitive for item queries; modelled wall time is not.** Exact k = 10 reads
  2.6 MB vs 6.8 MB of float32 for HNSW at recall 0.994. Native model: 16–22 ms (items), 52–139 ms
  (text). Measured single-thread: HNSW 0.25–1.4 ms, IVF-PQ + re-rank 1–4 ms, flat BLAS 54 ms.
- **Filtering is where it wins:** cost falls with the visible set and stays exact. faiss HNSW keeps
  the same work and loses recall (0.82 at 0.1% even with ef 4096); hnswlib keeps recall but takes
  126–484 ms at 0.1%; IVF-PQ needs 256 of 1,024 lists for 0.997.
- **Approximate mode** is ordinary IVF with a re-rank funnel: items reach recall@10 0.984 at
  337 KB; text plateaus at 0.78 (the 16-byte estimate is too weak). HNSW is better for text.
- **GloVe-100 is a poor fit:** exact k = 10 reads 13.4% of the dataset, touches 96% of items.

**To make k-NN first-class:** tighter cluster exclusion and a hierarchy over centres (77% of centres
opened for items, all for text); a persistent entry structure for the seed; interval pruning in the
approximate mode; visibility-aware cluster bounds for clustered filters; shared or removed
per-group basis cost (33–64 of 64 groups touched).

## Procedure

Track R = combined design (deterministic to 32 B, randomised after); track D = deterministic only.

- **Fixed:** read one support-table row → upper bound per cluster.
- **Stage 1, seed:** take the 64 clusters with the largest bound containing a visible item, read
  centres, re-rank by q·m̂; in the first c clusters read the 16-byte level of every visible item;
  keep best 20k+200 to 96 B, keep 2k+8, read those in full. τ₁ = exact k-th best among them.
- **Stage 2, sweep:** the threshold query at τ₁ over visible items.
- **Stage 3, selection:** τ = k-th largest lower bound; out if upper < τ; certified in if at most
  k−1 others have upper above its lower; advance undecided items a level per round.

Correctness proved given valid intervals. Oracle threshold argued as a lower bound for this level
structure. Literal best-first not implemented (cannot be simulated in a batch pass).

## Exact cost detail (track R, MB per query; 100 item, 150 text queries)

| Queries | k | Seed | Centres + searches | Item levels | Total excl. bases | Oracle | Bases (groups) | Items touched | Full reads |
|---|---|---|---|---|---|---|---|---|---|
| Item | 1 | 0.291 | 0.572 | 0.896 | 1.76 | 1.56 | 69 (29) | 71,503 | 11 |
| Item | 10 | 0.333 | 0.594 | 1.711 | 2.64 | 2.28 | 79 (33) | 106,241 | 30 |
| Item | 100 | 0.759 | 0.618 | 3.340 | 4.73 | 3.87 | 89 (38) | 156,275 | 216 |
| Text | 1 | 0.212 | 0.847 | 23.74 | 24.8 | 23.7 | 150 (64) | 539,215 | 12 |
| Text | 10 | 0.255 | 0.849 | 28.80 | 29.9 | 27.8 | 150 (64) | 590,329 | 43 |
| Text | 100 | 0.681 | 0.850 | 36.68 | 38.3 | 34.6 | 150 (64) | 660,350 | 327 |

Track D: ~1% cheaper for items, 8–12% dearer for text. Violations: 0 everywhere (m768 and GloVe).
Randomised levels: largest error 6.38 sd over 1.25 × 10⁹ triples (constant 9.35).

## Approximate mode (recall@10 / KB excl. bases)

| Queries | Filter | c = 1 | c = 4 | c = 8 | c = 16 | c = 64 |
|---|---|---|---|---|---|---|
| Item | none | 0.670 / 159 | 0.952 / 240 | 0.984 / 337 | 0.989 / 513 | 0.990 / 1,624 |
| Text | none | 0.359 / 148 | 0.666 / 194 | 0.741 / 259 | 0.773 / 393 | 0.778 / 1,359 |
| Item | 1% random | 0.397 / 74 | 0.805 / 100 | 0.922 / 106 | 0.977 / 114 | 0.989 / 144 |
| Text | 1% random | 0.193 / 68 | 0.493 / 96 | 0.637 / 103 | 0.788 / 109 | 0.907 / 142 |

## Comparison (1M BioCLIP, k = 10, unfiltered)

| Method | Recall@10 item / text | Distance computations | Vector bytes touched | Wall ms (1 thread) |
|---|---|---|---|---|
| Flat | 1 / 1 | 1,000,000 full | 3,072 MB | 54 |
| faiss HNSW ef 64 | 0.973 / 0.771 | 872 | 2.7 MB | 0.25 |
| faiss HNSW ef 256 | 0.994 / 0.924 | 2,210 | 6.8 MB | 0.87 |
| faiss HNSW ef 512 | 0.998 / 0.947 | 3,568 | 11.0 MB | 1.37 |
| hnswlib ef 256 / 512 | 0.995 / 0.918, 0.998 / 0.939 | — | — | 0.78 / 1.28 |
| IVF-PQ nprobe 16 + re-rank | 0.995 / 0.827 | 13,960 codes + 200 full | 5.2 MB | 1.05 |
| IVF-PQ nprobe 64 + re-rank | 1.000 / 0.851 | 51,756 codes + 200 full | 9.1 MB | 3.8 |
| This index, approximate c = 8 | 0.984 / 0.741 | 9,434 / 5,873 partial + 28 full | 0.34 / 0.26 MB + bases | — |
| This index, exact | 1 / 1 | 106,241 / 590,329 partial + 30 / 43 full | 2.64 / 29.9 MB + bases | model 16–22 / 52–139 |

Memory: HNSW 3,216–3,221 B per item resident; IVF-PQ 104 B + full vectors for re-rank; this index
22–39 B resident, 609 B codes + 1,536 B vector on disk, ~157 MB fixed. Build at 1M: faiss HNSW
328 s, hnswlib 210 s, IVF-PQ 110 s, this index 314 s (excluding code encoding).

## Filtered k-NN (k = 10)

This index, exact (MB excl. bases):

| Filter | Item | Text | Items touched (item) | Brute force over visible |
|---|---|---|---|---|
| none | 2.64 | 29.9 | 106,241 | 1,536 |
| 50% random | 1.85 | 16.3 | 58,886 | 767 |
| 10% random | 1.09 | 4.62 | 15,298 | 153 |
| 1% random | 0.86 | 1.47 | 2,925 | 15.2 |
| 0.1% random | 0.54 | 0.68 | 576 | 1.48 |
| whole clusters | 1.37 | 4.60 | 35,000 | 145 |
| id range | 5.48 | 6.06 | 74,528 | 154 |

Below 1% visible the floor is the centres of clusters containing a visible item; at 0.1% brute
force over the visible set is the better plan.

Baselines, recall@10 item / text:

| Filter | faiss HNSW ef 256 | faiss HNSW ef 4096 | hnswlib ef 64 | IVF-PQ nprobe 64 | IVF-PQ nprobe 256 |
|---|---|---|---|---|---|
| 50% random | 0.998 / 0.921 | 1.000 / 0.993 | 0.984 / 0.853 | 1.000 / 0.864 | 1.000 / 0.865 |
| 10% random | 0.992 / 0.894 | 0.999 / 0.986 | 0.997 / 0.950 | 1.000 / 0.904 | 1.000 / 0.918 |
| 1% random | 0.820 / 0.705 | 0.998 / 0.980 | 0.998 / 0.987 (11.9 ms) | 0.997 / 0.907 | 1.000 / 0.973 |
| 0.1% random | 0.184 / 0.142 | 0.821 / 0.739 | 1.000 / 0.998 (126 ms) | 0.967 / 0.727 | 0.997 / 0.961 |
| whole clusters | 0.517 / 0.457 | 0.824 / 0.867 | — | 0.945 / 0.806 | 0.986 / 0.880 |
| id range | 0.126 / 0.581 | 0.224 / 0.751 | — | 0.426 / 0.732 | 0.843 / 0.822 |

faiss HNSW traverses the same graph and withholds filtered nodes (returns fewer than k: 1.8 of 10
at 0.1%, ef 256). hnswlib keeps searching (time ~1/selectivity; includes a Python callback).
faiss IVF-PQ filters before scoring codes but recall is capped by nprobe.

## GloVe-100-angular (1,183,514 × 100; 200 test queries; ground truth matches published)

| Method | Recall@10 | Cost |
|---|---|---|
| This index, exact | 1 | 31.6 MB = 13.4% of dataset; 96% touched; 1,440 full reads |
| This index, approximate c = 8 / 64 | 0.722 / 0.794 | 274 KB / 2.1 MB |
| faiss HNSW ef 128 / 512 | 0.832 / 0.919 | 0.35 / 1.24 ms |
| IVF-PQ nprobe 64 / 256 + re-rank | 0.908 / 0.976 | 1.8 / 5.9 ms |
| Flat | 1 | 49 ms |

Published ann-benchmarks curves were not fetched or cited.
