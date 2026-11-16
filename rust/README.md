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

## Making an index

An index is a directory. `create` makes one from a file of vectors, the whole pipeline in one step: it partitions the
vectors into `DIR/partition` (`tools/partition.py`, with python3, numpy and faiss), fits the build parameters into `DIR`
(`src/fit.rs`, with the support table's anchors), encodes every item and saves the index in `DIR/encoded`:

```bash
V="$CARGO_TARGET_DIR/release/vexact"
$V --vectors VECTORS.npy --index DIR create [build flags]      # N x n0 floats; row i is entity i; DIR new or empty
$V --v2 --index DIR MODE...                                    # opens the saved index (never encodes) and runs MODE
$V --v2 --index DIR --rebuild [build flags] [MODE...]          # re-encodes the fit in DIR and saves it, then runs MODE
```

In the library these are `v2::create(vectors, dir, &BuildOptions, log)`, `v2::open(dir, &OpenOptions, log)` and
`v2::rebuild(dir, &BuildOptions, log)`; `v2::build` encodes in memory and saves nothing (`--in-memory`).

The build flags are the fields of `v2::BuildOptions`. The partition and the fit, given to `create` only (a rebuild
re-encodes the fit as it is): `--per-cluster by-n|M` (items per cluster; default by-n, 2,000 up to 2M items, doubling
for each 4x N), `--groups by-n|by-k8|G` (default by-n), `--basis dual|pca` (default dual), `--query-sample QUERIES.npy`,
`--cert-time SECONDS`. The support table's anchors, to `create` or a
rebuild (which writes them into the fit first): `--anchors A` (from the centres; default 1,024), `--text-anchors
PROMPTS.npy`, `--text-count C`. The encoding: `--dlv B,B,...|fit` (the deterministic levels; default 16,32, two levels; `fit`: the fit's six,
4 / 8 / 12 / 16 / 24 / 32 B), `--rlv B,B,...`,
`--profile P.npy`, `--cal-items N`, `--nguar N`, `--nusort`, `--design-exact`, `--csg-plain`, `--clusters K` (the first K
clusters only), and `--seed S` for all of them. A build flag where nothing is built is refused, as is any argument the
binary does not know (there are no positional arguments).

`v2::OpenOptions` holds what an open may choose without changing what is stored: `--ddec LEVELS` (the deterministic
levels that decide threshold queries) and `--keep-index` (open although the fit's vectors file, perm.npy or partition
changed since the build).

The saved index records the build options it was built with, and its key covers the fit, the profile, those options,
the formats and a fingerprint of the library's sources (`src/v2/store.rs`, `build_key`; it does not depend on how the
directory's path is spelled). An open reads the fit's parameters and the index's per-cluster records, maps the codes,
keys and vectors, and encodes nothing; a missing index, or a stale one (the fit changed, or any library edit), is an
error that says to rebuild. One directory holds one index: to compare build options on one fit, hard-link the fit into
a second directory (`cp -al DIR DIR2`) and rebuild that one. Other `encoded*` directories there (an earlier version's
`encoded-KEY`, an interrupted rebuild's scratch directory) are logged with their sizes, never removed. A rebuild holds
`DIR/encoded.lock` (a second one waits), writes and syncs a scratch directory of its own and swaps it into place.

`tools/fit.py` also fits a file of vectors, from the data alone, into an index directory that `--rebuild` then encodes
(it keeps an f16 copy of the vectors in cluster order, `Xs.npy`):

```bash
python3 -I tools/fit.py VECTORS.npy INDEX [--metric angular] [--per-cluster M]   # N x n floats; row i is entity i
```

Both normalise (angular), zero-pad to a multiple of 32 dimensions, cluster (by default 2,000 items per cluster up to 2M
items, doubling for each 4x N; `--per-cluster M` for about M; 8-bit centres), group the clusters (by default at most 64
groups and 20 n items per group; `--groups by-k8` also at most 8 clusters per group on average) with a basis per group
(by default the dual pair of the query and residual covariances; `--basis pca`: the residuals' PCA basis), and write the data-only `H25` profile (`profile.npy`), the level schedule and the randomised
blocks (`meta.json`). The fit's deterministic levels end at 4 / 8 / 12 / 16 / 24 / 32 B, and the build's default
replaces them with two, at 16 and 32 B (`BuildOptions::dlv`, `--dlv`); the randomised ones at 56, 96, 160 and
256 B, extended by steps of 1.6x for n above 768 and capped at 40% of the f16 vector and 8 bits per dimension; the four
randomised blocks split n at 1/8, 7/24 and 5/8 (multiples of 16). Every other build parameter (bit allocation,
certificate weights, whitening, block membership and rates, scalar grids) is derived from these in Rust when the index
is built (`src/derive.rs`, `src/v2/mod.rs`). `testdata/make_synth.py` fits the small index the tests use.

## Run

Queries come from `--queries` (npy, original dimension); `verify` also uses 200 corpus items as item queries.

```bash
B="$CARGO_TARGET_DIR/release/vexact --v2 --index INDEX --queries QUERIES.npy"   # results go to INDEX/results (--results)
$B verify levels setup e2e filter                 # correctness, ns per item per level, setup cost, end to end, filtered
$B bytesq                                         # bytes read per item only
$B dbcmp --thr THRESHOLDS.npy                     # threshold queries against flat scans (see run2::dbcmp)
$B dbtopk --order ORDER.npy                       # exact and approximate top-k (see run2::dbtopk)
$B --scale 10 --nq 20                             # 10M synthetic items in the int8 bases
$B --in-memory --clusters 40 verify               # quick development check
```

The runs open the saved index (`v2::open`) and stop with a message when there is none or it is stale: `--rebuild`
encodes it and saves it first (`v2::rebuild`), `--in-memory` encodes it without saving it (`v2::build`), and `--hash`
logs the hash of the codes, scalars, keys, row order and escapes (`Index2::code_hash`) and the hash of everything the
index holds, vectors included (`Index2::store_hash`, also logged by every rebuild and recorded in `index.json`). `e2e`'s
further variants (`--index-b DIR`, ...) open their own saved indexes.

The defaults are the measured configuration: integer deterministic levels, tables combined from the group's and the
centre's, and per-level repack thresholds (`--int`, `--gtab` and `--flags repack` restate them;
`--flags noint,nogtab,norepack` gives the float path). The binary refuses an argument it does not know.

`--plan auto|index|flat|scan` sets the planner, for threshold queries and exact k-NN alike (`src/v2/q.rs`, `plan2`).
Nothing is timed when the index opens. In memory and unfiltered, the support table decides the clusters it can and every
other cluster runs on the index path. Under a visible set one constant in rows decides (`Plan::r_rows`, 300): the set is
scanned when it holds at most that many rows per cluster holding one; otherwise a cluster holding at most that many is
scored directly and the rest run on the index path; exact k-NN also counts its seed as `r_seed` rows (6,000). With the
full vectors on file and read through io_uring (`--full-file`), a cost model prices the reads, from disk constants timed
once per machine and device and cached in the index directory (`disk-constants.txt`; in the per-user cache,
`~/.cache/vexact`, when the index directory cannot be written); `--plan-file` gives those
constants (`r_rows`, `r_seed`, `io_us`, `io_gbps` as name=value lines) instead. Full vectors are held in index row
order, so a scan reads memory in order. A rebuild takes about 100 s at 1M on 4 cores. Each run appends to
`results/run2.log`; tables go to `results/*.csv`.

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
- `src/fit.rs`, `src/bucket.rs` — the fit of a partition, and the partition's cells read in the fit's row order.
- `tools/partition.py` — the partition `create` runs; `tools/fit.py` — fits the build parameters to a file of vectors.
