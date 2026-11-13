# Native prototype of the exact, filter-aware vector index

A standalone Rust experiment (not product code). It encodes the 1M BioCLIP-2 set (n = 768) with the fixed-rate nested
tree code of `vec/quant/REPORT.md`, answers threshold and exact k-NN queries with per-viewer visibility, checks every
result against brute force, and times everything against flat f16 and int8 scans. `REPORT.md` has the results.

## Build

Stable Rust (1.97), release only, `-C target-cpu=native` (set in `.cargo/config.toml`; AVX-512 F/BW/DQ/VL required at
run time). The target directory is outside the tree (`../../rust-target`).

```bash
export CARGO_PROFILE_RELEASE_DEBUG=0
cargo build --release
```

## Inputs

The learned build parameters come from the numpy pipeline and are exported once:

```bash
python3 tools/export.py m768 /tmp/.../scratchpad/rustdata/m768   # clusters, 8-bit centres, bases, bit allocations,
                                                                  # randomised schedule and rates, tree tables, queries
```

The stored vectors are read from `vec/combined/idx_m768/Xs.npy` (f16, memory-mapped). Everything per item (sort key,
codes, norms, randomised scalars) is encoded in Rust (`src/index.rs`). Paths are constants at the top of `src/main.rs`
and `src/scale.rs`.

## Run

```bash
B=../../rust-target/release/vexact
$B bandwidth levels verify            # memory bandwidth; ns per item per level; correctness on all 569 queries
$B e2e batch --nq 100 --nflat 20      # end-to-end threshold and k-NN vs flat scans, 1 and 4 threads; batching
$B disk filter --dnq 20 --fnq 30      # pages read per residency cut (O_DIRECT); filtered queries
VEXACT_CAL_PER_GROUP=1 $B --scale 10 --nq 20 --anchors 1000   # synthetic 10M items from the fitted model
$B --clusters 40 verify levels        # quick development check on a 40-cluster subset
```

Each run rebuilds the index (about 100 s at 1M on 4 cores) and appends to `results/*.log`; tables go to
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
- `tools/export.py` — exports the numpy build parameters.

## Version 2: the fast-scan code (`--v2`)

Every query-profile-dependent parameter is derived in Rust (`src/derive.rs`) from a profile file (G x n query-offset
variances in each group's local basis). Export one with `tools/profile.py` (`qmix` text-tuned, `qi` items, `h0.25` the
data-only H25 profile, or a key of a `.npz`):

```bash
python3 tools/profile.py m768 h0.25 /tmp/.../scratchpad/rustdata/m768/prof_h0.25.npy
B=../../rust-target/release/vexact
$B --profile .../prof_h0.25.npy levels e2e        # the old tree kernel built with any profile
$B --v2 verify levels setup e2e filter            # the fast-scan index (profile defaults to prof_h0.25.npy)
$B --v2 --u8 ...                                  # 8-bit tables (one shuffle) instead of 16-bit
$B --v2 bytesq; $B --profile ... bytesq           # bytes read per item only, both codes
$B --v2 --scale 10 --nq 20                        # 10M synthetic items in the int8 bases
$B --v2 --clusters 40 verify                      # quick development check
```

`--plan auto|index|flat` sets the planner (constants of its cost model in `Plan::default`, `src/v2/q.rs`). Layout: `src/v2/mod.rs` (code fitting, plans, int8
bases, encoder, layout), `src/v2/kern.rs` (tables, survivor re-packing, shuffle kernel), `src/v2/q.rs` (setup,
breadth-first level loop, planner, k-NN, synthetic items), `src/run2.rs` (experiments). Results go to
`results/*2*.csv` and `results/run2.log`.
