# Exact threshold filter: databases, ANN libraries and our index

![latency against answer size, 1 thread](results/latency_1t.png)

![the same at 4 threads](results/latency_4t.png)

The plots show one line per system so that they stay readable; the tables below hold every row, including our index
as measured on 13 Nov, ClickHouse Float32, the half-precision faiss variants (the same speed as f32 at half the memory,
except flat range search) and each approximate method at its default setting. Approximate methods appear at their
fastest setting with mean recall at least 0.99; a hollow point is one where the method could not reach 0.99 within
5 s per query, labelled with the recall it reached. In the 1% visible panel faiss flat is the search over a compacted
copy of the visible rows, which takes about 30 ms to build per visible set. The data behind both plots is in
[results/plot_data.csv](results/plot_data.csv).

This is the 14 Nov update of the 13 Nov report. Our rows were re-measured with the current index. The approximate
rows were extended to larger answers, and half-precision faiss variants were added. The ClickHouse rows and the other
existing rows are as measured on 13 Nov.

## The question

The query is: given a unit query q and a threshold t, return the ids of every item with q·x ≥ t, together with
their count. It is run over the whole corpus, and within a viewer's visible set (visible ∩ {q·x ≥ t}). Every time below
covers the whole query, from the call to holding the full id set, so returning 750k ids is counted.

## Setup

- **Corpus.** 999,950 BioCLIP-2 image embeddings (768 d), from 20 random row groups of
  `imageomics/TreeOfLife-200M-Embeddings/bioclip-2_float16` (seed 7, the same picks as `../../fetch.py`), normalised in
  f64. 50 further rows were held out as image queries.
- **Queries.** 150 queries: 50 held-out images (in-distribution) and 100 out-of-distribution (OOD) queries, text-prompt
  embeddings searched against the image embeddings: 50 from `text_q.npy` and 50 from `text_insample.npy`. Timing used
  every 10th query, which gives 15 queries (5 of each kind). OOD queries cost our index more, because they point where
  the code is coarse and their scores bunch near any threshold; the medians mix the two kinds.
- **Thresholds.** For each query there are seven thresholds, with answers of 0.01%, 0.1%, 1%, 10%, 30%, 50% and 75% of
  the corpus. Each is the midpoint between the k-th and (k+1)-th f64 similarity, so the exact answer has exactly k items.
  Answer sizes run from 100 to 749,962.
- **Visible sets.** Random sets of 50%, 10% and 1% of the ids. ClickHouse and the ANN libraries ran unfiltered and at
  1%. Ours ran at all four.
- **Machine.** Ryzen 9 5900X (AVX2, no AVX-512) under WSL2.
  - 13 Nov: every run was pinned to cores 8–11, with another agent timing on cores 0–7.
  - 14 Nov: 1-thread runs were pinned to core 10 and 4-thread runs to cores 8–11. Builds ran on cores 9–11 (3
    threads). Other agents were running on cores 0–3 throughout, and on cores 4–8 during our 1-thread run and the
    first approximate runs. Memory bandwidth is shared, so expect up to 35% run-to-run variation.
- **Repetitions.** Each (query, threshold, visible set) ran 5 times. A system's figure is the median over queries of each
  query's median, and the p90 is over the same per-query medians.
- **Cache state.** All figures are warm. Cold runs needed `sudo` to drop the page cache, which was not available.

## Versions and the forms used

- **ClickHouse 26.10.4.207**, official image `clickhouse/clickhouse-server:26.10.4.207` (sha256:4f491abc…035), 16 GB
  limit. Tables were `MergeTree ORDER BY id`, with `vec Array(Float32) CODEC(NONE)` and `Array(BFloat16) CODEC(NONE)`.
  BFloat16 is supported and not experimental. The query that ran is `SELECT id FROM t WHERE dotProduct(vec, q) >= t`
  with `optimize_move_to_prewhere = 0` (0.54 s against 0.7–0.9 s with automatic PREWHERE),
  `use_query_cache = 0` and `use_query_condition_cache = 0`, returning RowBinary over HTTP.
  - Also tried: `cosineDistance` (15–20% slower), `L2SquaredDistance` (slower), mmap reads and the uncompressed cache
    (no gain).
  - For the visible set, `PREWHERE id IN s_vis`, where `s_vis` is a Set-engine table built once at load, was the
    fastest correct form. `bitmapContains` on a stored `groupBitmap` state was about the same. An IN subquery was
    slightly slower, and a JOIN was slower again.
  - A 1% visible set saves little, because every 8192-row granule holds visible rows, so the whole `vec` column is still
    read.
  - The vector-similarity (HNSW) index is not used for a WHERE threshold in 26.10. On a test table, `EXPLAIN
    indexes = 1` shows the skip index on `ORDER BY cosineDistance … LIMIT 10` (2 of 20 granules) and no skip index on
    `WHERE cosineDistance(...) <= r` (20 of 20 granules).
- **DuckDB 1.5.6** was measured on 13 Nov and is left out of the plots, the table and the conclusion: it took about
  2 s per query (2.1–2.3 s unfiltered, 1.7–1.8 s at 1% visible, 1 thread), bound by reading the `FLOAT[768]` column
  (`SELECT sum(vec[1])` alone takes 1.6 s). Its rows are still in `results/duckdb.csv`.
- **Ours.** The `research/bench-update` branch (from `research/rand-bounds` at 1d827e0de), with the `dbcmp` mode
  ported into `rust/src/run2.rs`. It takes explicit thresholds and visible-set files. It times query preparation, the
  query, and the entity-id answer as a Roaring bitmap.
  - The index is the current default: `fit.py --basis dual` (four randomised levels at 56, 96, 160 and 256 B), run
    with `--int --gtab --flags repack`. Its cost model was calibrated on core 10.
  - `index` is `threshold2` through the planner. `flat` is the plain f16 scan of the same stored vectors.
  - The visible set is permuted into row order once per session, in 0.5 ms for 1% and 8–9 ms for 50%. That step is
    outside the timed query.
- **faiss 1.16.0** (pip `faiss-cpu`, AVX2 build) provided:
  - `IndexFlatIP.range_search` (exact, f32). Single-query range search does not use more than one thread, so its
    4-thread figures equal its 1-thread ones.
  - `IndexScalarQuantizer` with `QT_fp16` (exact against the f16 vectors), also as a range search. It does not use
    more than one thread for one query either.
  - `IndexFlatIPPanorama` (exact, 8 levels, after an orthogonal PCA rotation of data and queries; faiss's `PCAMatrix`
    centres the data, which changes inner products). Panorama ran in f32 only, because faiss has no half-precision
    Panorama index.
  - `IndexIVFFlat` IVF4096 and `IndexIVFScalarQuantizer` IVF4096 `QT_fp16` (range search), trained on the same 200k
    sample.
  - `IndexHNSWFlat` and `IndexHNSWSQ` `QT_fp16` (M 16, efConstruction 200).
  - Filtering in faiss used `IDSelectorBitmap`. The flat indexes were also measured over a compacted copy of the
    visible rows, which took 30–60 ms to build per visible set.
- **hnswlib 0.8.0** (M 16, ef_construction 200), f32. It ran unfiltered only, because its Python binding filters only
  through a Python callback. faiss HNSW covers the filtered case.
- **RaBitQ: rabitqlib 0.2.0**, the authors' library, as `IvfIndex` with the IVF4096 centres, 7 bits and every cluster
  probed (nprobe 4096), run as top-m with `high_accuracy = True`.
  - The library uses its built-in error bound (ε₀ = 1.9) to choose which candidates are refined with the extra-bit code.
    Final distances come from the 7-bit code, and no full-precision re-rank is done.
  - The bound is a probabilistic heuristic, not a certificate.
  - faiss 1.16's RaBitQ indexes re-rank only by a fixed factor (IndexRefine), so they were not used.
  - rabitqlib has no filter, so RaBitQ ran unfiltered only.
- **Panorama PR.** [faiss PR 4970](https://github.com/facebookresearch/faiss/pull/4970) (head fa834bf3ea04) was not
  built. It adds `IndexIVFPQPanorama`, which is approximate because it uses PQ codes.
- **Rebuilt indexes.** The 13 Nov ANN index files had been deleted, so IVF, RaBitQ, hnswlib and faiss HNSW were
  rebuilt with the same parameters. They reproduce the 13 Nov points they were checked against: IVF tight at 10k items
  23.7 ms r .992 (13 Nov: 26.2 ms r .992), and at 1% visible 1.64 ms r .991 (1.95 ms r .991); RaBitQ at 10k 24.1 ms
  r .996 (23.9 ms r .996); hnswlib tight at 10k 42.7 ms r .992 (41.1 ms r .992); faiss HNSW tight at 10k 226 ms r .992
  (222 ms r .992). See `results/ann_logs/recheck.csv`.

## Approximate settings

- **Top-m methods.** These ask for m = the exact answer size. "Default" is ef = m. "Tight" is the smallest rung on
  m × {1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24, 32, 64, …, 1024} with mean recall ≥ 0.99.
- **IVF range search.** "Default" is nprobe 1, faiss's default. "Tight" is the smallest power of two with mean
  recall ≥ 0.99.
- **The cap on the 14 Nov points.** The ladder stops at ef = N, and at the first rung whose median query takes more
  than 5 s. If 0.99 is not reached within that, the tight point is the last rung within the cap, and its recall is
  labelled. The 13 Nov points capped ef at 200k instead.
- **Tuning.** The tight settings were tuned on the timed queries themselves, so they are optimistic.
- **Recall cases that need a note:**
  - Unfiltered at 100 items, hnswlib and f32 HNSW stopped at 32 m on 13 Nov, at recall 0.985–0.987. HNSW-SQfp16,
    on the new ladder, reached 0.991 at 64 m (16 ms).
  - Unfiltered at 100k items, IVF reaches 0.989 at nprobe 2048, so its tight point is nprobe 4096. That probes every
    cluster, so it is exact, and slower than a flat scan.
  - At 1% visible, IVF needs every cluster from 1k items on.
  - Filtered HNSW (f32 and SQfp16) does not reach 0.99 within the cap from 1k items on. At 1k it reaches 0.989 at
    128 m (3.2–3.4 s), and 256 m gives 0.998 in 13 s. At 3k, 5k and 7.5k items the best within 5 s is 32 m, 24 m and
    16 m, with recall 0.66, 0.53 and 0.37, at 2–3 s per query.

## Median ms per query (whole id set returned)

Answer sizes are 100, 10k, 300k and 750k items unfiltered, and about 1, 100, 3k and 7.5k items at 1% visible. Times
over 1 s are rounded to whole ms.

| system | precision | unfiltered, 1 thread | 1% visible, 1 thread | unfiltered, 4 threads | 1% visible, 4 threads |
|---|---|---|---|---|---|
| **ours: index** | f16 | **29.7 / 43.6 / 76.9 / 76.6** | **1.22 / 1.11 / 1.18 / 1.33** | **11.0 / 15.0 / 27.3 / 27.9** | **1.03 / 1.08 / 1.11 / 1.23** |
| ours: index (13 Nov) | f16 | 48.7 / 80.6 / 148 / 157 | 1.27 / 1.23 / 1.29 / 1.40 | 14.9 / 23.0 / 43.9 / 52.5 | 0.71 / 0.73 / 0.83 / 0.88 |
| ours: f16 scan | f16 | 80.9 / 83.0 / 90.7 / 98.9 | 0.91 / 0.75 / 0.85 / 0.86 | 36.1 / 35.6 / 40.2 / 47.7 | 0.81 / 0.78 / 0.84 / 0.88 |
| faiss Panorama flat (exact) | f32 | 42.0 / 61.7 / 159 / 191 | 3.4 / 4.5 / 7.9 / 9.9 | – | – |
| faiss IndexFlatIP range (exact) | f32 | 151 / 151 / 134 / 131 | 1.0 (compacted) / 5.4 (selector) | 125–132 | 1.0 / 5.8 |
| faiss SQfp16 flat range (exact) | f16 | 1070 / 1094 / 1150 / 1146 | 11 (compacted) / 16 (selector) | 1097–1186 | 11 / 15 |
| ClickHouse BFloat16 | bf16 | 423 / 426 / 444 / 451 | 283 / 288 / 284 / 298 | 161 / 168 / 169 / 177 | 121 / 126 / 128 / 131 |
| ClickHouse Float32 | f32 | 619 / 631 / 657 / 688 | 575 / 580 / 624 / 590 | 320 / 329 / 349 / 294 | 242 / 240 / 255 / 260 |

Approximate methods, 1 thread. Unfiltered answers are 100 / 1k / 10k / 100k items; at 1% visible they are about
1 / 11 / 100 / 1k / 3k / 5k / 7.5k items. The 100k column and the 1k–7.5k columns are new on 14 Nov, as are the
SQfp16 rows. `r` is mean recall, shown where it is below 1.

| system | precision | unfiltered | 1% visible |
|---|---|---|---|
| hnswlib ef = m | f32 | 0.16 r .77 / 1.95 r .90 / 17.3 r .95 / 157 r .977 | – |
| hnswlib tight | f32 | 4.7 r .985 / 12.6 r .990 / 41.1 r .992 / 224 r .993 | – |
| faiss HNSW ef = m | f32 | 0.16 r .75 / 1.87 r .90 / 36.9 r .95 / 2011 r .977 | 0.03 r .11 / 0.05 r .07 / 0.19 r .09 / 1.9 r .07 / 6.9 r .05 / 14 r .05 / 22 r .04 |
| faiss HNSW tight | f32 | 6.4 r .987 / 45.7 r .994 / 222 r .992 / 4532 r .992 | 0.76 / 37 r .987 / 614 r .996 / 3203 r .989 / 1967 r .66 / 3017 r .53 / 3005 r .37 |
| faiss HNSW-SQfp16 ef = m | f16 | 0.17 r .68 / 1.44 r .90 / 36.4 r .95 / 2060 r .977 | 0.02 r .11 / 0.04 r .07 / 0.17 r .10 / 1.4 r .07 / 5.4 r .05 / 12 r .05 / 22 r .04 |
| faiss HNSW-SQfp16 tight | f16 | 16.1 r .991 / 44.6 r .993 / 236 r .992 / 4578 r .992 | 0.47 / 33 r .991 / 584 r .997 / 3371 r .989 / 1916 r .66 / 2924 r .53 / 2886 r .37 |
| faiss IVF4096 range, nprobe 1 | f32 | 0.26 r .23 / 0.27 r .11 / 0.27 r .02 / 0.26 r .002 | 0.24 r .22 / 0.24 r .08 / 0.25 r .02 / 0.28 r .002 / 0.23 r .001 / 0.23 r .0004 / 0.24 r .0003 |
| faiss IVF4096 range, tight | f32 | 13.2 r .997 / 12.9 r .992 / 26.2 r .992 / 113 (all clusters) | 0.34 / 3.7 / 1.95 r .991 / 9.0 / 7.7 / 7.5 / 7.4 (all clusters from 1k) |
| faiss IVF4096-SQfp16 range, nprobe 1 | f16 | 0.26 r .23 / 0.26 r .11 / 0.27 r .02 / 0.31 r .002 | 0.25 r .22 / 0.29 r .08 / 0.28 r .02 / 0.26 r .002 / 0.34 r .001 / 0.29 r .0004 / 0.27 r .0003 |
| faiss IVF4096-SQfp16 range, tight | f16 | 13.9 r .997 / 14.7 r .992 / 27.0 r .992 / 138 (all clusters) | 0.32 / 3.9 / 2.0 r .991 / 8.0 / 10.3 / 10.4 / 8.5 (all clusters from 1k) |
| RaBitQ 7-bit, all clusters | 7-bit + extended code | 11.6 r .989 / 12.7 r .994 / 23.9 r .996 / 378 r .997 | – |

The CSVs are in `results/`. `summary.md` has p90s and the 10% and 50% visible sets for ours, and `results/ann_logs/`
has every ladder walked for the new points.

## What changed in our index

The 13 Nov rows used the `research/vector-bench` index: a PCA basis per group of clusters, fourteen randomised levels
from 40 to 256 B, and floating-point decisions at the deterministic levels. The current default differs in these ways:

- **The dual basis** (`fit.py --basis dual`). Each group's basis is fitted to the metric between query offsets and
  item residuals, instead of to the residuals alone.
- **An integer decision kernel** for the deterministic levels (`--int`), with an f64 implication check.
- **Tables per (group, level)**, combined per cluster with stored centre tables (`--gtab`), and a re-pack policy from
  calibrated per-level occupancy thresholds (`repack`).
- **Less fixed work per query.** Basis products are prefetched, group post-processing is fused, cluster setup drops
  unused weights, and the result bitmap is built without a sort.
- **Four randomised levels instead of fourteen** (56, 96, 160 and 256 B). This removes three quarters of the
  per-level bound, decision, compaction and re-pack work, and the survivor state is compacted only where a later level
  reads it.
- **Cheaper and tighter randomised bounds.** Constant terms are computed once, the code tables are decoded by permutes
  instead of gathers, and each (group, level) constant comes from the exact law of the error instead of the
  sub-Gaussian bound.

Together these take 39% off at 100 items and 51% off at 750k unfiltered at 1 thread, and 26–47% off at 4 threads. At
1% visible the index is 4–10% faster at 1 thread.

## Correctness against the f64 brute force

Differing items are counted over all checked answers. "Beyond rounding" counts items whose |s − t| exceeds the
rounding bound of the stored type.

- **Ours, index and f16 scan (14 Nov).** The index's answers are byte-identical to the scan's at all four visible
  sets. The differing items match 13 Nov in every cell: unfiltered, 6, 30, 122, 690, 1271, 1378 and 1143 items
  across the seven sizes, and 0–20 at 1% visible. Every one lies within f16 rounding: the largest |s − t| is 6.7e-5,
  and none is beyond rounding (`results/correct_dump_ours_r4.csv`).
- **faiss SQfp16 flat.** Recall was 0.9989–1.0000, with 3–487 extra items per size over 15 queries. These are the
  same f16 rounding differences, since it computes over the same f16 vectors.
- **ClickHouse Float32.** 0–8 items differed per size, with |s − t| ≤ 5.4e-8.
- **faiss flat and Panorama.** Recall was 1.0000 with at most 2 extra items, within f32 rounding.
- **ClickHouse BFloat16.** 334 of 1,500 items differed at 0.01%, and 187k of 7.5M at 50%. The largest |s − t| was
  2.8e-3, within the bf16 bound. bf16 has 8 mantissa bits, so it is exact only to its rounding.

## Storage and memory

"In RAM" is what a system must hold to answer at the speeds above. Figures marked "measured" are the process's
resident memory added by loading the index.

| system | precision | on disk | in RAM |
|---|---|---|---|
| ours | f16 vectors, 276 B/item of codes | 1.43 GiB of vectors, 0.14 GiB of bases and 0.01 GiB of other parameters; the codes are rebuilt at open (about 140 s on 3 cores) | 1.43 GiB of vectors, 0.26 GiB of codes and 0.14 GiB of bases, about 1.8 GiB. The prototype process measured 6.2 GiB resident after the queries (peak 6.6 GB), because it keeps what the build at open allocated and maps the vector file more than once |
| ClickHouse Float32 | f32 | 2.87 GiB table | the `vec` column in the page cache (2.87 GiB); server RSS peak 5.5 GiB with both tables loaded |
| ClickHouse BFloat16 | bf16 | 1.44 GiB table | the `vec` column in the page cache (1.44 GiB) |
| faiss IndexFlatIP | f32 | none (built in RAM) | 2.86 GiB |
| faiss SQfp16 flat | f16 | 1.43 GiB | 1.43 GiB (measured) |
| faiss Panorama | f32 | 2.9 GiB, built in 12 s | about 2.9 GiB |
| faiss IVF4096 | f32 | 2.88 GiB, built in 76 s | about 2.9 GiB |
| faiss IVF4096-SQfp16 | f16 | 1.45 GiB, built in 71 s | 1.45 GiB (measured) |
| faiss HNSW | f32 | 3.00 GiB, built in 222 s | 3.0 GiB (process 3.04 GiB after loading) |
| faiss HNSW-SQfp16 | f16 | 1.56 GiB, built in 223 s | 1.57 GiB (measured) |
| hnswlib | f32 | 3.00 GiB, built in 132 s | about 3.0 GiB |
| RaBitQ | 7-bit + extended code | 0.67 GiB | 0.7 GiB |

The 14 Nov builds ran on 3 threads. Panorama and RaBitQ are 13 Nov figures on 4 threads. faiss reads a whole index
into memory, so where RAM is not measured it is the file size.

## Conclusion

At 1 thread the current index answers 5–14× faster than ClickHouse BFloat16 and 8–21× faster than ClickHouse Float32,
with the gap largest at small answers. At a 1% visible set the gap is 220–270× against BFloat16 and 440–560×
against Float32, because ClickHouse reads the whole vector column whatever the filter. At 4 threads the index is
6–15× faster than ClickHouse BFloat16.

Against the other exact methods:

- **faiss Panorama.** On 13 Nov Panorama was level with the index at 0.01–1%. The index is now faster at every
  size: 30 against 42 ms at 100 items, 44 against 62 ms at 10k, and 77 against 191 ms at 750k.
- **Our own f16 scan.** On 13 Nov the scan beat the index from about 1% of the corpus upwards. The index now beats it
  at every unfiltered size, by 2.7× at 100 items and 1.3× at 750k, and at 4 threads by 1.7–3.3×.
- **faiss over f16.** The half-precision flat range search takes about 1.1 s per query, 7–9× its f32 version, because
  the range search over `QT_fp16` does not take the batched path that the top-k search does (137 ms for k = 100 on
  the same index). The half-precision IVF and HNSW indexes run at the f32 speeds, with the same recall at each
  setting, in half the memory.
- **At 1% visible.** The plain scan of the visible rows (0.75–0.9 ms) and faiss f32 over a compacted copy (1.0 ms)
  are still slightly faster than the index (1.1–1.3 ms). The planner still takes the index there; the packed scan of
  the visible rows (`pscan`) is built but off by default.

Against the approximate methods:

- **Unfiltered, up to 10k items** (1%), the methods reaching recall 0.99 (IVF, RaBitQ and hnswlib) take 5–41 ms. They
  are faster than the exact index at 100 and 1k items, and level with it at 10k (26–41 ms against 44 ms).
- **Unfiltered, at 100k items** (10%), the exact index (62 ms) is faster than every approximate method at recall
  0.99: IVF 113 ms (only with every cluster, so exact), hnswlib 224 ms, RaBitQ 378 ms and faiss HNSW 4.5 s.
- **At 1% visible**, approximate search does not help. IVF needs every cluster from 1k items on (7–10 ms, 6–8× the
  index). HNSW collapses: 3.2 s for recall 0.989 at 1k items, and at 3k–7.5k items it cannot pass 0.66 recall within
  5 s per query.

Two things to read with care. The 4-thread figures at 1% visible are slower than on 13 Nov for both the index (1.0–1.2
against 0.7–0.9 ms) and the unchanged f16 scan (0.8–0.9 against 0.5–0.7 ms). This run pinned each thread to one core
of 8–11 where the 13 Nov run let them float, and below a millisecond thread wake-up dominates, so treat a difference
of that size as noise. And the 13 Nov 1-thread rows floated over cores 8–11, while the 14 Nov ones are pinned to core
10.

## Reproducing

```bash
V=venv-with: numpy 2.2.6, duckdb 1.5.6, pyarrow 21.0.0, scipy 1.15.3, faiss-cpu 1.16.0, hnswlib 0.8.0, rabitqlib 0.2.0, matplotlib 3.10.9
D=/home/joe/code/vexact-data/bioclip2-1m; I=/home/joe/code/vexact-benchup-data/index_r4; A=/home/joe/code/vexact-benchup-data/ann
python3 -I fetch.py $D; python3 -I prep.py $D ../../../vexact-data/bioclip2-text $D/prep
taskset -c 9-11 python3 -I ../../rust/tools/fit.py $D/prep/corpus16.npy $I --basis dual
taskset -c 9-11 vexact --v2 --int --gtab --flags repack --no-approx --index $I --queries $D/prep/queries.npy \
  --results results/ours --core 10 --pin 10 dbcmp --thr $D/prep/thresholds.npy \
  --vis $D/prep/vis_50.npy,$D/prep/vis_10.npy,$D/prep/vis_1.npy --step 10 --reps 5 --threads 1 --dump DUMP_DIR --tag r4_1t
taskset -c 8-11 vexact ...the same... --pin 8 dbcmp ... --threads 4 --tag r4_4t     # without --dump
python3 -I check.py $D/prep DUMP_DIR results/correct_dump_ours_r4.csv
python3 -I load.py parquet $D/prep $D/corpus.parquet; bash clickhouse.sh start; bash clickhouse.sh load
python3 -I dbrun.py clickhouse $D/prep results/clickhouse.csv --table t32|tbf --vis all|vis_1 --step 10 --dump DIR
taskset -c 9-11 python3 -I ann.py build ivf|rabitq|hnswlib|faiss_hnsw|sq16|ivf_sq16|hnsw_sq16|panorama $D/prep $A --threads 3
taskset -c 10 python3 -I ann.py query METHOD $D/prep $A results/ann.csv [--vis all|vis_1] [--shares 3,4,5,6] \
  [--settings default,tight|ladder] [--ladder-from X] [--tight X] [--cap-ms 5000]
python3 -I summarise.py results/summary.md results/ours/dbcmp_r4_1t.csv results/ours/dbcmp_r4_4t.csv results/clickhouse.csv
python3 -I plot.py results $D/prep
```

The 4-thread faiss flat runs used `taskset -c 8-11` and `--threads 4`. The ladder and timed setting of each new point
are in `results/ann_logs/`. Long ladders started at a rung whose recall was below 0.99 (`--ladder-from`), and slow
settings were timed in a separate run with `--tight`.
