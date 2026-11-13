# Native prototype, version 2: 1M results

Compiled by the coordinating session from the result files in `results/` after the agent was
stopped by request before its final report and before the 10M scale test finished. Profile: the
hedged data-only `H25` throughout. All figures measured on the same 4-core AVX-512 VM as version 1.

## What changed from version 1

- A fast-scan-decodable code: deterministic levels re-coded so each level's 4-bit units score by
  table lookup over 32-item blocks; lookup tables quantised to 16 bits and rounded outward
  (8-bit tables cost too many bytes, see `bytes_compare.csv`).
- Survivor re-packing between levels (lane occupancy 91–97%, was 56–74%).
- int8 local bases (exactly sound via the stored basis and its true inverse).
- One global set of rotations per block size (rotation setup now ~1 ms, was 7.5–9.6 ms).
- A planner choosing per cluster between the index path, reading full vectors, and a scan of the
  visible items.
- Profile loaded from a file (data-only `H25`).

## Correctness

569 queries × 4 answer sizes: 0 wrong; 731,378,512 interval checks with 0 deterministic and 0
randomised violations; largest randomised error 5.08 sd (c = 9.46); k-NN exact on 569/569;
filtered runs 0 wrong across 192 configurations.

## End to end at 1M (median ms; `e2e2_1m.csv`)

| Queries | Threads | 0.01% | 0.1% | 1% | 5% | k-NN k=10 | Flat f16 | Flat int8 |
|---|---|---|---|---|---|---|---|---|
| Item | 1 | 5.3 | 8.1 | 27.9 | 75.6 | 4.8 | 175 | 88 |
| Item | 4 | 2.6 | 3.5 | 9.1 | 23.0 | 2.9 | 45 | 23 |
| Text | 1 | 38.6 | 47.6 | 75.6 | 110.1 | 35.3 | 175 | 88 |
| Text | 4 | 13.1 | 15.5 | 21.0 | 32.5 | 12.5 | 46 | 23 |

Version 1 for comparison (text-tuned profile): item 19 / 33 / 72 / 170 (1 thread), 6.9 / 11 / 22 / 47
(4 threads); text 84 / 102 / 156 / 254 (1 thread), 23 / 27 / 42 / 70 (4 threads).

Bytes read per item on the index path alone: item 2.42 / 6.05 / 19.8 / 67.5, text 18.6 / 29.0 / 64.8 /
134.0 (version 1 with `H25`: item 2.20 / 5.38 / 17.4 / 59.7, text 16.7 / 25.9 / 58.2 / 120.8). End to
end with the planner, counting full vectors read for small clusters at 1,536 B: item 10.8 / 16.3 /
30.6 / 81.0, text 31.5 / 40.6 / 74.7 / 143.5.

## Where the time goes: text queries (`breakdown2_text.csv`, ms)

At a 1% answer, 1 thread, 83.5 ms in total:

| Part | ms |
|---|---|
| Anchor and cluster cut | 3.7 |
| Basis projection (int8) | 6.8 |
| Rotations | 1.0 |
| Cluster setup | 1.9 |
| Lookup tables | 13.0 |
| Re-packing survivors | 17.9 |
| Level decode | 19.3 |
| Bounds, decisions, compaction | 24.2 |
| Full vectors | 3.2 |
| Candidate setup and result bitmap | 2.7 |

At 5%: 116.5 ms (tables 15.3, re-pack 23.9, decode 25.5, bounds/decisions 35.5, full vectors 8.8).

**Decode is now under a quarter of the time; re-packing and per-level bound and decision work are
half.** Variants tried at 1% and 5%:

| Variant | 1%, 1 thread | 1%, 4 threads | 5%, 1 thread | 5%, 4 threads |
|---|---|---|---|---|
| Level walk (as built) | 83.5 | 23.2 | 116.5 | 34.5 |
| Tables shared per group | 84.4 | 23.4 | 117.6 | 33.8 |
| Single pass at 64 B, then full vectors | 107.7 | 30.0 | 151.6 | 44.7 |
| Single pass at 96 B | 88.1 | 23.4 | 116.9 | 35.0 |
| Single pass at 128 B | 87.1 | 24.1 | 114.1 | 33.6 |

Sharing tables per group cuts table time from 13 to 2 ms but widens bounds enough to read 40–46%
more bytes, so there is no net gain. No single-pass variant beats the level walk.

## Filtered queries (median ms, threshold at the unfiltered 1% answer; `filter2_1m.csv`)

| Queries, filter | Visible | Planner, 1 thread | Index only | Scan of visible | Planner, 4 threads | Scan of visible |
|---|---|---|---|---|---|---|
| Item, random | 50% | 6.3 | 6.3 | 98 | 4.7 | 29 |
| Item, random | 10% | 3.5 | 4.1 | 20 | 2.7 | 5.4 |
| Item, random | 1% | 1.3 | 3.0 | 1.2 | 0.64 | 0.48 |
| Item, random | 0.1% | 0.42 | 1.9 | 0.15 | 0.37 | 0.17 |
| Item, clustered | 10% | 1.3 | 1.3 | 15 | 1.7 | 3.5 |
| Item, clustered | 0.1% | 0.36 | 0.38 | 0.11 | 0.42 | 0.11 |
| Text, random | 50% | 71 | 71 | 99 | 20 | 29 |
| Text, random | 10% | 23 | 46 | 21 | 6.1 | 6.1 |
| Text, random | 1% | 1.4 | 28 | 0.99 | 0.61 | 0.47 |
| Text, random | 0.1% | 0.42 | 16 | 0.19 | 0.35 | 0.15 |
| Text, clustered | 10% | 18 | 15 | 17 | 3.9 | 4.4 |
| Text, clustered | 0.1% | 0.39 | 0.80 | 0.09 | 0.42 | 0.09 |

The planner tracks the better path to within a fraction of a millisecond at sparse visibility and
avoids the index's large losses on random filters for text, but still trails a plain visible scan
by 0.1–0.4 ms at 1% and below (its fixed setup).

## Not finished

- The 10M scale test (stopped during the build).
- Closing the remaining gap to faiss's kernel speed per byte.
