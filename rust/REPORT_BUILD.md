# The build: a faster encoder, a saved index, the fit in Rust, the partitioned build, the K × A table

> Brought forward with the rebase onto `research/exact-vector-search` (2026-11-16). The figures below were measured on
> the pre-rebase branch; the rebased branch's gates are in "Rebased" at the end. The commands there are interim forms,
> until the API is redesigned: `vexact --index FIT fit --partition PART [...]` for `vexact fit FIT --partition PART`,
> `vexact --v2 --index FIT --rebuild` for `vexact build FIT`, `vexact --index FIT anchors [...]` for `vexact anchors
> FIT`; `VEXACT_CAL_ITEMS` is `BuildOptions::cal_items`. A fit whose anchors would be exactly its K centres (A >= K and
> no prompts) now gets no `anchors.npy`, and the planner's centre-anchor shortcuts follow the index's recorded flag
> (`anc_centres`), not the anchor count.

2026-11-15. Steps 1 to 3 (first sections) are from the 60,000-row synthetic set; the partitioned build (step 4, from
"The partitioned build" on) was measured on 1M BioCLIP-2 rows and on 1M and 4M synthetic rows.

## Steps 1 to 3

Every figure in this part is from the 60,000-row synthetic set of `tools/synth_vectors.py` (768 dimensions, a mixture
of 300 Gaussian clusters with a decaying spectrum; 200 held-out items from the same mixture as item queries; 50 points
of the mixture without the data's rotation as an out-of-distribution stand-in, called text below), on a 4-core Xeon at
2.3 GHz (AVX-512) in a container.

### What changed

1. **Randomised encoder by table.** `rand_encode` is the greedy expansion's cell on a dyadic grid, so it is a clamped
   floor and a bit-reverse table; `rand_value` is a table. `rand_choose` sums every rise's dot product and norm in one
   pass and hands the winner's to the scalar stage, which recomputed them. The stored code is unchanged: the hash of
   codes, scalars, keys, row order and escapes is equal before and after (`722e49d7be0d1584`).
2. **Calibration on items, not whole clusters.** Each group takes about 8,192 items spread over up to 256 of its
   clusters (`VEXACT_CAL_ITEMS`), reading only those rows. The same number of items as before at 1M; at 1B it is 256
   clusters per group in place of 8.
3. **Saved index.** The first run on a fit writes the encoded index to `INDEX/encoded-KEY/`; later runs with the same
   key map it. The key covers the fit's parameters, the profile, the vectors file and every option that changes what
   is built. The open derives the plans as the build does, skipping the basis inverses and the randomised constants,
   and reads the calibration, constants and per-cluster data. JSON floats are parsed exactly (`float_roundtrip`), so
   the scalar grids an open sets are the build's bit for bit; a test fails without it.
4. **The fit in Rust** (`vexact fit`), then fed by `tools/cluster.py` (faiss k-means), since replaced by
   `tools/partition.py` (below). Stored centres are the exact means of the assigned rows. Every pass over the vectors
   goes cluster by cluster. The dual basis's `lam` comes from the group covariance, `diag(V8⁻¹ C V8⁻ᵀ)`, without a second
   pass. The certificate is still `tools/certify.py`, run as a subprocess.

### Build time, one thread, same fit

Two runs of each; seconds from the start of the build.

| | Randomised constants | Plans done | Calibrated | Encoded |
|---|---|---|---|---|
| Before | 8.0, 8.3 | 10.2, 10.4 | 17.5, 17.7 | 29.8, 30.2 |
| After | 8.1, 8.3 | 9.8, 9.7 | 12.1, 12.0 | 17.3, 17.3 |

Calibration and encoding together fall from 19.6 to 7.5 s (2.6×). The randomised constants, 8 s of what remains, are
not recomputed when a saved index is opened.

### The saved index

| | Before (build at every open) | Build and save | Open |
|---|---|---|---|
| Time to a usable index, 4 threads | 11.5–12 s | 11.5–12 s, save 0.1 s | 0.1 s |
| Peak memory of a `bytesq` run | — | 358 MB | 221 MB |
| Code hash | `c44827e2e155e64f` | `c44827e2e155e64f` | `c44827e2e155e64f` |

On disk: 107 MB, of which 92 MB is the f16 vectors. Bytes read and items touched are identical at every answer size;
`verify` on the opened index finds no wrong item, no interval violation in 310M checks and exact k-NN. The interval
check count varies by about 10,000 between runs of the same index, because the planner and re-pack thresholds are
timed at each open.

Query time, mapped against read onto the heap (`--resident`) from the same files, five alternating runs of each, 50
queries, seven answer sizes and k-NN: the median of mapped/read is 0.88 (item, 1 thread), 0.93 (item, 4 threads), 1.02
(text, 1 thread) and 0.99 (text, 4 threads). Mapping costs nothing measurable here. At 60k the whole index fits in the
page cache; under a memory cap it would not.

### The Rust fit against `tools/fit.py`

| | Fit time | Peak memory |
|---|---|---|
| `tools/fit.py --basis dual`, with certificate | 22.7 s | 2,723 MB |
| `tools/cluster.py` | 5 s | |
| `vexact fit --basis dual`, with certificate | 17.0 s | 1,883 MB |
| `vexact fit --basis dual`, without certificate | 2.7 s | 633 MB |

The certificate takes about 15 s in both, and its Python process sets both peaks.

Bytes read per item (item queries at answers of 0.01%, 0.1%, 1% and 10%; text the same), every index verified clean:

| Fit | Item | Text |
|---|---|---|
| `fit.py`, dual | 7.39 / 13.68 / 43.88 / 114.87 | 183.07 / 242.41 / 407.62 / 873.00 |
| Rust, dual, flat clustering, three seeds | 7.37–7.40 / 13.62–13.69 / 43.73–43.88 / 114.56–114.88 | 183.03–183.68 / 242.07–243.05 / 407.15–409.04 / 872.78–874.93 |
| Rust, dual, 4 cells, two seeds | 7.46–7.48 / 13.75–13.81 / 43.95–44.04 / 114.76–114.99 | 184.04–184.11 / 243.99–244.07 / 410.63–410.72 / 876.92–877.13 |
| Rust, dual, 4 cells, 5 global refinement steps | 7.46 / 13.78 / 43.95 / 114.71 | 184.14 / 243.96 / 410.60 / 876.92 |
| `fit.py`, PCA | 7.42 / 13.86 / 44.22 / 114.53 | 179.54 / 236.87 / 397.63 / 857.03 |
| Rust, PCA, flat | 7.49 / 13.93 / 44.28 / 114.63 | 179.96 / 237.41 / 399.15 / 858.59 |

The Rust fit with flat clustering is within the spread of clustering seeds of `fit.py`. Two-level training costs a
small amount every time: about 1% of the bytes at small item answers and 0.3–0.8% for text, outside the flat seeds'
range, and five global refinement steps on the sample do not remove it. That was at 60k rows: at 1M BioCLIP-2 (below) cells
cost nothing measurable, and are the default for large N.

## The partitioned build (step 4)

Nothing holds the vectors in memory any more, at any N:

```
VECTORS.npy --tools/partition.py (faiss)--> PART/ --vexact fit--> FIT/ --vexact build--> FIT/encoded-KEY/
```

- **`tools/partition.py`** replaces `cluster.py`. Coarse k-means to C cells on 1,000 rows per cell; one pass over the
  source appends each unit f16 row to its cell's file; fine k-means per cell on up to 200 rows per centre, K in
  proportion to the cell's rows; each row then goes to its nearest fine centre among those of its cell's T nearest
  cells (`--cross T`), and a row whose centre is in another cell is appended to that cell's moved-in files. C = 1 is
  flat k-means, so small runs take the same path as large ones.
- **`vexact fit FIT --partition PART`** reads each cell once for exact means (int8 + scale) and the row order
  (`ord{c}.u32`: a cell's rows in cluster order), groups clusters by k-means on the means, numbers them group by group
  (as `fit.py` does: queries visit a group's clusters together, and cell-major numbering was 3.8% slower at one thread
  for the same bytes), then reads each cluster once more for the group covariances. `perm.npy` is written streaming.
  No `Xs.npy`: the fit writes no copy of the vectors.
- **`vexact build FIT`** computes every cluster's offset in every output array from its size and its group's plan
  before encoding, then encodes clusters in parallel straight into the store's files (mapped, written in place), with
  the f16 vectors in their final row order. `x2` is taken from the written keys after encoding. The saved index is the
  output directory, sealed with its key and renamed into place.

The same fit built into memory and into files gives the same code hash, and the `fit.py` directory of the synthetic set
still hashes to `c44827e2e155e64f`.

**One-time migration (since 1077831).** A partition is known by its `manifest.json`'s content and the lengths of the
files it lists (`bucket::partition_id`). A partition written by an older `partition.py` (a manifest without `files`)
is refused by `vexact fit`, and a fit made from one (its `partition_id` under the old hash) is refused by the build and
by the open of its saved index, which then opens only with `--keep-index`. Run `tools/partition.py` again, refit and
`--rebuild`; no conversion tool is provided. Deleting the cells after the build is fine: an open warns that the
partition, or files its manifest lists, are missing, and opens the index (it holds its own vectors).

### 1M BioCLIP-2

1,000,000 rows (768 dimensions) from `fetch.py`'s 20 row groups; item queries are 200 rows of a held-out row group, text
queries 200 BioCLIP-2 text-tower prompts. 4 threads.

| | Partition | Fit | Build |
|---|---|---|---|
| `tools/fit.py --basis dual` | | 116 s, 7.8 GB peak RSS | 68 s |
| C = 1 | 70 s | 31 s, 3.7 GB | 70 s |
| C = 32, T = 1 | 22 s | 30 s, 3.7 GB | 68 s |
| C = 32, T = 32 (exact assignment) | 31 s | 30 s | 68 s |

Fit times include the certificate (`certify.py`, about 15 s). Peak RSS counts mapped pages of the cells; the fit's
anonymous memory is far smaller (below). `SYSTEM.md` records 147 s for the 1M build before step 1, on three cores.

Bytes read per item at answers of 0.01%, 0.1%, 1% and 10% (200 queries each):

| Fit | Item | Text |
|---|---|---|
| `fit.py`, three seeds | 24.80–25.39 / 36.75–37.47 / 60.56–61.37 / 129.92–130.77 | 19.13–19.65 / 28.77–29.41 / 62.24–63.35 / 197.34–199.00 |
| C = 1 | 25.90 / 38.04 / 62.21 / 131.85 | 19.29 / 28.97 / 62.64 / 198.22 |
| C = 32, T = 1 | 25.47 / 37.48 / 61.38 / 130.42 | 19.35 / 29.03 / 62.60 / 198.23 |
| C = 32, T = 2 | 25.65 / 37.72 / 61.67 / 131.20 | 19.20 / 28.81 / 62.26 / 197.30 |
| C = 32, T = 32 | 24.87 / 36.80 / 60.58 / 129.40 | 19.04 / 28.66 / 62.16 / 197.11 |
| C = 32, T = 32, group-major numbering | 25.47 / 37.58 / 61.57 / 130.93 | 18.98 / 28.57 / 61.96 / 196.66 |

Items touched agree to the third decimal in every row. Text is level or slightly lower. Items: the partitioned fits
span 24.87–25.90 at the smallest answer against `fit.py`'s 24.80–25.39; the median is about 1% higher (0.5% at 10%).
It is not the cells or the cross assignment (C = 32 reads no more than C = 1, and T has no effect beyond the seed
spread), and refitting `fit.py`'s own clusters in Rust ruled out the clustering; it is not resolved. **Defaults: C
of about N / 250,000 for large N, T = 1.**

`verify` on the group-major 32-cell index: no wrong item in or out at seven answer sizes over 80 queries, no interval
violation in 4.0 billion checks, exact k-NN. Earlier, the cell-major index: the same, over 2 billion checks, including
filtered sets down to 1,000 visible rows.

Query time, `e2e` (50 queries, seven answer sizes and k-NN, `--plan-default`), three rounds alternating with `fit.py`'s
index; the median over rows of partitioned / `fit.py`:

| | Item, 1 thread | Item, 4 threads | Text, 1 thread | Text, 4 threads |
|---|---|---|---|---|
| Cell-major numbering | 1.038 | 0.999 | 1.007 | 1.001 |
| Group-major numbering | 1.023 | 1.008 | 1.003 | 0.977 |

In the group-major runs one round was slower on every item row (1.02–1.13), while the other two were level (rounds 1
and 3: 0.97–1.04 at one thread); the gap at one thread left is that round, not a pattern across rows. Cell-major
numbering had the slowdown in every round, 5–7% at the smallest answers.

### Bounded memory, 1M and 4M rows

Synthetic, 256 dimensions (a mixture of 4,000 clusters; disk here does not hold 4M BioCLIP-2 rows twice), cells of
about 250,000 rows (C = 4 and C = 16), `--cert-time 0`, 4 threads. Peak anonymous memory (`RssAnon` of the process and
its children, sampled every 0.1 s), the memory the kernel cannot drop; the vectors are read and written through mapped
files.

| | 1M rows | 4M rows |
|---|---|---|
| `partition.py` | 478 MB, 13 s | 473 MB, 53 s |
| `vexact fit` | 86 MB, 1.4 s | 190 MB, 4.5 s |
| `vexact build` | 317 MB, 34 s | 683 MB, 149 s |

The build logs its anonymous memory at each phase:

| Phase | 1M | 4M | Grows with |
|---|---|---|---|
| Fit loaded | 25 MB | 52 MB | `perm` (8 B a row) |
| Calibrated | 266 | 328 | the calibration sample, capped at 8,192 items per group (reached at 4M) |
| Encoded | 267 | 333 | nothing |
| Assembled | 274 | 375 | per-cluster records (about 18 KB a cluster) and escapes |
| Support table | 282 (peak 317) | 497 (peak 683) | K²: 16 bytes per pair while built, 8 kept |

The encoding phase grew by 38 bytes a row (+151 MB at 4M against +38 at 1M) until the build fixed glibc's mmap
threshold at 1 MB: above its default, which rises to 32 MB as large blocks are freed, glibc kept each cluster's freed
buffers in its arenas. Only `vexact build` sets it; it costs about 1 s of 35 in encoding at 4M, and the codes are the
same (hashes `15cfe6ee75d39ecc` and `6a2af64cdcfa8b43` with and without).

**This gate is not fully met.** The passes over the vectors are flat in N; what still grows is named above. At 1B
rows and 1M clusters, `perm` would be 8 GB and the per-cluster records 18 GB, both read onto the heap, and the support
table 8 TB. The table goes with step 2. `perm` and the per-cluster records need to be mapped rather than read, at
build and at open, which is the "per-cluster records mapped" item below. The support table is also most of the 4M
build's time (from 59 s to 149 s).

## The support table against a fixed set of anchors (step 5)

The support table held each cluster's largest and smallest x · a against one anchor per cluster: K² entries, built
in a pass of N·K·n over the vectors after encoding (90 of 149 s and 240 MB of the 4M build; 8 TB at 1B). A query
bounds a cluster with the anchor nearest the query, and the bound holds for any anchor, so the anchors need not be
the clusters. The table is now K × A against the fit's `anchors.npy`, computed while each cluster is encoded (the
cluster's f16 rows are in hand), written through the store and mapped at open.

### Which anchors (1M BioCLIP-2, `tools/near_table.py`)

Before building it, `tools/near_table.py` measured candidate tables on the 1M `fit.py` index with the query's bound
arithmetic (200 item queries, 200 text queries; no violations). Items touched relative to the K × K table:

| Table | item 0.01% | 0.1% | 1% | t = 0.9 | text 0.01% | 0.1% | 1% |
|---|---|---|---|---|---|---|---|
| no table | 1.011 | 1.006 | 1.003 | 1.096 | 1.107 | 1.069 | 1.021 |
| each anchor's 64 nearest clusters, a cell bound beyond | 1.011 | 1.006 | 1.003 | 1.094 | 1.094 | 1.062 | 1.020 |
| 32 coarse anchors (k-means of the centre directions) | 1.007 | 1.004 | 1.002 | 1.054 | 1.027 | 1.016 | 1.005 |
| 128 coarse | 1.005 | 1.003 | 1.001 | 1.045 | 1.015 | 1.010 | 1.004 |
| 512 coarse | 1.002 | 1.001 | 1.001 | 1.012 | 1.010 | 1.006 | 1.002 |
| 512 coarse + 256 text | 1.002 | 1.001 | 1.001 | 1.012 | 0.767 | 0.836 | 0.939 |
| K + 256 text | 1.000 | 1.000 | 1.000 | 1.000 | 0.767 | 0.836 | 0.939 |
| 512 coarse + 256 text, genus held out | 1.002 | 1.001 | 1.001 | 1.012 | 0.968 | 0.980 | 0.992 |
| K + 256 text, genus held out | 1.000 | 1.000 | 1.000 | 1.000 | 0.962 | 0.976 | 0.990 |
| 512 coarse + 256 text, genus and higher taxa held out | 1.002 | 1.001 | 1.001 | 1.012 | 0.963 | 0.976 | 0.989 |
| K + 256 text, genus and higher taxa held out | 1.000 | 1.000 | 1.000 | 1.000 | 0.957 | 0.972 | 0.988 |

A sparse table of each anchor's nearest clusters does no better than no table: the clusters a query cannot skip are
the far ones, which the lists leave out. What sets the cost is how near the best anchor is to the query, not how many
clusters each anchor covers, so a dense table against fewer anchors keeps almost all of the K × K table's effect.
Item thresholds (t = 0.9) lose 1.2% at 512 coarse anchors.

Text anchors (k-means of embedded training prompts) cut text queries' items touched by a quarter in the first rows,
but that figure is mostly overlap between training and test prompts, and is only an upper bound. Both sets
(`agent2/text_train.json`, 2,206 prompts; `agent2/text_q.json`, 369) are drawn from the same prompt kinds and
templates, and 57 of the test prompts' 61 genus words (93%) occur in training prompts. With the text anchors fitted
only on the 1,610 training prompts that contain no test genus word ("genus held out"), the gain is 3–4% at an answer
of 0.01% and about 1% at 1%. That is the gain to expect for queries about taxa the prompts did not cover. Also
leaving out the 28 test family, order and class names (1,498 prompts kept; this drops every full-lineage prompt)
gives about the same: 0.963 and 0.957 at 0.01%. Item queries are unchanged. No bound was violated in any run.

`vexact fit` writes `anchors.npy`: all K centre directions when `--anchors` (1,024 by default) is at least K, else
k-means of them, plus `--text-count` (256) k-means centres of the prompts given with `--text-anchors`; `vexact anchors`
rewrites it for an existing fit. Without `anchors.npy` the table is the K × K one, byte-identical to before (code hash
`b8bdd054938fe563`), up to K = 4,096: beyond that the build refuses (K² · 8 bytes of table) and asks for `vexact
anchors`. `anchors.npy` is checked on load (rows of n, at least one, unit within 1e-12); prompts may be f16, f32 or f64;
`--anchors 0` or `--text-count 0` leaves that set out. The table is computed in f32 and each extreme recomputed in f64 over the items within twice the
f32 error bound of the f32 extreme, so it equals the f64 table (`hi.bin`, `lo.bin` byte-identical at 768 and 1,256
anchors).

This parity rests on a dataset where the table barely matters for item queries: with no table at all, items touched
rise only 1.1% at an item answer of 0.01% on 1M BioCLIP-2. On the index of review/REPORT.md §1.3 (`e1_anchors.py`, a
different 1M index) the anchor count mattered a lot (items touched at 0.01%: 12.9 with 1,000 anchors, 19.9 with 16,
27.6 with no table), so these figures do not show that 512 or 1,024 coarse anchors are enough there or at 10⁹. That
index's data is not in this container, so `near_table.py` was not run on it.

### 1M BioCLIP-2, against the K × K table of the same fit

`pyA`: 512 coarse + 256 text anchors (half the default's 1,024 coarse anchors, so more pessimistic than the default
at any K above 1,024); `pyB`: K + 256 text (what the default gives at 1M, where K = 1,000 is below 1,024).
Bytes read per item (`bytesq`, 200 queries):

| | item 0.01% | 0.1% | 1% | 10% | text 0.01% | 0.1% | 1% | 10% |
|---|---|---|---|---|---|---|---|---|
| K × K | 24.80 | 36.75 | 60.56 | 129.92 | 19.13 | 28.77 | 62.68 | 197.36 |
| `pyA` | 24.82 | 36.76 | 60.57 | 129.92 | 17.90 | 27.82 | 62.28 | 197.35 |
| `pyB` | 24.80 | 36.75 | 60.56 | 129.92 | 17.93 | 27.85 | 62.32 | 197.35 |

Text items touched at 0.01%: 0.679 (K × K), 0.521 (`pyA`), 0.527 (`pyB`). `pyA` and `pyB` were built with text
anchors from all 2,206 training prompts, so their text gains are the upper bound above, not the genus-held-out
figure. `verify` (80 queries, 7 answer sizes):
no wrong answers, no interval violations, on both. `e2e`, three rounds, median time against K × K: at one thread
0.95–1.06 (`pyA`) and 0.93–1.03 (`pyB`) across the rows; at four threads up to 1.12 (`pyA`, item, 30% answer) with
identical bytes read, where `pyA`'s own rounds ranged 31.8–39.1 ms: this container's four-thread timings move by
that much between rounds. The bytes are the parity measure here. The timings do not establish parity at four threads:
the rounds always ran `pyfit`, `pyA`, `pyB` in that order, and `pyA` was slower than K × K in 7 of 8 item rows at four
threads (mean ratio 1.054), which is order effect or a real cost, not shown to be noise. An alternated rerun (ABBA,
five or more rounds) is still to do.

Build: the table costs part of the encoding pass, rising with A; at 1,256 anchors the f32 table took encoding from
about 49 to 44 s against the f64 one (two alternating rounds each), before the anchor-major layout below. This container's speed moved by about 30% over
the day (the same `pyfit` build took 68 s in the morning and 89 s in the afternoon), so only same-session comparisons
are given.

### Bounded memory with the K × A table

The 4M synthetic build of "Bounded memory" above (256 dimensions, K = 3,999, the default 1,024 coarse anchors), the
K × K build of before this step against this one, alternating on the same fit and partition, in one session:

| | Calibrated | Encoded | Support table | Total | Peak anonymous memory |
|---|---|---|---|---|---|
| K × K, separate pass (2 rounds) | 30, 27 s | +48, +44 s | +109 s | 189, 183 s | 667, 714 MB |
| K × A in the encoding pass, anchor-major f32 (2 rounds, later session) | 25, 25 s | +58, +61 s (table included) | — | 90, 93 s | — |
| K × A, first layout (2 rounds) | 27, 26 s | +77, +73 s (table included) | — | 107, 105 s | 455, 461 MB |

The anchor-major rounds were run in a later session than the other two rows, alternated with the first layout's
build rather than with the K × K one; across sessions this container's speed moves by up to 2.5 times (below), so
compare them with the K × K row loosely. The code hash is the same in all (`6a2af64cdcfa8b43`). The table adds about 15 s to encoding at 4M rows and 1,024
anchors (about 70 GFLOP/s of f32 multiply on the 4 cores); it grows as N·A·n, linear in N, against N·K·n before. The
anonymous memory of the phases is now: fit loaded 52 MB, calibrated 329, encoded 338, assembled 376 at 4M (319 MB peak
at 1M, unchanged); the K² heap of the table phase (+120 MB kept, +300 MB peak) is gone. What still grows with N is
`perm` and the per-cluster records (the assembled phase), which step 5's Part C maps.

This container ran about 2.5 times slower in the afternoon than in the morning (`partition.py` at 4M: 53 s, then 145
s), so the times in this section are only comparable with each other.

## Not done, and not measured

- `perm` and per-cluster records (`Cl2`, the centre tables) mapped rather than read onto the heap, at build and open.
- int8 q·m.
- The item-bytes gap of about 1% at small answers between the Rust fit and `fit.py` at 1M BioCLIP-2.
- Anything above 4M rows; a GPU faiss partition.
- The certificate table that would take Python out of the fit.

## Commands

```bash
python3 -I tools/synth_vectors.py DATA
python3 -I tools/partition.py DATA/x.npy DATA/part [--cells C] [--cross T]
$CARGO_TARGET_DIR/release/vexact fit DATA/rfit --partition DATA/part --basis dual --cert-time 60
$CARGO_TARGET_DIR/release/vexact build DATA/rfit
python3 -I tools/fit.py DATA/x.npy DATA/pyfit --basis dual --cert-time 60
$CARGO_TARGET_DIR/release/vexact anchors DATA/pyfit [--anchors 512] [--text-anchors DATA/text_train.npy]
python3 -I tools/near_table.py DATA/pyfit DATA/heldout.npy DATA/q.npy --nq 200 --text-train DATA/text_train.npy
# genus held out: text_train.npy's rows (agent2/text_train.json order) whose prompt has no genus word of a test prompt
# (first word of a species- or genus-rank name in agent2/text_q.json); then --text-train with that subset
$CARGO_TARGET_DIR/release/vexact --v2 --index DATA/rfit --queries DATA/q.npy --item-queries DATA/heldout.npy bytesq verify
$CARGO_TARGET_DIR/release/vexact --v2 --index DATA/rfit --queries DATA/q.npy --plan-default [--resident] --tag _X e2e --nq 50 --nflat 5
```

`--rebuild` builds the index again in place of opening the saved one, and `--hash` logs the code hash. The cells can
be deleted after the build. The 1M BioCLIP-2 runs used `--cells 32 --cross {1,2,32}` and `--nq 200` for `bytesq`.

## Rebased (2026-11-16)

The same steps re-applied onto `research/exact-vector-search`, gated against its build (`UP`), 4 threads. Reference
code hashes after the calibration change, which is the only intended change of what is encoded: synth4k
`7ac80b6c54101fa9` (unchanged from UP: its 4,000 items are all sampled), 60k `c44827e2e155e64f`, 1M BioCLIP-2 `pyfit`
`b8bdd054938fe563`. Every later step keeps them.

| | item 0.01% / 0.1% / 1% / 10% | text 0.01% / 0.1% / 1% / 10% |
|---|---|---|
| 60k, UP (`--plan-default`) | 7.30 / 13.55 / 43.30 / 113.95 | 183.07 / 242.40 / 407.66 / 873.09 |
| 60k, calibration on items | 7.29 / 13.54 / 43.30 / 113.95 | 183.09 / 242.44 / 407.67 / 873.10 |
| 1M `pyfit`, UP | 24.81 / 36.76 / 60.60 / 130.03 | 19.13 / 28.78 / 62.70 / 197.45 |
| 1M `pyfit`, calibration on items (and after the K × A table) | 24.81 / 36.76 / 60.60 / 130.04 | 19.13 / 28.78 / 62.70 / 197.47 |
| 1M, partition C = 32, T = 32, Rust fit (dual) | 25.48 / 37.59 / 61.61 / 131.06 | 18.99 / 28.58 / 61.98 / 196.77 |
| 1M `pyA` (512 coarse + 256 text anchors) | 24.83 / 36.77 / 60.60 / 130.04 | 17.90 / 27.82 / 62.30 / 197.46 |
| 1M `pyB` (K + 256 text anchors) | 24.81 / 36.76 / 60.60 / 130.04 | 17.94 / 27.86 / 62.34 / 197.46 |

`verify` (200 queries, 7 answer sizes, filtered sets to 1/1000): no wrong item, no interval violation, exact k-NN on
the 60k and 1M `pyfit` indexes, the partitioned 1M index, `pyA` and `pyB`. Partitioned 1M: `partition.py` 78 s,
1,349 MB anonymous peak; the fit 57 s with the certificate (2,505 MB, its Python certificate process). `pyA` and `pyB`'s
`anchors.npy` written again by the rebased code are bit-identical to the pre-rebase ones.

Without `anchors.npy` the support table built in the encoding pass (`hi.bin`, `lo.bin`, `anchors.bin`) is
byte-identical to UP's `build_table2` on synth4k, 60k and 1M `pyfit` (dumped from UP's build).

Bounded memory (synthetic, 256 dimensions, K × K table as UP builds it, 4 threads): 1M build 320 MB peak anonymous,
43 s; 4M 695 MB, 157 s (fit loaded 52, calibrated 326, encoded 333, assembled 374, support table 496 MB), against 317
MB / 34 s and 683 MB / 149 s above, in another session. Code hashes `15cfe6ee75d39ecc` (1M) and `d1dad1a234029a56`
(4M; the 4M partition of the K × A runs above).

Behaviour changes against UP to know of: a fit with more than 4,096 clusters and no `anchors.npy` is refused by the
build (`KK_MAX`; UP builds its K × K table at any K: 8 GB at K = 32,000), with what to do (write anchors); an
`anchors.npy` must carry its sidecar `anchors.json`, which names the fit it was made for (written by `vexact anchors`
and the fit); the support table is mapped at open rather than read.

## For the owner's decision: the levels-clusters defaults (1M BioCLIP-2, 2026-11-16)

`research/levels-clusters` was brought forward with every default off (cd1278d; 4142b9c makes `ddec` an open-time
option). Nothing is decided here: below are the measurements of the four candidate configurations and the options.
Raw logs: the session's `rb/b3/m/` (`build_*`, `bytesq_*`, `verify_*`, `open_*`, `e2e/r1..r6`, `e2e_ratios.txt`).

| | fit | clusters (K) | groups | deterministic levels |
|---|---|---|---|---|
| base | `pyfit`: `fit.py --basis dual`, the default cluster and group rules | 1,000 per cluster (1,000) | 64 | 4, 8, 12, 16, 24, 32 (NDL 6) |
| A | `fitA`: `fit.py --basis dual --per-cluster by-n --groups by-n` | 2,000 per cluster (500; sizes 45..8,947) | 64 | NDL 6 |
| B | `pyfit`, `--dlv 16,32` | 1,000 | 64 | 16, 32 |
| C | `fitA`, `--dlv 16,32` | 2,000 (500) | 64 | 16, 32 |

Both fits use the dual basis (`fit.py`'s default is `pca`); they differ only in the cluster and group rules. The
group rule changes nothing at this N or at 1B: at 1M with K = 500, by-k8 gives 62 groups and by-n 64; at 1B (64,000
per cluster, K = 15,625) both give 64. So A tests the cluster size essentially alone.

Setup: the cd1278d binary, 4 threads (`RAYON_NUM_THREADS=4`), 200 text and 200 held-out item queries for `bytesq` and
`verify`. The builds, opens, `verify` and `e2e` ran with `--plan-default --no-approx`; the `bytesq` runs had neither,
which is harmless there (`bytesq` forces the index path, so neither the plan's constants nor the approximation
enter).

### Build, open, verify (measured)

| | build wall | build peak anonymous | open (3 runs) | open peak anonymous | stored B/item, last level |
|---|---|---|---|---|---|
| base | 75.8 s | 1,121 MB | 0.79–0.85 s | 237–264 MB | 276.0 |
| A | 74.1 s | 1,200 MB | 0.80–0.83 s | 213–228 MB | 276.5 |
| B | 77.0 s | 1,081 MB | 0.79–0.82 s | 235–262 MB | 274.0 |
| C | 75.2 s | 1,226 MB | 0.76–0.79 s | 224–239 MB | 274.5 |

Build and open cost the same within noise. At the build's peak the coarser fit holds more: A 7.0% more than base
(1,200 against 1,121 MB), C 13.4% more than B (1,226 against 1,081 MB). At open it holds less: A 213–228 MB against
base's 237–264, C 224–239 against B's 235–262 (peak anonymous memory of the process; the `[open]` line's anonymous
figure after the open is 214 and 213 MB for A and C, 237 and 235 MB for base and B). `verify` (200 queries × 7 answer sizes, filtered sets, k-NN) is clean on all four: 0 wrong in or out, 0
deterministic and 0 randomised interval violations, 0 k-NN mismatches of 200.

### Bytes read per item on the index path (`bytesq`, measured; change against base)

| | item 0.01% / 0.1% / 1% / 10% | text 0.01% / 0.1% / 1% / 10% |
|---|---|---|
| base | 24.81 / 36.76 / 60.60 / 130.04 | 19.13 / 28.78 / 62.70 / 197.47 |
| A | 26.71 +8% / 38.87 +6% / 63.10 +4% / 132.78 +2% | 20.86 +9% / 30.76 +7% / 65.09 +4% / 202.93 +3% |
| B | 29.82 +20% / 40.39 +10% / 62.71 +3% / 129.92 −0% | 23.04 +20% / 32.45 +13% / 64.96 +4% / 196.43 −1% |
| C | 31.54 +27% / 42.28 +15% / 65.02 +7% / 132.54 +2% | 25.07 +31% / 34.57 +20% / 67.31 +7% / 201.85 +2% |

Touched share (items entering the first level), text 0.01%: base and B .679, A and C .731; item 0.01%: .945 and .963.

### End-to-end time (measured)

Six rounds, the four indexes in one process per round, the order rotated per round (`--index`, `--index-b/c/d`; B
and C get their levels with the per-variant flag `dlv=16.32`). Rounds 1–3 cover every answer size, rounds 4–6 only
0.01%, 0.1%, 1% and the top-10 / top-100 k-NN; 50 queries, 3 runs. The ratio is the variant's median over the base's
median in the same round: the mean, and min..max over rounds. Text rows:

| threads, cell | base ms | A | B | C |
|---|---|---|---|---|
| 1, 0.01% | 34.87 | 0.83 (0.75..0.94) | 0.86 (0.77..0.95) | 0.72 (0.68..0.77) |
| 1, 0.1% | 44.71 | 0.98 (0.93..1.04) | 0.90 (0.86..0.95) | 0.86 (0.84..0.90) |
| 1, top-10 | 29.43 | 0.90 (0.86..0.95) | 0.86 (0.75..0.94) | 0.73 (0.69..0.78) |
| 1, top-100 | 35.82 | 0.85 (0.74..0.96) | 0.81 (0.68..0.94) | 0.72 (0.61..0.78) |
| 4, 0.01% | 11.29 | 0.84 (0.75..0.90) | 0.86 (0.81..0.91) | 0.74 (0.69..0.81) |
| 4, 0.1% | 14.83 | 0.96 (0.89..1.01) | 0.89 (0.81..0.97) | 0.86 (0.79..0.95) |
| 4, top-10 | 10.24 | 0.91 (0.86..0.94) | 0.87 (0.77..0.94) | 0.80 (0.73..0.85) |
| 4, top-100 | 13.25 | 0.94 (0.83..1.01) | 0.86 (0.78..0.98) | 0.77 (0.72..0.85) |

- **Text, small answers:** C is 14–28% faster at 0.01%, 0.1% and both k-NN cells, on 1 and 4 threads; A and B each
  about half of that. But C reads 20–31% more bytes per text query item at 0.01–0.1% on the index path (7% at 1%), and
  B 13–20%; for item queries C reads 15–27% more and B 10–20% (`bytesq` above).
- **Text at 1% and above, and every item row, do not tell the indexes apart.** The mean ratios there are 0.969–1.038
  (single rounds 0.93..1.09), and the base takes 183–190 ms (1 thread) for each threshold cell and 174.7 / 177.0 ms for
  item top-10 / top-100: the planner scans every full vector in those cells (1,536 B per item, `full_reads_per_item`
  1.0), on all four indexes. That is a planner calibration issue, not these options; see below.
- **A against levels-clusters.** `REPORT_SCALING.md`'s −5…−12% for 2,000 per cluster at 1M is time, not bytes. Coarser
  clusters cut less: more items enter the levels (touched .731 against .679 at text 0.01%), so bytes go up (+2–9%),
  while the per-cluster work (cut, tables, setup) falls with K, so time goes down. `REPORT_CLUSTERSIZE.md` shows the
  same at 4,000 per cluster (text 1%: 71.6 B against 67.9). Where this run can compare times (text ≤0.1% and k-NN,
  on the index path), A is 16–17% faster at 0.01% and, on 1 thread, 10–15% at k-NN (4 threads: 9.5% at top-10, 5.7%
at top-100); at 0.1% it is only 2–4% faster, against −12% there.
  That gap is not explained: those runs used other fixed plan constants and a pinned core, and this cell's spread
  (0.93..1.04) is about as wide as the gap.

### Why item queries are a full scan in `e2e` (measured, pre-existing)

`e2e` runs each query as planned (`run2.rs:898`); `bytesq` forces the index path (`PlanMode::Index`, `run2.rs:493`),
so the two count different paths. With `--plan-default`, the planner prices the index path above a scan whenever most
of a cluster's rows remain candidates after the cut, as for these held-out items (d about 0.93): per cluster of 1,000
rows, scoring its rows directly costs `nv·cscan_us + dir_us` = 101 µs, the cut plus the index path 2.5 + 930·(0.02 +
0.08·0.93) + 4 + 20·0.93 = 113 µs (`q.rs:2599`), so the estimate (about 101 ms) is not below the scan's `nrows·cscan_us`
= 100 ms (`q.rs:2479–2480`) and the query scans (`q.rs:2609`). On this machine the constants are off by about 2× in
both directions: the scan takes 0.19 µs a row (`--plan-default`'s `cscan_us` is 0.10) and the index path about 0.045
µs a candidate. Checked with 10 queries on base: item 0.01% planned 191 ms at 1,536 B/item, forced index 41 ms at 21
B/item; text 1% 197 ms against 60 ms. With constants calibrated on this index (no `--plan-default`: `cscan_us` 0.20,
`item_us1` 0.053) the planner picks the index path: item 0.01% 39 ms, text 1% 61 ms. The upstream reference binary
behaves identically (same bytes, same times), so it predates this branch.

To compare the indexes on item queries and large text answers, rerun `e2e` with `--plan index`, with calibrated
constants, or with corrected `--plan-default` constants. (Since "The planner without timing" below, the planner runs
these on the index path and `--plan-default` no longer exists.)

### Memory at 1B (estimated, not measured)

The rule gives 64,000 items per cluster at 1B. The build holds each cluster's rows while encoding them: about 14–16 KB
per item in flight, about 1 GB for an average 64k cluster and 4–5 GB for the largest. The f64 encoder matrices and the
f32 copy of the rows are what needs bounding (in blocks of rows) before that rule can run at 1B.

### The per-variant flags

`dlv=B.B…` and `ddec=L.L…|all|none` set the levels of each `e2e` variant (`--flags` for the first, `--b`/`--c`/`--d`
for the others; `--dlv`/`--ddec` apply only to the first). `dlv` is a build input and in the index's key, so each
schedule is its own saved index (about 1.8 GB at 1M), and a variant with `dlv=` needs its own `--index-X`. `ddec` is
a per-query plan setting (`Plan::ddec`; `--ddec`, as `BuildOptions::ddec`, sets the index's default): it is not in
the key, `index.json` or `plan_hash`, and one opened index serves variants with different `ddec` (`--b ddec=…` with no
`--index-b`). A `ddec` naming a level the index does not have is refused when the plan meets the index; `dlv=` or
`nusort` on a variant that shares the first index is an error (they were silently dropped before). Before 4142b9c each
`ddec` needed its own copy of the index.

### Options

1. **Keep every default off** (base). Fewest bytes; text 0.01% is the slowest of the four.
2. **A: cluster size by N.** Text 0.01% −17%, k-NN −10 to −15% on 1 thread (4 threads: top-10 −9.5%, top-100
   −5.7%); +2–9% bytes on the index path; build time and open time unchanged, build peak +7%.
   At 1B it needs the encoder's per-cluster memory bounded first (above).
3. **B: two deterministic levels (16, 32).** Text −10 to −19% at small answers; bytes at 0.01–0.1% +13–20% for text
   queries, +10–20% for item queries.
4. **C: both.** The fastest (text 14–28% at small answers); bytes at 0.01–0.1% +20–31% for text queries, +15–27% for
   item queries; build peak +13% against B. This conflicts most with the rule that queries stay cheap in bytes read.
5. **Decide after the planner.** Fix or recalibrate the plan constants first and rerun `e2e`, so that items and text
   at ≥1% are measured on the index path. Until then, those rows give no evidence for any option.

## The planner without timing (1M BioCLIP-2, 2026-11-16)

The planner priced every choice in microseconds from constants timed at open (about 10 s at 1M) or, with
`--plan-default`, taken from another machine, which put item queries and text queries at 1% or more on a 180 ms scan
(above, "Why item queries are a full scan"). It now times nothing in memory; `--calib` and `--plan-default` are gone.

**Measured first** (the pre-change binary, `pyfit`, 50 text and 50 held-out item queries, `--no-approx`, 3 runs;
milliseconds as M / I / S / D: timed constants, forced index path, forced whole-query scan, `Plan::default`):

| answer | item, 1 thread | item, 4 threads | text, 1 thread | text, 4 threads |
|---|---|---|---|---|
| 0.01% | 39.9 / 42.6 / 177.6 / 177.6 | 12.2 / 11.8 / 48.0 / 48.1 | 29.4 / 29.6 / 177.9 / 35.5 | 9.2 / 7.7 / 48.6 / 10.2 |
| 0.1% | 49.9 / 49.3 / 177.7 / 181.2 | 13.5 / 13.8 / 47.5 / 48.4 | 37.6 / 37.9 / 183.1 / 46.7 | 9.9 / 10.0 / 47.9 / 14.5 |
| 1% | 55.1 / 59.4 / 178.5 / 181.2 | 18.3 / 16.9 / 49.1 / 49.5 | 55.2 / 55.2 / 182.2 / 182.4 | 15.7 / 15.2 / 48.8 / 47.6 |
| 10% | 80.7 / 81.2 / 181.6 / 180.7 | 23.1 / 24.3 / 47.7 / 50.1 | 87.5 / 87.3 / 179.7 / 181.8 | 25.0 / 24.6 / 48.6 / 49.4 |
| 30% | 100.7 / 99.2 / 181.0 / 184.8 | 29.0 / 28.3 / 48.6 / 50.7 | 109.2 / 106.4 / 182.9 / 187.6 | 31.0 / 30.7 / 49.9 / 50.2 |
| 50% | 99.7 / 103.6 / 181.7 / 181.8 | 28.2 / 28.8 / 50.1 / 51.4 | 116.2 / 116.1 / 184.9 / 189.4 | 34.4 / 33.4 / 50.3 / 51.9 |
| 75% | 93.9 / 95.2 / 182.8 / 186.4 | 28.1 / 27.3 / 50.5 / 51.0 | 107.5 / 108.0 / 189.6 / 194.0 | 32.8 / 32.2 / 50.6 / 52.2 |
| k-NN 10 | 38.0 / 36.7 / 175.1 / 171.3 | 12.0 / 11.4 / 46.9 / 46.4 | 28.7 / 29.3 / 174.3 / 28.3 | 9.0 / 9.1 / 47.3 / 9.3 |
| k-NN 100 | 49.0 / 47.5 / 173.6 / 174.9 | 19.4 / 19.0 / 48.4 / 50.0 | 32.6 / 30.7 / 173.2 / 33.3 | 11.0 / 10.5 / 48.8 / 13.5 |

The forced index path is within noise of the timed planner in every cell and 1.6–6× faster than a scan; the timed
planner never beat it beyond noise. Errors in the constants' ratios toward the index path cost nothing; 2× toward a
scan or full vectors cost up to 80%.

**The rule.** Unfiltered, in memory (`plan2`, `PlanMode::Auto`): the support table skips or takes in what its bounds
decide, and every other live cluster is cut and run on the index path. No whole-query scan, direct scoring or
full-vector path, and no sample or answer-share estimate. This needs no timing because nothing is compared: in the
unfiltered case one path was best or tied everywhere, and its own cost (the cut, the levels, the full vectors of the
last survivors) is already proportional to what the bounds leave, so there is nothing for a constant to trade.

Under a visible set, one constant in rows, `Plan::r_rows` = R = 300: the set is scanned when it holds at most R rows
per cluster holding a visible row; otherwise a cluster the support table leaves open is scored directly when it holds
at most R visible rows, and cut and run on the index path when it holds more. Filtered exact k-NN scans before the
seed when the set holds at most R rows per such cluster plus `r_seed` = 6,000 rows (the seed: 1.18–1.45 ms against
0.19 µs a row, so 6,100–7,600; the lower end). Both are ratios of costs of the same kind (a pass over memory per row),
so they do not move with the machine's speed the way each microsecond cost does.

R is not the cut's cost in rows (2.35–2.48 µs / 0.190–0.195 µs = 12–13): under a visible set the index path on a
cluster costs much more than its cut. With R = 12, 2–30% visible sets took the index path at 1.3–7.5× a scan's time
(2%: 32.4 ms against 4.3; 10%: 53.0 against 20.8; 30%: 77.4 against 61.1). R comes from timing the planner at several
R (text queries at five thresholds, `dbcmp --step 8 --reps 3`, random visible sets, 1,000 rows a cluster; ms):

| threads | visible | forced index | forced scan | R = 150 | R = 200 | R = 250 | R = 300 | R = 400 |
|---|---|---|---|---|---|---|---|---|
| 1 | 10% | 48.0 | 20.3 | 20.3 | 20.2 | 20.2 | 20.1 | 20.2 |
| 1 | 20% | 57.1 | 39.8 | 52.4 | 39.8 | 39.7 | 40.0 | 39.7 |
| 1 | 30% | 68.7 | 59.0 | 69.6 | 66.0 | 65.4 | 59.0 | 59.1 |
| 1 | 40% | 73.0 | 77.3 | 68.4 | 68.0 | 69.9 | 72.5 | 75.5 |
| 1 | 50% | 73.8 | 95.3 | 73.7 | 73.1 | 72.5 | 71.8 | 77.6 |
| 1 | 70% | 73.9 | 132.6 | 71.6 | 72.9 | 73.9 | 73.2 | 76.3 |
| 4 | 10% | 16.5 | 5.9 | 5.5 | 5.2 | 4.9 | 4.6 | 4.5 |
| 4 | 20% | 18.0 | 11.1 | 16.4 | 11.1 | 10.9 | 10.9 | 10.7 |
| 4 | 30% | 21.0 | 16.3 | 20.3 | 19.2 | 19.3 | 16.3 | 16.1 |
| 4 | 40% | 21.0 | 20.7 | 20.8 | 20.4 | 20.7 | 20.8 | 21.0 |
| 4 | 50% | 21.7 | 26.1 | 22.4 | 22.2 | 22.3 | 22.1 | 22.6 |
| 4 | 70% | 24.0 | 38.0 | 22.7 | 23.0 | 23.2 | 23.8 | 25.2 |

The index path per cluster holding a visible row costs about 150 rows of scan at 10 visible rows a cluster, 250 at
100 and 400 at 500; a whole query's scan and index path cross at 350–400 visible rows a cluster. R = 300 sits below,
toward the index path; its worst cell is 40% at one thread (+6% against R = 200).

**Speed, unfiltered** (the new binary against the old one with the timed constants `M`, each against its own forced
index path, which runs the same code in both; 4 rounds, binaries and variant order alternated, `e2e --nq 50 --runs 3
--knn-k 10,100`; ms as new / M, and in brackets (new / its forced index) ÷ (M / its forced index), mean over rounds):

| answer | item, 1 thread | item, 4 threads | text, 1 thread | text, 4 threads |
|---|---|---|---|---|
| 0.01% | 45.6 / 44.5 (1.001) | 12.8 / 14.1 (0.916) | 31.8 / 31.4 (0.992) | 9.8 / 9.7 (0.987) |
| 0.1% | 51.8 / 50.8 (0.996) | 15.0 / 15.3 (1.035) | 38.2 / 38.4 (0.998) | 11.3 / 11.3 (0.953) |
| 1% | 60.8 / 61.5 (0.966) | 17.3 / 18.8 (0.944) | 57.9 / 57.2 (0.997) | 16.8 / 17.1 (0.969) |
| 10% | 85.8 / 84.9 (1.026) | 23.9 / 24.6 (1.002) | 90.7 / 90.1 (1.002) | 26.3 / 26.6 (0.982) |
| 30% | 102.6 / 102.6 (0.991) | 29.2 / 30.0 (0.995) | 114.2 / 114.4 (0.987) | 33.6 / 33.0 (0.992) |
| 50% | 104.9 / 107.9 (0.983) | 30.7 / 31.5 (0.978) | 118.8 / 119.3 (1.008) | 35.0 / 35.7 (0.969) |
| 75% | 97.7 / 99.0 (0.991) | 29.2 / 29.6 (0.985) | 113.4 / 113.8 (0.982) | 34.4 / 34.2 (0.985) |
| k-NN 10 | 39.2 / 38.1 (1.055) | 11.9 / 12.5 (0.947) | 28.5 / 28.3 (0.955) | 9.3 / 9.2 (0.878) |
| k-NN 100 | 51.7 / 50.4 (1.024) | 20.1 / 20.5 (0.995) | 33.6 / 32.4 (1.010) | 11.6 / 11.7 (0.949) |

Mean 0.984 over the 36 cells, 0.878–1.055; the largest (item k-NN 10, one thread) has rounds 0.98–1.07 against
0.93–1.00, within the spread. The new planner runs every open cluster on the index path; M sent 7–17 a query to full
vectors or direct scoring for item queries, 7–100 for text (the most at 0.01% and at k-NN).

**Speed, filtered** (`dbcmp`, 93 text queries × 5 thresholds, 3 reps, 2 rounds alternating the binaries; medians, ms):

| threads | visible | M | new | forced index | forced scan | new / M |
|---|---|---|---|---|---|---|
| 1 | 0.1% | 0.16 | 0.15 | 13.95 | 0.14 | 0.94 |
| 1 | 1% | 1.60 | 1.46 | 28.91 | 1.51 | 0.91 |
| 1 | 10% | 19.98 | 19.58 | 46.13 | 19.48 | 0.98 |
| 1 | 50% | 70.94 | 72.10 | 70.71 | 94.58 | 1.02 |
| 4 | 0.1% | 0.15 | 0.14 | 4.56 | 0.12 | 0.87 |
| 4 | 1% | 0.61 | 0.53 | 8.76 | 0.55 | 0.88 |
| 4 | 10% | 5.96 | 5.07 | 14.60 | 4.95 | 0.85 |
| 4 | 50% | 22.56 | 21.87 | 21.60 | 25.95 | 0.97 |

The new rule scans at 0.1–10% (M scanned 77% of the 10% runs) and takes the index path at 50%, within 8% of the better
forced path (the largest gaps at 0.1%, 0.01–0.02 ms). Answers: the `dbcmp` dumps (unfiltered and the four sets, every
variant) are byte-identical between the binaries and, where one exists (unfiltered and 1%), to the upstream reference
dump; 1M `verify` is clean; the code hashes are unchanged.

**Vectors on file.** Where reads are priced (`--full-file`, read through io_uring), the old cost model stays
(`plan2_priced`): its reads at `io_us`/`io_gbps`, timed by `Plan::calibrate_disk` once per machine and cached in the
index directory (`disk-constants.txt`, keyed by the CPU model and the vectors file's device; timed again when missing
or under another key, and logged), its compute at fixed constants (`q::COSTS`, the values timed on this machine:
full vector 0.19 µs, index path 0.0133 + 0.053·d per candidate and 2.67 + 13.3·d per cluster, group 93 µs, scan
0.19/0.18 µs a row, anchor search 430 µs, direct 1.08 µs, cut 2.45 µs). `knn2_fast` uses the same `COSTS`. Measured
here: io_us 4.90, io_gbps 2.01 at the first open (0.1 s), read from the cache at the second. `--plan-file` takes
`r_rows`, `r_seed`, `io_us`, `io_gbps` only.

**Open:** 0.83–0.92 s in all at 1M (wall 0.9 s), against 11.3 s with the timing pass.

**Caveats.** One machine (4 cores, AVX-512) and one corpus (1M BioCLIP-2, 768 dimensions, 1,000 rows a cluster).
The unfiltered rule should hold wherever the index path beats a scan by this margin; at 1B, clusters hold about
64,000 items, so cutting and running a cluster costs relatively less, which favours the index path further. R was
measured at 1,000 rows a cluster: the index path's cost per cluster under a visible set grows with the cluster's
rows, so at other cluster sizes the crossover may move roughly with the cluster's size; that is not measured. R is
dimensionless but its value comes from this machine. Raw logs: the session's `plan3b/` (`e2e/`, `e2e_table.md`,
`fl/`, `fl_table.md`, `sweep/`, `sweep2/`, `ans/`, `disk/`, `open/`).
