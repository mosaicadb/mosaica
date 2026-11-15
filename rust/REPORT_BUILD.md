# The build: a faster encoder, a saved index, the fit in Rust, the partitioned build, the K × A table

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

A sparse table of each anchor's nearest clusters does no better than no table: the clusters a query cannot skip are
the far ones, which the lists leave out. What sets the cost is how near the best anchor is to the query, not how many
clusters each anchor covers, so a dense table against fewer anchors keeps almost all of the K × K table's effect, and
anchors placed where queries fall (k-means of embedded training prompts) cut text queries' items touched by a quarter.
The text figure is optimistic: the training and test prompts come from the research branch's prompt sets and may
share templates. Item thresholds (t = 0.9) lose 1.2% at 512 coarse anchors.

`vexact fit` writes `anchors.npy`: all K centre directions when `--anchors` (1,024 by default) is at least K, else
k-means of them, plus `--text-count` (256) k-means centres of the prompts given with `--text-anchors`; `vexact anchors`
rewrites it for an existing fit. Without `anchors.npy` the table is the K × K one, byte-identical to before (code hash
`b8bdd054938fe563`). The table is computed in f32 and each extreme recomputed in f64 over the items within twice the
f32 error bound of the f32 extreme, so it equals the f64 table (`hi.bin`, `lo.bin` byte-identical at 768 and 1,256
anchors).

### 1M BioCLIP-2, against the K × K table of the same fit

`pyA`: 512 coarse + 256 text anchors (what the default gives at 1B scale); `pyB`: K + 256 text (the default at 1M).
Bytes read per item (`bytesq`, 200 queries):

| | item 0.01% | 0.1% | 1% | 10% | text 0.01% | 0.1% | 1% | 10% |
|---|---|---|---|---|---|---|---|---|
| K × K | 24.80 | 36.75 | 60.56 | 129.92 | 19.13 | 28.77 | 62.68 | 197.36 |
| `pyA` | 24.82 | 36.76 | 60.57 | 129.92 | 17.90 | 27.82 | 62.28 | 197.35 |
| `pyB` | 24.80 | 36.75 | 60.56 | 129.92 | 17.93 | 27.85 | 62.32 | 197.35 |

Text items touched at 0.01%: 0.679 (K × K), 0.521 (`pyA`), 0.527 (`pyB`). `verify` (80 queries, 7 answer sizes):
no wrong answers, no interval violations, on both. `e2e`, three rounds, median time against K × K: at one thread
0.95–1.06 (`pyA`) and 0.93–1.03 (`pyB`) across the rows; at four threads up to 1.12 (`pyA`, item, 30% answer) with
identical bytes read, where `pyA`'s own rounds ranged 31.8–39.1 ms: this container's four-thread timings move by
that much between rounds. The bytes are the parity measure here; the timings show no regression beyond that noise.

Build: the table costs part of the encoding pass, rising with A; at 1,256 anchors the f32 table took encoding from
about 49 to 44 s against the f64 one (two alternating rounds each). This container's speed moved by about 30% over
the day (the same `pyfit` build took 68 s in the morning and 89 s in the afternoon), so only same-session comparisons
are given.

### Bounded memory with the K × A table

(The 1M and 4M synthetic runs are being repeated; figures to follow.)

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
$CARGO_TARGET_DIR/release/vexact --v2 --index DATA/rfit --queries DATA/q.npy --item-queries DATA/heldout.npy bytesq verify
$CARGO_TARGET_DIR/release/vexact --v2 --index DATA/rfit --queries DATA/q.npy --plan-default [--resident] --tag _X e2e --nq 50 --nflat 5
```

`--rebuild` builds the index again in place of opening the saved one, and `--hash` logs the code hash. The cells can
be deleted after the build. The 1M BioCLIP-2 runs used `--cells 32 --cross {1,2,32}` and `--nq 200` for `bytesq`.
