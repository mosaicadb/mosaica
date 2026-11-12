# Native prototype: report

Saved by the coordinating session from the build agent's returned text; condensed. Every number is
the agent's. Build and run instructions in `README.md`; raw results in `results/`.

Machine: 4-core AVX-512 Xeon (Sapphire Rapids) VM, stable Rust 1.97, `-C target-cpu=native`,
release. Memory read bandwidth 13.4 GB/s (1 thread), 42.5 GB/s (4 threads).

## Summary

- **The guarantee held everywhere:** 569 queries × 4 answer sizes with 0 wrong decisions; 691M
  per-level interval checks with 0 deterministic and 0 randomised violations (largest randomised
  error 5.34 sd against c = 9.46); exact k-NN 569/569; filtered runs and the 10M run 0 wrong.
- **The decode kernel is ~1 ns per stored byte,** 15–20× slower per byte than faiss fast-scan
  (0.046–0.066 ns/B). The nested tree code's refinements depend on earlier bits, so a lookup keyed on
  new bits alone isn't exact; the kernel decodes bit-planes over 16-item blocks with register
  tables. Closing the gap needs a per-level code with self-contained 4-bit units, outward-rounded u8
  tables, 32-item transposed blocks and re-packing survivors between levels (conjectured).
- **End to end at 1M** the index beats a flat f16 scan for items and for text at tight answers; text
  at 4 threads is roughly level with a flat int8 scan. ~31 ms of each text query is fixed setup
  (151 MB of f32 bases plus rotated weights). The index is compute-bound.
- **At 10M (synthetic)** text queries gain 3–4× over a flat scan at tight answers; item queries
  6–40×.
- **Filters don't speed up the block kernel;** a flat scan of the visible set wins below ~10%
  random visibility for items and at every random fraction tested for text. Whole-cluster filters
  favour the index.
- **Disk: walking the levels reads 1.3–11× fewer pages** than going straight to the full vector,
  because sorted blocks share pages — overturning the review's conjecture.

## What was built

From numpy (`tools/export.py`): clusters (K = 1000), 8-bit centres, groups (G = 64), f32 bases,
mixture profile, allocations and weights, block index sets and rates, tree quantiser tables, queries.
In Rust: sort key â, residuals in f64, nested tree codes as real bit-planes, 4-bit relative norms
with calibration, randomised levels (one global Haar rotation per block size from ChaCha20 and an
IEEE-only Householder QR with the sign fix; 1-byte relative block norms; 4-bit tan φ; fallbacks),
the support table on the stored f16 vectors rounded outward, all bounds and decisions, exact k-NN.

Stored bytes per item, cumulative: 6.5, 11, 15.5, 20, 28.5, **37** (32 B), 41, 53.6, 66.5, 71, 79.5,
88, **105.5** (96 B), 122, 138.5, 156, 172.5, **203** (192 B), 234.5, 272 (256 B).

Simplifications adopted: no level-0 norm; 320–576 B levels dropped (straight to the full vector
after 256 B); closed-form sub-Gaussian bound (c = 9.46, union over 14 levels); no text anchors;
one global rotation per block size; one α per cluster for all levels (≤1.4% wider at 4 B); norm
prediction from item scale and level only; sphere-in-the-certificate dropped.

**Exactness.** s(q, x) = 2⁻⁴⁸ Σ QᵢXᵢ with f16 values × 2²⁴ summed exactly in i128; queries rounded to
f16; every slack has an explicit source (basis orthogonality defect ≤ 1e-6, f32 accumulation error
bound per query and cluster, â quantisation, f32 scalar ops, randomised scale factor); stored bound
‖x‖² ≤ 1 + 4.9e-4; f32 threshold rounded upward; near-threshold f32 dots fall back to the exact
integer product.

**Id architecture.** Rows in (cluster, â) order with a u32 row-to-entity map; each viewer's Roaring
bitmap permuted once per session into a dense row bitset plus per-cluster visible counts; per block a
16-bit visibility mask ANDed with the undecided range; result built by mapping, sorting and
`RoaringBitmap::from_sorted_iter` (included in every timing).

## Measurements

**Decode cost per item, cumulative** (1M items, all lanes active, best of 5, 1 thread):

| Level (B) | 4 | 8 | 12 | 16 | 24 | 32 | 40 | 48 | 64 | 96 | 128 | 192 | 256 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ns per item | 11.0 | 15.2 | 20.4 | 23.7 | 31.5 | **39.6** | 46.6 | 59.7 | 82.8 | **129.7** | 172.1 | **266.1** | 385.0 |

**End to end at 1M** (100 item, 100 text queries; flat scans 20; best of 2; median ms; answer sizes
0.01 / 0.1 / 1 / 5%):

| Queries | Threads | 0.01% | 0.1% | 1% | 5% | k-NN k=10 | Flat f16 | Flat int8 |
|---|---|---|---|---|---|---|---|---|
| Item | 1 | 19.4 | 32.7 | 71.8 | 169.7 | 15.6 | 180 | 90 |
| Item | 4 | 6.9 | 11.4 | 21.5 | 47.3 | 6.2 | 47.3 | 24.5 |
| Text | 1 | 83.7 | 102.0 | 156.2 | 254.4 | 76.8 | 177 | 88 |
| Text | 4 | 22.8 | 27.0 | 42.4 | 69.7 | 23.2 | 46.1 | 24.0 |

Bytes read per item: items 2.26 / 5.46 / 17.7 / 60.5 (numpy reference 2.12 / 5.21 / 18.7 / 58.5);
text 16.2 / 25.2 / 57.2 / 119.9 (reference 22.8 / 32.2 / 57.7 / 99.3; 5% higher because deep levels
were dropped). Items touched: items 13 / 21 / 39 / 76%, text 66 / 77 / 93 / 99%. Lane occupancy
56–74%. Fixed setup per text query, 1 thread: 20 ms basis products, 8 ms rotated weights, 1 ms
anchors. Flat f16 reaches 63% (1 thread) / 77% (4 threads) of read bandwidth; the index scales
~3.8× from 1 to 4 threads.

**Batched text queries** (256 queries, queries per second):

| Answer | Threads | B = 1 | B = 4 | B = 16 | B = 64 |
|---|---|---|---|---|---|
| 0.1% | 1 | 9.1 | 14.6 | 15.4 | 9.6 |
| 0.1% | 4 | 31.4 | 51.0 | 56.6 | 36.3 |
| 1% | 4 | 21.6 | 35.6 | 38.7 | 26.9 |

**Filtered** (threshold at the unfiltered 1% answer; median ms, index / flat scan of visible set):

| Filter | Item, 1 thread | Item, 4 | Text, 1 | Text, 4 |
|---|---|---|---|---|
| Random 50% | 17.9 / 101 | 13.0 / 32.3 | 164 / 100 | 43.9 / 28.8 |
| Random 10% | 9.9 / 22.0 | 7.5 / 6.9 | 123 / 21.1 | 34.3 / 6.8 |
| Random 1% | 8.4 / 2.3 | 3.5 / 1.1 | 68 / 2.6 | 18.9 / 0.9 |
| Random 0.1% | 5.1 / 0.28 | 2.4 / 0.22 | 42 / 0.34 | 13.3 / 0.28 |
| Clustered 10% | 4.0 / 16.7 | 2.8 / 4.6 | 37 / 17 | 12.2 / 5.5 |
| Clustered 1% | 0.9 / 0.83 | 0.9 / 0.45 | 5.2 / 0.83 | 1.9 / 0.48 |

Probe-per-row variant 1.0–1.3× slower than the row bitset; permuting the bitmap once per session
1.1–4.9 ms.

**10M synthetic** (10 synthetic items per real item in the same 1000 clusters, from each group's real
spectrum and residual-norm distribution; full vectors regenerated from a seed; 0 wrong everywhere;
median ms):

| Queries | Threads | 0.01% | 0.1% | 1% | 5% | k-NN |
|---|---|---|---|---|---|---|
| Item | 1 | 41 | 85 | 356 | 2,147 | 30 |
| Item | 4 | 20 | 41 | 129 | 636 | 17 |
| Text | 1 | 410 | 652 | 1,567 | 3,813 | 332 |
| Text | 4 | 109 | 166 | 442 | 997 | 85 |

Flat f16 at 10M ≈ 1.8 s (1 thread), 0.47 s (4) (modelled). Times include synthetic regeneration
(text 1 thread: 14 / 90 / 571 / 1,990 ms); with real reads at ~0.2 µs, text 0.1% ≈ 565 ms and text
5% ≈ 1.8 s (1 thread) (modelled). Text gains 3–4× over flat at tight answers and breaks even at 5%.
Does not represent non-Gaussian residuals, a real 10M build's cluster sizes and anchor ratio, or real
full-vector reads.

**Disk** (O_DIRECT, 4 KiB pages, 20 queries; pages and ms at 1 / 4 threads):

| Queries | Answer | Resident through | Walk the levels | Straight to full vector |
|---|---|---|---|---|
| Item | 0.1% | 32 B | 1,468, 58 / 22 | 6,325, 260 / 96 |
| Item | 0.1% | 96 B | 989, 38 / 15 | 1,981, 78 / 32 |
| Item | 0.1% | 192 B | 564, 22 / 9 | 762, 29 / 13 |
| Text | 0.1% | 32 B | 7,567, 313 / 120 | 86,010, 4,870 / 1,492 |
| Text | 0.1% | 96 B | 1,771, 83 / 31 | 4,508, 253 / 76 |
| Text | 0.1% | 192 B | 923, 52 / 20 | 1,236, 61 / 20 |
| Text | 1% | 96 B | 18,189, 685 / 276 | 54,272, 2,083 / 801 |

## Conclusions against the byte-count research

Confirmed: per-item bytes for item queries within ~7%; exactness in native integer and bounded-float
arithmetic; k-NN ≈ one threshold sweep; batching helps (1.8×); fixed costs at 1M are real.

Overturned or new: the tree code decodes at ~1 ns/B, not 0.2–0.6 (a self-contained per-level code is
needed for fast-scan speeds); lane occupancy costs ~1.5×; random filters don't reduce block-kernel
time, so a planner must switch paths below ~10% visibility; walking the levels beats going straight
to the full vector on disk; at 10M text beats flat by 3–4× at tight answers, items by 6–40×.

## Caveats

One VM with a virtual disk; medians over 100 / 30 / 20 queries; levels benchmark ±20% noise at some
randomised levels; item queries at 1M are corpus members; profile fitted on `text_train` only; 10M
flat scans modelled.
