# Native prototype of the exact, filter-aware vector index

A standalone Rust experiment (not product code). It encodes a set of vectors (first measured on the 1M BioCLIP-2 set, n = 768) with the fixed-rate nested
tree code of `vec/quant/REPORT.md`, answers threshold and exact k-NN queries with per-viewer visibility, checks every
result against brute force, and times everything against flat f16 and int8 scans. `REPORT.md` has the results.

## Build

Stable Rust (1.99), release only, `-C target-cpu=native` (set in `.cargo/config.toml`). The fast-scan index (`--v2`)
runs on AVX2 (with FMA and F16C) or AVX-512: `src/simd.rs` gives its 16-lane operations as two 256-bit registers when the
target lacks AVX-512, and `src/v2/kern.rs`, `src/v2/q.rs` and `src/kernel.rs` have AVX2 versions of the kernels that
need byte permutes, VNNI or horizontal sums. Version 1 still needs AVX-512 at run time. Set `CARGO_TARGET_DIR` to a
directory outside the tree.

```bash
export CARGO_PROFILE_RELEASE_DEBUG=0
cargo build --release
```

## Inputs

`tools/fit.py` fits the build parameters to any file of vectors, from the data alone (no example queries, no text
anchors), and writes an index directory:

```bash
python3 -I tools/fit.py VECTORS.npy INDEX [--metric angular]   # N x n floats; row i is entity i
```

It normalises (angular), zero-pads to a multiple of 32 dimensions, clusters with about 1,000 items per cluster (8-bit
centres), groups the clusters (at most 64 groups, 8 clusters and 20 n items per group on average) with a PCA basis of the
residuals per group, and writes the data-only `H25` profile (`profile.npy`), the level schedule and the randomised
blocks (`meta.json`), and the f16 vectors in cluster order (`Xs.npy`). The deterministic levels end at 4 / 8 / 12 / 16 /
24 / 32 B; the randomised ones at 56, 96, 160 and 256 B, extended by steps of 1.6x for n above 768 and capped at 40% of the f16
vector and 8 bits per dimension; the four randomised blocks split n at 1/8, 7/24 and 5/8 (multiples of 16). Every
other build parameter (bit allocation, certificate weights, whitening, block membership and rates, scalar grids) is
derived from these in Rust, and every item is encoded, by the first run on a fit (about 11 s at 1.2M items on 12
cores).

Version 1 reads the same directory and needs `--profile INDEX/profile.npy` to derive its allocation.

## Run

```bash
B="$CARGO_TARGET_DIR/release/vexact --index INDEX --queries QUERIES.npy"   # results go to INDEX/results (--results)
$B bandwidth levels verify            # memory bandwidth; ns per item per level; correctness on all 569 queries
$B e2e batch --nq 100 --nflat 20      # end-to-end threshold and k-NN vs flat scans, 1 and 4 threads; batching
$B disk filter --dnq 20 --fnq 30      # pages read per residency cut (O_DIRECT); filtered queries
VEXACT_CAL_PER_GROUP=1 $B --scale 10 --nq 20 --anchors 1000   # synthetic 10M items from the fitted model
$B --clusters 40 verify levels        # quick development check on a 40-cluster subset
```

The first run on a fit builds the version 2 index (about 100 s at 1M on 4 cores) and saves it to
`INDEX/encoded-KEY/`, where KEY covers the fit, the profile and every option that changes what is built; later runs
with the same key map it instead (`--rebuild` builds again, `--resident` reads it onto the heap instead of mapping it,
`--hash` logs a hash of everything encoded). Runs append to `results/*.log`; tables go to
`results/*.csv`. Large files (the disk experiment's level and full-vector files) are written under the scratchpad, not
here.

## Layout

- `src/util.rs` — .npy reader, exact similarity on f16 integers, reproducible Haar rotations (ChaCha20 + IEEE-only
  arithmetic, Householder QR with the sign fix).
- `src/index.rs` — parameters, per-group code plans (ops, plane numbering, index-state slots), the encoder, the
  stored layout (bit-planes over blocks of 16 items, one array per level per cluster), the support table.
- `src/kernel.rs` — AVX-512 block decoder (single and batched queries), flat f16/int8 kernels, basis products.
- `src/query.rs` — per-query and per-cluster constants and slacks, cluster cut and sorted range, threshold query,
  batched threshold, exact k-NN, visibility, flat scans, brute-force truth.
- `src/synth.rs`, `src/scale.rs` — synthetic items for the scale test.
- `tools/fit.py` — fits the build parameters to a file of vectors.

## Version 2: the fast-scan code (`--v2`)

Every query-profile-dependent parameter is derived in Rust (`src/derive.rs`, `src/v2/mod.rs`) from the profile file
(G x n query-offset variances in each group's local basis; default `INDEX/profile.npy`, the data-only `H25`).
Queries come from `--queries` (npy, original dimension); `verify` also uses 200 corpus items as item queries.

```bash
B="$CARGO_TARGET_DIR/release/vexact --v2 --index INDEX --queries QUERIES.npy"
$B verify levels setup e2e filter                 # the fast-scan index
$B --u8 ...                                       # 8-bit tables (one shuffle) instead of 16-bit
$B bytesq                                         # bytes read per item only
$B --scale 10 --nq 20                             # 10M synthetic items in the int8 bases
$B --clusters 40 verify                           # quick development check
```

`--plan auto|index|flat` sets the planner, for threshold queries and exact k-NN alike. Its cost model (`Plan`, `src/v2/q.rs`) is re-timed when the index opens, on one thread, from 16 stored rows used as queries (`Plan::calibrate`, about 1 s; `--calib N` rows, `--plan-default` keeps the BioCLIP constants of `Plan::default`). k-NN goes straight to a scan of the visible set when that costs less than cutting the clusters that hold it plus the seed; otherwise after the seed it cuts a sample of the clusters and scans if the sample says nothing will prune, and only then cuts the rest. Full vectors are held in index row order, so a scan reads memory in order. Layout: `src/v2/mod.rs` (code fitting, plans, int8
bases, encoder, layout), `src/v2/kern.rs` (tables, survivor re-packing, shuffle kernel), `src/v2/q.rs` (setup,
breadth-first level loop, planner, k-NN, synthetic items), `src/run2.rs` (experiments). Results go to
`results/*2*.csv` and `results/run2.log`.
