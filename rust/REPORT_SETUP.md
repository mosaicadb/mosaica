# Fixed per-query costs of the threshold query (1M BioCLIP-2, one thread)

Branch `research/setup-cost` from `research/colorder` at 9425c14ce. Configuration throughout: the dual-basis index
`c1m/dual`, `--int --gtab --flags repack --no-approx`, the fixed plan `results/setup/plan_fixed.txt`, 20 item and 20
text queries, AMD Ryzen 9 5900X (AVX2, no AVX-512) under WSL2, timing pinned to core 10, builds on 3 cores. Scripts:
`results/setup/run.sh` (1M) and `check.sh` (50k). Answer sizes are percentages of the 999,000 rows.

## 1. Where the time went

The breakdown now times the profiled run itself and derives the time between phases from enclosing totals (all of
`run_cluster`, all of one cluster's work, all of a query, the merge), so every millisecond lands somewhere.
Unattributed time is within 2% at every answer size, item and text, on a quiet machine (within 4% on a busy one).

The 8–19 ms the old breakdown left unattributed (text 1%: about 14 ms of a 63 ms profiled query) was:

| What | Text 1%, ms | Column |
|---|---|---|
| Compaction of the survivors' state after each block and level (outside every phase, although the old column was named "bounds+decisions+compaction") | 6.0 | compaction |
| The profiler's own time stamps, about six `rdtsc` per block; it tracks the profiled-minus-unprofiled wall gap (7.7 ms) | 5.8 | level other |
| Per-cluster bookkeeping: candidate lists, pushing in-ranges, allocation | 1.9 | cluster other |
| Per-(cluster, level) setup: the integer-to-float move at level 6, margins, scalar steps | 1.8 | level setup |

The old wall time was also the unprofiled best of two while its phases came from the profiled third run. At 50–75%
answers the result bitmap (7–11 ms, a sort of every answer) was the largest fixed item after the levels.

## 2. Basis products

The exact int8 product (`qv_exact`) was stalled, not bandwidth-bound: 64 bases of 768 x 768 (37.7 MB) took 3.1 ms
against 1.0–1.2 ms for a plain read of the same bytes. Prefetching the next four basis rows, line by line, while the
current four are multiplied brings it to the read floor in the microbenchmark (`vexact qvbench`: 3.13 -> 1.22 ms) and
to 1.5–1.7 ms on the index's own bases (`setup`; prefetch none / one / two / three blocks ahead: 2.43 / 1.66 / 1.76 /
1.52 ms). The arithmetic is unchanged (integer sums of int8 x 13-bit halves, one rounding), so the products stay
exact and no bound changes. A dense batched product cannot help: there is one query vector, so it is a matrix-vector
product and every basis byte is used once. The group post-processing (`cq` and `qvd2` in one pass, four partial
sums, inside the 1e-12 `cq` slack of `cluster_q2a`) is timed apart: 0.55 ms. Rotations stay at 0.75 ms.

Two figures for the same product: `setup` times the kernel alone, the same query's 64 groups repeated back to back
(1.5–1.7 ms); the breakdown times it inside each query, after the cut has run and with the bases evicted by the
previous query's reads (basis 2.3–2.5 ms; before, 4.15 ms including the post-processing). In the query: basis and
post-processing 4.15 -> 3.0 ms per text query (64 groups).

## 3. Cluster setup and tables

- With combined tables (`gtab`) a cluster's combined weights `w` are never read; `cluster_q2w` skips them. Cluster
  setup 2.2 -> 1.85 ms.
- `combine_lut` reduces the units' minima and maxima eight units at a time instead of one horizontal reduction per
  unit, recomputes values in its second pass and allocates nothing; its tables equal the unit-wise version's bit for
  bit (`combine_lut_unitwise`, tested). Group tables' extremes likewise.
- A fold's `State` keeps finished clusters' tables and builds the next ones into their storage.
- Tables were already built lazily, per level reached, per opened cluster. At text 1% a query builds 15,800
  (cluster, level) tables (319,000 units, about 400 survivors each); 2,600 of them serve 8 survivors or fewer.

Tables: text 1% 7.4 -> 4.8 ms, 50% 10.6 -> 7.0 ms; item 1% 1.4 -> 0.8 ms.

Sending a cluster's last survivors to full vectors instead of building another table (plan flag `bail=X`: before a
level with more than X table units per survivor) gains little and reads more: at X = 2, text 1% 47.6 -> 45.5 ms and
item 10% 75.6 -> 70.8 ms, the rest within noise, with bytes read up 1–30% (full vectors at 1,536 B). It is **off by
default** (`results/setup/e2e_bail.csv`).

## 4. What step 1 found

- **Result bitmap** (`to_bitmap2`): with many rows, set the entities' bits in a plain bitset and build the bitmap from
  its bytes instead of sorting. 10% 1.4 -> 0.2 ms, 50% 7.1 -> 0.5 ms, 75% 10.6 -> 0.6 ms.
- **Per-cluster bookkeeping**: unfiltered in-ranges are pushed as ranges and unfiltered candidate lists made directly:
  1.9–2.4 -> 0.4–0.5 ms.
- **Candidate init**: slots held in place read their keys in one load instead of a gather: -0.4 ms.
- **Level setup**: the move from the integer deterministic levels to the randomised ones converts four slots at a time
  with the same operations: 1.8–2.3 -> 1.6–1.9 ms.
- **Compaction**: an in-register version of the AVX2 16-lane compress emulation moved about 1.2 ms from compaction to
  re-pack and left their sum unchanged (11.2 ms at text 1%), so it was dropped; its test now covers all 65,536 masks.
- **Profiler overhead** exists only in profiled runs.

## Correctness

50k, 200 item and 200 text queries, 7 answer sizes, filtered runs, float64 oracle, after every step: 0 wrong
decisions, 0 deterministic and 0 randomised interval violations (about 137.7M checks; the count differs slightly from run to
run because the 50k check recalibrates the planner's cost model, which moves a few clusters between the index path
and full vectors), 0 deterministic decisions not implied
by the f64 certificate, k-NN exact on 400/400, oracle 0 wrong. The same with `bail=2`. At 1M every breakdown run had 0
wrong against brute force, and bytes read per item are identical to the baseline at every answer size.

## Before and after (1M, one thread, median ms)

Three processes per binary, interleaved base / new / base / new / base / new; each figure is a process's median over 3
runs of the median over 20 queries, and the table gives the median of the three and each one. The first new process
still had the in-register compress (no net effect). Another agent's benchmarks ran on cores 4–6 meanwhile; the second
baseline at text 1% (76.7) and the second new process were disturbed.

| Queries | Answer | Baseline ms (runs) | New ms (runs) | Change | Bytes/item |
|---|---|---|---|---|---|
| item | 0.01% | 2.4 (2.4 / 2.3 / 2.5) | 2.4 (2.4 / 2.8 / 2.3) | +0% | 2.8 / 2.8 |
| item | 0.1% | 3.6 (3.6 / 3.4 / 3.6) | 3.4 (3.3 / 3.9 / 3.4) | -4% | 5.8 / 5.8 |
| item | 1% | 9.0 (9.0 / 8.9 / 9.2) | 8.2 (8.1 / 9.5 / 8.2) | -10% | 18.7 / 18.7 |
| item | 10% | 82.1 (83.3 / 82.1 / 81.5) | 73.4 (69.5 / 78.9 / 73.4) | -11% | 136.0 / 136.0 |
| item | 30% | 110.3 (111.3 / 110.3 / 109.5) | 98.0 (94.3 / 105.9 / 98.0) | -11% | 256.8 / 256.8 |
| item | 50% | 121.9 (121.9 / 120.7 / 122.9) | 110.4 (102.6 / 114.0 / 110.4) | -9% | 296.6 / 296.6 |
| item | 75% | 121.4 (122.0 / 119.5 / 121.4) | 105.4 (98.1 / 105.4 / 106.5) | -13% | 254.0 / 254.0 |
| item | k-NN 10 | 2.5 (2.6 / 2.5 / 2.5) | 2.6 (2.3 / 3.0 / 2.6) | +5% | 2.2 / 2.2 |
| text | 0.01% | 23.4 (23.4 / 23.2 / 24.4) | 20.4 (19.2 / 21.4 / 20.4) | -13% | 20.1 / 20.1 |
| text | 0.1% | 30.6 (29.9 / 30.6 / 33.3) | 26.1 (25.0 / 26.1 / 26.8) | -15% | 25.1 / 25.1 |
| text | 1% | 61.9 (57.3 / 76.7 / 61.9) | 49.8 (48.5 / 49.8 / 54.2) | -20% | 64.6 / 64.6 |
| text | 10% | 93.3 (92.3 / 93.3 / 93.5) | 83.5 (83.1 / 83.5 / 86.1) | -11% | 192.0 / 192.0 |
| text | 30% | 118.7 (118.7 / 118.3 / 121.9) | 108.7 (108.9 / 105.2 / 108.7) | -8% | 309.5 / 309.5 |
| text | 50% | 129.8 (129.8 / 131.3 / 127.9) | 115.3 (115.3 / 114.3 / 118.7) | -11% | 347.0 / 347.0 |
| text | 75% | 132.4 (131.6 / 132.4 / 133.1) | 113.9 (109.9 / 113.9 / 116.9) | -14% | 300.5 / 300.5 |
| text | k-NN 10 | 21.7 (21.7 / 21.2 / 22.5) | 19.4 (17.6 / 19.4 / 19.4) | -10% | 20.2 / 20.2 |

Phases (ms). Before: the baseline code with step 1's instrument (`breakdown_step1.csv`). After: `breakdown_final.csv`,
the final code but with the in-register compress, which moves time only between re-pack and compaction
(`breakdown_head.csv` is the final code itself, on a busier machine). Before, "basis" includes the post-processing.

| Phase | item 0.01% | item 1% | item 50% | text 0.01% | text 1% | text 50% |
|---|---|---|---|---|---|---|
| wall | 2.24 → 1.88 | 8.76 → 7.62 | 119.80 → 103.66 | 23.44 → 18.98 | 55.89 → 45.42 | 127.34 → 108.21 |
| prof_wall | 2.48 → 2.06 | 14.61 → 12.53 | 133.99 → 117.47 | 28.61 → 23.09 | 63.62 → 53.35 | 142.96 → 123.65 |
| anchor | 0.15 → 0.12 | 0.22 → 0.21 | 0.25 → 0.25 | 0.25 → 0.24 | 0.26 → 0.24 | 0.25 → 0.25 |
| cut | 0.76 → 0.65 | 1.42 → 1.45 | 1.55 → 1.58 | 1.56 → 1.47 | 1.52 → 1.54 | 1.58 → 1.55 |
| basis | 0.15 → 0.06 | 1.52 → 0.89 | 4.18 → 2.51 | 4.14 → 2.29 | 4.15 → 2.44 | 4.15 → 2.45 |
| group post | – → 0.02 | – → 0.20 | – → 0.55 | – → 0.47 | – → 0.55 | – → 0.56 |
| rotations | 0.11 → 0.10 | 0.35 → 0.36 | 0.75 → 0.75 | 0.69 → 0.69 | 0.74 → 0.76 | 0.75 → 0.75 |
| cluster setup | 0.06 → 0.06 | 0.77 → 0.66 | 2.20 → 1.88 | 2.02 → 1.73 | 2.21 → 1.88 | 2.36 → 1.85 |
| tables | 0.12 → 0.08 | 1.36 → 0.84 | 10.22 → 6.87 | 3.02 → 1.77 | 7.37 → 4.81 | 10.59 → 6.99 |
| level setup | 0.04 → 0.04 | 0.39 → 0.40 | 2.17 → 1.84 | 0.93 → 1.03 | 1.84 → 1.58 | 2.22 → 1.88 |
| candidate init | 0.09 → 0.08 | 0.71 → 0.61 | 3.25 → 2.86 | 2.07 → 1.74 | 3.04 → 2.63 | 3.31 → 2.92 |
| re-pack | 0.10 → 0.11 | 0.93 → 1.12 | 6.57 → 9.37 | 2.05 → 2.31 | 5.18 → 6.40 | 6.53 → 9.00 |
| level decode | 0.15 → 0.13 | 1.29 → 1.16 | 18.27 → 16.70 | 2.37 → 2.20 | 7.52 → 6.87 | 19.53 → 17.94 |
| bounds+decisions | 0.23 → 0.20 | 2.01 → 1.75 | 46.33 → 42.68 | 2.59 → 2.20 | 13.43 → 11.87 | 49.38 → 45.58 |
| compaction | 0.15 → 0.12 | 1.24 → 0.99 | 8.19 → 7.89 | 2.86 → 2.26 | 5.98 → 4.78 | 8.26 → 8.02 |
| full vectors | 0.06 → 0.06 | 0.63 → 0.61 | 10.00 → 10.20 | 0.44 → 0.44 | 0.97 → 0.97 | 12.06 → 12.26 |
| result bitmap | 0.00 → 0.00 | 0.12 → 0.12 | 7.02 → 0.54 | 0.00 → 0.00 | 0.14 → 0.14 | 7.07 → 0.52 |
| cluster other | 0.06 → 0.02 | 0.48 → 0.11 | 2.42 → 0.47 | 1.24 → 0.24 | 1.92 → 0.36 | 2.33 → 0.51 |
| level other | 0.12 → 0.10 | 1.00 → 0.87 | 11.13 → 9.83 | 2.09 → 1.80 | 5.75 → 5.05 | 12.01 → 10.55 |
| query other | 0.07 → 0.08 | 0.13 → 0.13 | 0.40 → 0.44 | 0.20 → 0.19 | 0.32 → 0.36 | 0.44 → 0.46 |
| merge | 0.00 → 0.00 | 0.00 → 0.00 | 0.28 → 0.13 | 0.00 → 0.00 | 0.00 → 0.01 | 0.12 → 0.20 |
| unattributed | 0.03 → 0.03 | 0.05 → 0.04 | -1.22 → 0.11 | 0.09 → 0.01 | 1.28 → 0.13 | 0.03 → -0.60 |

## On by default

Everything above except `bail=X` (off). The in-register compress was dropped.

## The next bottleneck

The fixed costs are now about 9 ms of a text query (cut 1.5, basis 2.5 and post 0.55, rotations 0.75, cluster setup
1.9, tables 1.8–7 depending on depth) and 2 ms of an item query at 0.01%. What remains is per item: at text 1%,
bounds and decisions 12 ms, decode 7, re-pack and compaction together 11, candidate init 2.6, against 4.8 ms of
tables.

1. **Re-pack and compaction** (11 ms at text 1%, 16 ms at 50%): the survivors' state is compressed in up to 20 arrays
   after every block through the AVX2 emulation of the 16-lane compress. Fewer arrays per survivor, or compacting less
   often, is the lever.
2. **Bounds and decisions** (12 ms at 1%, 46 ms at 50%): the randomised levels' per-lane arithmetic.
3. **Basis products on the real index** run at 1.5–1.7 ms alone and 2.3–2.5 ms inside a query, against a 1.0 ms
   read floor, and the cut reads 6 MB of f64
   centres per query for `q.m` (reading the f32 centres and widening in registers would give identical results from
   half the bytes).
