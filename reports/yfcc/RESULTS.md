# YFCC-10M filtered track: filtered-set sizes

base: 10000000 rows x 200386 tags, nnz=108210476; data values: [1.]
queries: 100000 rows x 200386 tags, nnz=138374; data values: [1.]
base nnz after dedup: 108210476
loaded + transposed in 3.7s
bitmaps built in 7.2s
query tag-count distribution: {1: 61626, 2: 38374}
V computed in 0.9s; distinct tag-sets: 23930
cross-check of 200 two-tag queries against numpy intersect1d: OK

## Distribution of V (measured)

| queries | n | p50 | p75 | p90 | p95 | p99 | p99.9 | max | mean |
|---|---|---|---|---|---|---|---|---|---|
| all | 100000 | 14,141 | 154,208 | 705,726 | 1,133,362 | 1,707,412 | 1,915,985 | 1,915,985 | 188,870 |
| 1-tag | 61626 | 86,892 | 368,312 | 991,156 | 1,379,732 | 1,707,412 | 1,915,985 | 1,915,985 | 304,233 |
| 2-tag | 38374 | 1,423 | 4,575 | 9,313 | 14,489 | 25,328 | 41,147 | 45,115 | 3,605 |

## Histogram of V on log bins (measured)

| V range | all | 1-tag | 2-tag |
|---|---|---|---|
| [0, 1) | 0 (0.0%) | 0 (0.0%) | 0 (0.0%) |
| [1, 10) | 0 (0.0%) | 0 (0.0%) | 0 (0.0%) |
| [10, 32) | 0 (0.0%) | 0 (0.0%) | 0 (0.0%) |
| [32, 100) | 799 (0.8%) | 708 (1.1%) | 91 (0.2%) |
| [100, 316) | 12,610 (12.6%) | 6,659 (10.8%) | 5,951 (15.5%) |
| [316, 1,000) | 10,646 (10.6%) | 684 (1.1%) | 9,962 (26.0%) |
| [1,000, 3,162) | 11,155 (11.2%) | 1,314 (2.1%) | 9,841 (25.6%) |
| [3,162, 10,000) | 11,645 (11.6%) | 2,744 (4.5%) | 8,901 (23.2%) |
| [10,000, 31,623) | 11,324 (11.3%) | 7,895 (12.8%) | 3,429 (8.9%) |
| [31,623, 100,000) | 12,119 (12.1%) | 11,920 (19.3%) | 199 (0.5%) |
| [100,000, 316,228) | 12,018 (12.0%) | 12,018 (19.5%) | 0 (0.0%) |
| [316,228, 1,000,000) | 12,026 (12.0%) | 12,026 (19.5%) | 0 (0.0%) |
| [1,000,000, 3,162,278) | 5,658 (5.7%) | 5,658 (9.2%) | 0 (0.0%) |
| [3,162,278, 10,000,001) | 0 (0.0%) | 0 (0.0%) | 0 (0.0%) |

## Share of queries with V below a threshold (measured)

| threshold | all | 1-tag | 2-tag |
|---|---|---|---|
| V < 1,000 | 24.05% | 13.06% | 41.71% |
| V < 10,000 | 46.86% | 19.65% | 90.55% |
| V < 30,000 | 57.68% | 31.65% | 99.48% |
| V < 100,000 | 70.30% | 51.80% | 100.00% |
| V < 300,000 | 82.32% | 71.30% | 100.00% |
| V < 1,000,000 | 94.34% | 90.82% | 100.00% |

## Cost model (MODELLED: 20 ns/item if V <= cut, else 3 ns/item + 0.2 ms)

| cut-off | total single-core s | s in small regime (#q) | s in large regime (#q) | share from top 1% of V | share from top 10% of V | QPS on 8 cores | mean ms/query |
|---|---|---|---|---|---|---|---|
| 10,000 | 69.0 | 2.0 (46,855) | 67.0 (53,145) | 8.2% | 54.8% | 11,601 | 0.690 |
| 30,000 | 70.2 | 6.0 (57,682) | 64.2 (42,318) | 8.0% | 53.8% | 11,393 | 0.702 |
| 100,000 | 80.3 | 20.8 (70,298) | 59.5 (29,702) | 7.0% | 47.0% | 9,962 | 0.803 |

At cut 30k, large regime: fixed 0.2 ms part = 8.5 s, per-item part = 55.8 s. Sum of V over all queries = 18,886,961,087.

## Tag frequencies in the base (measured)

(item, tag) pairs: 108,210,476; tags with >=1 item: 200,363 of 200,386; items with no tag: 1,072; tags per item p50 7, p90 20, p99 62, max 1,517, mean 10.82
| tags with more than | count |
|---|---|
| 1,000 items | 11,321 |
| 10,000 items | 1,326 |
| 100,000 items | 43 |
| 1,000,000 items | 9 |

largest tag frequencies: [3386745, 1915985, 1707412, 1379732, 1333819, 1310314, 1284396, 1235270, 1133362, 991156]

## Posting lists as Roaring bitmaps (measured with pyroaring 1.2.0)

serialized size, no run optimisation: 289,098,250 bytes (275.7 MiB), 21.37 bits per pair
serialized size, after run_optimize: 289,098,250 bytes (275.7 MiB)
for comparison, plain uint32 arrays: 432,841,904 bytes (412.8 MiB)

## Two-tag queries: V / rarer tag's count (measured)

| V/rarer | queries |
|---|---|
| exactly 0 | 0 (0.0%) |
| [0, 0.0001) (V>0) | 0 (0.0%) |
| [0.0001, 0.001)  | 0 (0.0%) |
| [0.001, 0.01)  | 139 (0.4%) |
| [0.01, 0.1)  | 12,725 (33.2%) |
| [0.1, 0.5)  | 18,638 (48.6%) |
| [0.5, 1]  | 6,872 (17.9%) |

median V/rarer for two-tag queries: 0.1562; rarer-tag count p50/p90/max: 11,384 / 39,267 / 62,620
sum over two-tag queries of rarer-tag count: 621,478,279 vs sum of V: 138,325,816

## Notes

- Source: big-ann-benchmarks `benchmark/datasets.py` class `YFCC100MDataset` (filtered), files `base.metadata.10M.spmat` and `query.metadata.public.100K.spmat` from https://dl.fbaipublicfiles.com/billion-scale-ann-benchmarks/yfcc100M/. Format per `benchmark/dataset_io.py` `read_sparse_matrix`: int64 nrow, ncol, nnz; int64 indptr[nrow+1]; int32 indices[nnz]; float32 data[nnz]. All data values are 1.0; file length matched the header exactly.
- V for two-tag queries computed with pyroaring intersection cardinality; 200 random queries cross-checked against numpy intersect1d.
- Minimum V over all queries is 58: no query has an empty or near-empty filtered set. The most frequent tag (3,386,745 items) is never used as a single-tag query; the largest single-tag V is 1,915,985.
- Single-tag V is spread almost evenly across the half-decade bins from 31.6k to 1M (about 12k queries each), which looks like the query set was stratified by selectivity.
- run_optimize changed nothing: the posting lists have no runs worth encoding (items are not ordered by tag).
