# The build: a faster encoder, a saved index, and the fit in Rust

2026-11-14. Every figure is from the 60,000-row synthetic set of `tools/synth_vectors.py` (768 dimensions, a mixture
of 300 Gaussian clusters with a decaying spectrum; 200 held-out items from the same mixture as item queries; 50 points
of the mixture without the data's rotation as an out-of-distribution stand-in, called text below), on a 4-core Xeon at
2.3 GHz (AVX-512) in a container. Nothing here was run on BioCLIP-2 or at 1M rows; the query-cost gates of the build
plan need that run.

## What changed

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
4. **The fit in Rust** (`vexact fit`), fed by `tools/cluster.py` (faiss k-means, `--cells` for two levels, every row
   assigned over all centres). Stored centres are the exact means of the assigned rows. Every pass over the vectors
   goes cluster by cluster. The dual basis's `lam` comes from the group covariance, `diag(V8⁻¹ C V8⁻ᵀ)`, without a second
   pass. The certificate is still `tools/certify.py`, run as a subprocess.

## Build time, one thread, same fit

Two runs of each; seconds from the start of the build.

| | Randomised constants | Plans done | Calibrated | Encoded |
|---|---|---|---|---|
| Before | 8.0, 8.3 | 10.2, 10.4 | 17.5, 17.7 | 29.8, 30.2 |
| After | 8.1, 8.3 | 9.8, 9.7 | 12.1, 12.0 | 17.3, 17.3 |

Calibration and encoding together fall from 19.6 to 7.5 s (2.6×). The randomised constants, 8 s of what remains, are
not recomputed when a saved index is opened.

## The saved index

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

## The Rust fit against `tools/fit.py`

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
range, and five global refinement steps on the sample do not remove it. Flat clustering stays the default; two levels
are opt-in until the 1M comparison decides.

## Not done, and not measured

- The 1M BioCLIP-2 comparison of every row above, and its query times.
- The replacement for the K×K support table (step 2 of the plan), int8 q·m, and per-cluster records mapped rather
  than read. The saved index reads the per-cluster data onto the heap, 18 KB per cluster.
- The fit still writes `Xs.npy` in cluster order and the build writes the vectors again in key order. The partitioned
  build (step 4) rewrites each cell in cluster order and is the place to write the final order once.
- The certificate table that would take Python out of the fit.

## Commands

```bash
python3 -I tools/synth_vectors.py DATA
python3 -I tools/cluster.py DATA/x.npy DATA/cl1                 # --cells 4 [--refine 5] for two levels
$CARGO_TARGET_DIR/release/vexact fit DATA/x.npy DATA/rfit --clustering DATA/cl1 --basis dual --cert-time 60
python3 -I tools/fit.py DATA/x.npy DATA/pyfit --basis dual --cert-time 60
$CARGO_TARGET_DIR/release/vexact --v2 --index DATA/rfit --queries DATA/q.npy --item-queries DATA/heldout.npy bytesq verify
$CARGO_TARGET_DIR/release/vexact --v2 --index DATA/rfit --queries DATA/q.npy --plan-default [--resident] --tag _X e2e --nq 50 --nflat 5
```

`--rebuild` builds the index again in place of opening the saved one, and `--hash` logs the code hash.
