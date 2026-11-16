# Native prototype of the exact, filter-aware vector index

A standalone Rust experiment (not product code). It encodes a set of vectors (first measured on the 1M BioCLIP-2 set,
n = 768) in the fast-scan code, answers threshold and k-NN queries exactly or approximately within each viewer's visible
set, checks results against brute force, and times them against flat scans. SYSTEM.md at the branch root describes the
index; the REPORT files here and under bench/ hold the measurements.

## Build

Stable Rust (1.99), release only, `-C target-cpu=native` (set in `.cargo/config.toml`). The index runs on AVX2 (with FMA
and F16C) or AVX-512: `src/simd.rs` gives its 16-lane operations as two 256-bit registers when the target lacks AVX-512,
and `src/v2/kern.rs`, `src/v2/q.rs` and `src/kernel.rs` have AVX2 versions of the kernels that need byte permutes, VNNI
or horizontal sums. Set `CARGO_TARGET_DIR` to a directory outside the tree.

```bash
export CARGO_PROFILE_RELEASE_DEBUG=0
cargo build --release
cargo test --release      # includes the end-to-end test on testdata/synth4k
```

## Inputs

`tools/fit.py` fits the build parameters to any file of vectors, from the data alone (no example queries, no text
anchors), and writes an index directory:

```bash
python3 -I tools/fit.py VECTORS.npy INDEX [--metric angular] [--per-cluster M]   # N x n floats; row i is entity i
```

It normalises (angular), zero-pads to a multiple of 32 dimensions, clusters with about 1,000 items per cluster (8-bit
centres), groups the clusters (at most 64 groups, 8 clusters and 20 n items per group on average) with a PCA basis of the
residuals per group, and writes the data-only `H25` profile (`profile.npy`), the level schedule and the randomised
blocks (`meta.json`), and the f16 vectors in cluster order (`Xs.npy`). The deterministic levels end at 4 / 8 / 12 / 16 /
24 / 32 B; the randomised ones at 56, 96, 160 and 256 B, extended by steps of 1.6x for n above 768 and capped at 40% of the f16
vector and 8 bits per dimension; the four randomised blocks split n at 1/8, 7/24 and 5/8 (multiples of 16). Every
other build parameter (bit allocation, certificate weights, whitening, block membership and rates, scalar grids) is
derived from these in Rust when the index is built (`src/derive.rs`, `src/v2/mod.rs`), and every item is encoded then
(about 11 s at 1.2M items on 12 cores). The encoded index is saved beside the fit, in `INDEX/encoded-KEY/`
(`src/v2/store.rs`; KEY covers the fit, the profile, the identity of the vectors file (name, length, modification and
status-change times, device and inode, not its contents), every option that changes the build and a fingerprint of the
library's sources, so any library edit makes saved indexes stale; it does not depend on how the directory's path is
spelled), with the f16 vectors in index row order; opening it maps the codes, keys and vectors and encodes nothing.
Other `encoded-*` directories there are logged as stale with their sizes, never removed. A rebuild holds
`INDEX/encoded.lock` (a second one waits), writes and syncs a scratch directory of its own and swaps it into place.
`testdata/make_synth.py` fits the small index the tests use.

## Run

Queries come from `--queries` (npy, original dimension); `verify` also uses 200 corpus items as item queries.

```bash
B="$CARGO_TARGET_DIR/release/vexact --v2 --index INDEX --queries QUERIES.npy"   # results go to INDEX/results (--results)
$B verify levels setup e2e filter                 # correctness, ns per item per level, setup cost, end to end, filtered
$B bytesq                                         # bytes read per item only
$B dbcmp --thr THRESHOLDS.npy                     # threshold queries against flat scans (see run2::dbcmp)
$B dbtopk --order ORDER.npy                       # exact and approximate top-k (see run2::dbtopk)
$B --scale 10 --nq 20                             # 10M synthetic items in the int8 bases
$B --clusters 40 verify                           # quick development check
```

The runs open the index saved for the fit and the build options given (`v2::open`), and stop with a message when there
is none: `--rebuild` encodes it and saves it first (`v2::rebuild`), `--in-memory` encodes it without saving it
(`v2::build`), and `--hash` logs the hash of the codes, scalars, keys, row order and escapes (`Index2::code_hash`) and
the hash of everything the index holds, vectors included (`Index2::store_hash`, also logged by every rebuild and recorded
in `index.json`).

The defaults are the measured configuration: integer deterministic levels, tables combined from the group's and the
centre's, and per-level repack thresholds (`--int`, `--gtab` and `--flags repack` restate them;
`--flags noint,nogtab,norepack` gives the float path). The binary refuses an argument it does not know.

`--plan auto|index|flat` sets the planner, for threshold queries and exact k-NN alike. Its cost model (`Plan`,
`src/v2/q.rs`) is re-timed when the index opens, on one thread, from 16 stored rows used as queries (`Plan::calibrate`,
about 1 s; `--calib N` rows, `--plan-default` keeps the BioCLIP constants of `Plan::default`). k-NN goes straight to a
scan of the visible set when that costs less than cutting the clusters that hold it plus the seed; otherwise after the
seed it cuts a sample of the clusters and scans if the sample says nothing will prune, and only then cuts the rest. Full
vectors are held in index row order, so a scan reads memory in order. Each run rebuilds the index (about 100 s at 1M on
4 cores) and appends to `results/run2.log`; tables go to `results/*.csv`.

## Layout

- `src/v2/mod.rs` — code fitting, plans, int8 bases, the encoder, the stored layout, per-session compaction.
- `src/v2/q.rs` — query setup, the breadth-first level loop, the planner, threshold queries, scans, exact and
  approximate k-NN, synthetic items.
- `src/v2/kern.rs` — tables, survivor re-packing, the shuffle kernel; `src/v2/tail.rs` — the certified constants;
  `src/v2/disk.rs` — full vectors on file under a memory cap; `src/v2/setup.rs` — the exact basis product.
- `src/v2/e2e.rs` — the end-to-end test.
- `src/index.rs` — the fitted parameters and the mapped vector file; `src/derive.rs` — the profile and rate design;
  `src/query.rs` — brute force over the stored vectors; `src/kernel.rs` — statistics and the f16 dot products;
  `src/util.rs` — .npy reader, exact similarity on f16 integers, reproducible Haar rotations; `src/simd.rs` — the AVX2
  forms of 16-lane operations.
- `src/run2.rs`, `src/ann.rs`, `src/scale.rs`, `src/main.rs` — the experiments and the command line.
- `tools/fit.py` — fits the build parameters to a file of vectors.
