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
Raw logs: the session's `rb/b3/m/` (`build_*`, `bytesq_*`, `verify_*`, `open_*`, `e2e/r1..r6`, `e2e_ratios.txt`);
the index-path times, encoded again with the planner without timing: `b3t/` (`build_*`, `bytesq_*`, `verify_*`,
`e2e/r1..r3`, `table.txt`, `meas.sh`).

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
enter). The `e2e` times below are from the 3762490 binary (the planner without timing), not cd1278d.

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

### End-to-end time on the index path (measured, the planner without timing)

Re-timed with the planner of "The planner without timing" below (the f47dbe9 library, unchanged at 3762490;
`--no-approx`, no plan constants to give). The library changed, so each index was encoded again (the same fits and
options); the code hashes are those of batch 3 (base `b8bdd054938fe563`, A `08e9fb204de3da37`, B `c8779c606204880b`,
C `771eaba1d111dca4`), the store hashes too, and `bytesq` gives the bytes above to the digit. `verify` (200 queries ×
7 answer sizes, filtered sets, k-NN; this planner) is clean again on all four: 0 wrong in or out, 0 interval
violations, 0 k-NN mismatches of 200. Three rounds, the four indexes in one process per round, the order rotated per
round (`--index`, `--index-b/c/d`, `dlv=16.32` for B and C); answers 0.01%, 0.1%, 1% and the top-10 / top-100 k-NN;
1 and 4 threads; 50 text and 50 held-out item queries, 3 runs. Every query of every index ran on the index path (no
scan; 1 or 2 clusters a query scored directly, the rest cut). Base's median in ms (mean over rounds); for A, B and C the ratio of the variant's
median to the base's in the same round: the mean, and min..max over the 3 rounds.

| class, threads, cell | base ms | A | B | C |
|---|---|---|---|---|
| item, 1, 0.01% | 45.79 | 0.91 (0.91..0.92) | 0.85 (0.84..0.86) | 0.75 (0.73..0.76) |
| item, 1, 0.1% | 52.93 | 0.89 (0.88..0.90) | 0.86 (0.85..0.88) | 0.76 (0.75..0.77) |
| item, 1, 1% | 60.76 | 0.90 (0.89..0.91) | 0.86 (0.85..0.87) | 0.80 (0.79..0.81) |
| item, 1, top-10 | 40.23 | 0.94 (0.91..0.99) | 0.79 (0.78..0.82) | 0.72 (0.70..0.74) |
| item, 1, top-100 | 52.57 | 0.95 (0.93..0.98) | 0.80 (0.79..0.81) | 0.73 (0.72..0.74) |
| item, 4, 0.01% | 13.88 | 0.88 (0.86..0.89) | 0.85 (0.85..0.86) | 0.75 (0.74..0.76) |
| item, 4, 0.1% | 15.65 | 0.89 (0.86..0.92) | 0.87 (0.85..0.88) | 0.77 (0.74..0.79) |
| item, 4, 1% | 18.95 | 0.85 (0.83..0.87) | 0.85 (0.83..0.88) | 0.77 (0.75..0.79) |
| item, 4, top-10 | 13.16 | 0.93 (0.90..0.96) | 0.80 (0.78..0.82) | 0.73 (0.71..0.76) |
| item, 4, top-100 | 21.13 | 0.90 (0.88..0.93) | 0.78 (0.75..0.80) | 0.70 (0.68..0.73) |
| text, 1, 0.01% | 33.41 | 0.87 (0.84..0.90) | 0.82 (0.81..0.83) | 0.71 (0.70..0.73) |
| text, 1, 0.1% | 38.89 | 0.89 (0.88..0.91) | 0.86 (0.86..0.87) | 0.75 (0.73..0.78) |
| text, 1, 1% | 57.42 | 0.90 (0.89..0.91) | 0.88 (0.87..0.88) | 0.79 (0.78..0.81) |
| text, 1, top-10 | 29.57 | 0.86 (0.86..0.86) | 0.85 (0.82..0.88) | 0.71 (0.70..0.72) |
| text, 1, top-100 | 32.64 | 0.92 (0.87..0.96) | 0.84 (0.83..0.87) | 0.74 (0.73..0.75) |
| text, 4, 0.01% | 10.46 | 0.82 (0.76..0.85) | 0.88 (0.84..0.90) | 0.71 (0.68..0.73) |
| text, 4, 0.1% | 12.07 | 0.87 (0.83..0.89) | 0.85 (0.81..0.87) | 0.73 (0.71..0.75) |
| text, 4, 1% | 17.44 | 0.92 (0.91..0.94) | 0.89 (0.87..0.92) | 0.82 (0.80..0.85) |
| text, 4, top-10 | 9.53 | 0.92 (0.88..0.99) | 0.88 (0.84..0.95) | 0.74 (0.72..0.77) |
| text, 4, top-100 | 11.94 | 1.00 (0.99..1.01) | 0.87 (0.79..0.92) | 0.81 (0.79..0.83) |

Bytes read per item on the path taken (`e2e`, 50 queries; change against base):

| | base | A | B | C |
|---|---|---|---|---|
| item 0.01% | 27.18 | 29.30 +8% | 31.96 +18% | 33.88 +25% |
| item 0.1% | 39.58 | 41.94 +6% | 43.10 +9% | 45.22 +14% |
| item 1% | 64.13 | 66.92 +4% | 66.22 +3% | 68.81 +7% |
| item top-10 | 25.18 | 27.63 +10% | 27.88 +11% | 28.93 +15% |
| item top-100 | 42.06 | 45.15 +7% | 42.74 +2% | 44.95 +7% |
| text 0.01% | 16.33 | 17.97 +10% | 20.42 +25% | 22.43 +37% |
| text 0.1% | 25.15 | 27.04 +8% | 29.06 +16% | 31.16 +24% |
| text 1% | 58.59 | 60.94 +4% | 61.19 +4% | 63.51 +8% |
| text top-10 | 15.63 | 17.81 +14% | 18.15 +16% | 20.43 +31% |
| text top-100 | 21.39 | 24.01 +12% | 23.97 +12% | 26.30 +23% |

- **C is the fastest in every cell, then B, then A, base last.** C is 17–30% faster than base (ratios 0.70–0.83; no
  round above 0.85), B 11–22% (0.78–0.89), A 5–18% (0.82–0.95), and A ties base at text top-100 on 4 threads (1.00,
  0.99..1.01). A round's ratio lies within 0.08 of the mean. Of the 60 cell-rounds, C beats B in 59 and A and base
  in all; B beats A in 53 (A is ahead at text 0.01% on 4 threads in every round, 0.82 against 0.88). Items and text
  gain alike, 1 and 4 threads alike. The two options compose about multiplicatively: C is within 0.04 of A × B in 18
  of 20 cells (text k-NN on 4 threads: C 0.06 better).
- **Bytes go the other way, in the same order.** C reads 7–37% more than base on the path taken, B 2–25%, A 4–14%;
  the gap is widest at 0.01% and text k-NN (C: +37% and +31%) and closes to +3–8% at 1%, as the time gain does
  (C 0.71–0.75 at 0.01%, 0.77–0.82 at 1%).
- **Superseded:** the batch 3 `e2e` (`rb/b3/m/e2e`, `--plan-default`) ran item queries and text at 1% or more as a
  scan of all 1M full vectors on every index (about 180 ms on 1 thread; ratios 0.97–1.04), so it could not tell the
  indexes apart there; its text rows at ≤0.1% and k-NN were on the index path and agree with the above (C 0.72–0.86).
  Item queries now separate the variants as text does.
- **A against levels-clusters.** `REPORT_SCALING.md`'s −5…−12% for 2,000 per cluster at 1M is time, not bytes.
  Coarser clusters cut less: more items enter the levels (touched .731 against .679 at text 0.01%), so bytes go up,
  while the per-cluster work (cut, tables, setup) falls with K, so time goes down. `REPORT_CLUSTERSIZE.md` shows the
  same at 4,000 per cluster (text 1%: 71.6 B against 67.9). Here A is 5–18% faster (a tie at text top-100 on 4
  threads); the batch 3 gap at text 0.1% (0.98 there, against −12%) does not recur (0.89 and 0.87).

### Why item queries were a full scan in the batch 3 `e2e` (superseded)

With `--plan-default` the old planner priced the index path above a scan whenever most of a cluster's rows remained
candidates after the cut, as for these held-out items, from microsecond constants that were off by about 2× in both
directions on this machine (a scan 0.19 µs a row against 0.10 assumed; the index path about 0.045 µs a candidate).
Checked then on base with 10 queries: item 0.01% planned 191 ms, forced index path 41 ms; text 1% 197 against 60 ms.
The planner without timing (below) takes the index path in all these cells (above).

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

1. **Keep every default off** (base). Fewest bytes; the slowest of the four in every cell measured.
2. **A: cluster size by N.** 5–18% faster (item and text, 1 and 4 threads; text top-100 on 4 threads a tie); +4–14%
   bytes on the path taken; build time and open time unchanged, build peak +7%. At 1B it needs the encoder's
   per-cluster memory bounded first (above).
3. **B: two deterministic levels (16, 32).** 11–22% faster; +2–25% bytes (+25% at text 0.01%, +18% at item 0.01%).
4. **C: both.** The fastest everywhere, 17–30%; +7–37% bytes (+37% at text 0.01%, +31% at text top-10, +25% at item
   0.01%); build peak +13% against B. This conflicts most with the rule that queries stay cheap in bytes read.

These rest on the index-path times above; the batch 3 `e2e` table (old planner) is superseded.

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

**The rule.** In memory (reads not priced; `plan2`, `PlanMode::Auto`), in rows, with or without a visible set (without
one, a cluster's visible rows are all its rows): the query scans the visible rows when they number at most
Σ (R0 + κ·S) over the clusters holding one (S a cluster's rows), a test made first, before the anchor search and the
support table, since it needs neither. Otherwise the support table skips or takes in what its bounds decide, and a
cluster it leaves open is scored directly when it holds at most R0 + κ·S visible rows, else cut and run on the index
path. `Plan::r_rows` = R0 = 64, `r_kappa` = κ = 0.24 (fitted below). No sample or answer-share estimate. Filtered
exact k-NN (and unfiltered k-NN in memory) scans before the seed when the rows are at most that sum plus `r_seed` =
6,000 rows (the seed: 1.18–1.45 ms against 0.19 µs a row, so 6,200–7,600; the lower end). The seed is the anchor
search, a probe of 64 clusters to 2,000 items and 2k + 8 exact products: it grows with the number of anchors and with
k, not with the clusters' rows, so beyond k = 100 the constant underestimates it, which leans toward the index path.
All are ratios of costs of the same kind (a pass over memory per row), so they do not move with the machine's speed
the way each microsecond cost does.

Without a visible set, a cluster is scored directly only when it holds at most R0 / (1 − κ) = 84 rows: on a full index
(`pyfit`: 3 of 1,000 clusters hold 100 rows or fewer) every open cluster takes the index path, where the measurement
above found it best or tied everywhere, and its own cost (the cut, the levels, the full vectors of the last
survivors) is already proportional to what the bounds leave. An index compacted to a visible set (`Index2::compact`,
queried with no visible set) holds only the set's rows in each cluster, so the rule scans it as it would scan the set
on the full index; the first version of this planner sent every cluster of such an index to the index path (below).

The first version of the rule had one constant, R = 300 rows per cluster (κ = 0, and no rule without a visible set),
measured as follows at 1,000 rows a cluster. R is not the cut's cost in rows (2.35–2.48 µs / 0.190–0.195 µs = 12–13): under a visible set the index path on a
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

**R and the cluster size (measured).** Per cluster the index path costs roughly A + γ·d·S + B·(candidates). The
candidate lists used to test every row of the cut's range; they are now built a word of the visible set at a time
(`Vis2::each`, also for the rows the key range takes in), with the same answers. That changed the forced index path's
time within noise (1%: 33.0 ms against 32.8, 10%: 48.4 against 48.0, one thread): the cost that grows with S is
elsewhere, most likely in running dense enough candidates in place, on whole blocks of the range with their holes.
So κ does not vanish. It was fitted by refitting the 1M BioCLIP-2 vectors at larger clusters (`fit.py --basis dual
--per-cluster 4000|16000 --groups by-n`, as `fitA`; K = 250, sizes 443..23,638, 64 groups; K = 62, sizes
4,515..49,837, 62 groups, each cluster its own group since by-k8 would give 7), each built in memory (`--in-memory`) and
deleted after its sweep: forced index path against forced scan, text queries at five thresholds, random visible sets
of 1, 2, 5, 10, 20, 30, 40 and 50% (`dbcmp --what idx,scan --step 8 --reps 3`, medians; the ratio is index / scan):

| rows a cluster (K) | threads | 1% | 5% | 10% | 20% | 30% | 40% | 50% | crossover | rows a cluster at it |
|---|---|---|---|---|---|---|---|---|---|---|
| 1,000 (1,000) | 1 | 14.73 | 4.13 | 2.34 | 1.47 | 1.06 | 0.84 | 0.68 | 32.2% | 322 |
| 1,000 (1,000) | 4 | 12.69 | 4.59 | 2.69 | 1.65 | 1.21 | 0.98 | 0.77 | 38.8% | 388 |
| 4,000 (250) | 1 | 9.38 | 2.93 | 1.71 | 1.25 | 0.90 | 0.72 | 0.58 | 26.3% | 1,053 |
| 4,000 (250) | 4 | 8.75 | 3.35 | 1.96 | 1.47 | 1.07 | 0.83 | 0.69 | 32.3% | 1,293 |
| 16,129 (62) | 1 | 6.83 | 2.66 | 1.61 | 1.18 | 0.88 | 0.69 | 0.57 | 25.1% | 4,048 |
| 16,129 (62) | 4 | 7.32 | 2.94 | 1.82 | 1.37 | 1.03 | 0.83 | 0.68 | 31.0% | 5,000 |

The crossover in visible rows a cluster is a line in S: 72 + 0.246·S on one thread and 78 + 0.305·S on four, each
within 6 rows of its three points; a constant R would be off by 4× at 4,000 rows a cluster and 13× at 16,000. R0 = 64
and κ = 0.24 sit just below both lines, toward the index path (304 rows at 1,000 a cluster, the old R = 300; 1,024 at
4,000; 3,934 at 16,129). (The crossover is the share where the index / scan ratio crosses 1, interpolated in log
share; the clusters are of unequal size, and a random set gives each the same share.) Raw logs: `plan3c/sweep3/`.

**Compacted sessions (measured).** `dbcmp`'s `cindex`/`cvindex` query the index compacted to the session's visible
set (codes only; codes and vectors) with no visible set. The first version of this planner took every such cluster
to the index path; the rule now scans them. 93 text queries × 5 thresholds, `--step 4 --reps 3`, medians in ms; before
is the previous binary (2f1bc58), two runs, after the final binary:

| session | threads | cindex before | cvindex before | cindex after | cvindex after | its scan (cscan / cvscan) |
|---|---|---|---|---|---|---|
| 0.1% | 1 | 11.7, 13.8 | 11.4, 13.9 | 0.23 | 0.16 | 0.13–0.20 / 0.08–0.14 |
| 0.1% | 4 | 4.5, 4.3 | 4.4, 4.2 | 0.19 | 0.13 | 0.13–0.20 / 0.10–0.14 |
| 1% | 1 | 26.1, 24.8 | 26.2, 24.9 | 2.27 | 1.84 | 1.04–2.31 / 0.74–1.93 |
| 1% | 4 | 8.4, 7.7 | 8.3, 7.5 | 0.73 | 0.60 | 0.48–0.92 / 0.38–0.74 |

All 1,395 runs of each scanned (the same scan as `cscan`/`cvscan`; in the after run those ran second on each query,
with warmer caches, and took 0.13 and 0.08 ms at 0.1%, 1.04 and 0.74 ms at 1%, one thread). Larger compacted sessions (codes only, `--step 8`; ms for the rule / forced index path
/ scan): 5%, 1 thread 10.2 / 29.2 / 10.4, 4 threads 2.0 / 10.1 / 2.9 (scanned); 10%, 28.9 / 32.6 / 20.7 and
9.7 / 10.8 / 6.2 (about 100 rows a cluster, above the rule's 84: the index path, 1.4–1.6× the scan); 20%, 35.0 / 35.4
/ 39.9 and 11.0 / 10.7 / 11.0. So on a compacted index the scan and the index path cross at about 150–200 rows a
cluster, above the 84 the filtered fit gives: the rule leans toward the index path there, by up to 1.6× at 10%. Raw
logs: `plan3c/comp/`.

**Speed after the rule by cluster size (quick check).** Unfiltered (`e2e`, answers up to 1% and top-10/100, 2 rounds,
binaries and variant order alternated; the final binary N against the old one with the timed constants M, each with
its own forced index path): the mean of (N / its forced index) ÷ (M / its forced index) over the 20 cells is 0.991
(0.934–1.072); N runs 2 clusters a query directly (those of at most 84 rows), the rest on the index path. The largest
cells are text k-NN at 4 threads (top-10 1.045, top-100 1.060) and text top-100 at one thread (1.072), each with a
round within 0.98–1.03. Filtered (`fl.sh`, 2 rounds; ms, M / new / forced index / forced scan): 0.1% 0.17 / 0.19 /
17.15 / 0.15 and 0.15 / 0.16 / 4.72 / 0.13 (1 and 4 threads); 1% 1.65 / 1.69 / 30.80 / 1.57 and 0.61 / 0.66 / 9.51 /
0.57; 10% 21.84 / 21.50 / 48.25 / 21.13 and 6.28 / 5.80 / 14.84 / 5.68; 50% 70.44 / 66.00 / 68.46 / 99.47 and 22.34 /
20.53 / 21.15 / 27.17. Both planners scan every 0.1% and 1% run; at 10% the new one scans all (M 77%), at 50% none.
At 0.1–1% the new planner is 0.01–0.05 ms above M and 0.01–0.09 ms above its forced scan, the same scan behind a plan; at 10–50% it is
within 2% of the better forced path or below it. Raw logs: `plan3c/e2e/`, `e2e_table.md`, `fl/`, `fl_table.md`.
Answers: 1M `verify` clean; the `dbcmp` dumps (unfiltered and the four sets) byte-identical to the old binary's.

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
0.93–1.00, within the spread. (These tables are of the first version, R = 300; the rows rule by cluster size below now also
scores clusters of 84 rows or fewer directly, about 2 a query on this index.) That version ran every open cluster on
the index path; M sent 7–17 a query to full
vectors or direct scoring for item queries, 7–100 for text (the most at 0.01% and at k-NN). Unfiltered on these
clusters that version and its forced index path were the same code (the support table, then the index path on
every open cluster), so the new / forced-index ratio is 1 by construction and says nothing on its own; the evidence
that the new planner is no slower than M is that M was within noise of the old binary's own forced index path
(M ≈ I_old, the bracketed ratios, and the first table's M against I).

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

The new rule scans at 0.1–10% (M scanned 77% of the 10% runs) and takes the index path at 50%, within 8.4% of the better
forced path (the largest gaps at 0.1%, 0.01–0.02 ms). Answers: the `dbcmp` dumps (unfiltered and the four sets, every
variant) are byte-identical between the binaries and, where one exists (unfiltered and 1%), to the upstream reference
dump; 1M `verify` is clean; the code hashes are unchanged.

**Vectors on file.** Where reads are priced (`--full-file`, read through io_uring), the old cost model stays
(`plan2_priced`): its reads at `io_us`/`io_gbps`, timed by `Plan::calibrate_disk` once per machine and cached in the
index directory (`disk-constants.txt`; timed again when missing or under another key, and logged), its compute at
fixed constants (`q::COSTS`, the values timed on this machine: full vector 0.19 µs, index path 0.0133 + 0.053·d per
candidate and 2.67 + 13.3·d per cluster, group 93 µs, scan 0.19/0.18 µs a row, anchor search 430 µs, direct 1.08 µs,
cut 2.45 µs). The key (`q::disk_key`) holds a format version, the machine (/etc/machine-id, else the host name), the
CPU (model name; on aarch64, whose /proc/cpuinfo has none, implementer, architecture, variant, part and revision), the
device (major:minor, name, and the model and serial of the device or of a partition's disk, where /sys gives them),
the file system's type and the read policy the timing reads with (reads in flight, the O_DIRECT and RWF_DONTCACHE
thresholds). Two opens that find no cache time one at a time (an flock on `disk-constants.lock`; the second reads
what the first saved), and the cache is written through a temporary file of its own name, synced, then renamed. In
an index directory that cannot be written, the cache is the per-user one (`$XDG_CACHE_HOME/vexact` or
`~/.cache/vexact`, a file named by the key); an open never fails on the cache. Timing drops the vectors file from the
page cache (`Rows::evict`, before each timing and after), so the first queries after it read from the device.
Measured here: io_us 4.90, io_gbps 2.01 at the first open (0.1 s), read from the cache at the second.
`--plan-file` takes `r_rows`, `r_kappa`, `r_seed`, `io_us`, `io_gbps` only (fractional rows, a disk constant of 0 or
below and a negative κ are errors); when it gives both disk constants nothing is timed or evicted. `knn2_fast` uses
the same `COSTS`: its cut-off between scoring a cluster's visible items and its index path is now about 129 visible
items (it was about 240 under the old default constants), so approximate results with a dial can differ from before.
A library user who prices reads calls `q::disk_constants` or sets `io_us`/`io_gbps` (`Plan::default`'s are
placeholders); "in memory" means reads are not priced, so a mapped vectors file that is not resident counts as in
memory.

**Open:** 0.83–0.92 s in all at 1M (wall 0.9 s), against 11.3 s with the timing pass.

**Caveats.** One machine (4 cores, AVX-512) and one corpus (1M BioCLIP-2, 768 dimensions). The rule's dependence on
the cluster size was measured at 1,000, 4,000 and 16,129 rows a cluster (above): linear, with κ well above 0. At 1B
clusters hold about 64,000 rows, beyond the measured range: the rule gives 15,400 visible rows a cluster (24%). The
crossover shares measured fall with S but flatten (32, 26, 25% on one thread; 39, 32, 31% on four), toward the rule's
share; if they kept falling below 24% at 64,000 rows, the rule would score directly or scan a little past the
crossover there (the harmful direction, by that margin). Not measured. R0 and κ are dimensionless but their values come from this machine. Raw logs: the session's
`plan3b/` (`e2e/`, `e2e_table.md`, `fl/`, `fl_table.md`, `sweep/`, `sweep2/`, `ans/`, `disk/`, `open/`) and
`plan3c/` (`sweep3/`, `comp/`, `e2e/`, `fl/`, `ans/`).
