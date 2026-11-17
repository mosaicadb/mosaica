# The benchmark suite

One entry point for every comparison of the index (`rust/`, through the bindings in `bench/annb/vexact-py`) against
other systems, on public datasets, in two sizes: **quick** (an afternoon on one machine) and **full**.

There are three query types, and filtering is an axis across all three:

- **approximate k-NN**: unfiltered on the public suites, VIBE (`bench/vibe`) and ANN-Benchmarks (`bench/annb`),
  unchanged, so those figures are the suites' own; under a visible set on this runner (the public suites have no
  filters), as recall against time at a few settings;
- **exact k-NN**: this runner;
- **threshold queries** (every item with q.x >= t; t is set so that a given share of the corpus, or of the visible
  set, answers): this runner.

The filter axis is `none` plus each suite's visible sets: `random-p` (a uniform random p% of the ids) and, in the full
suite, `region-p` (a correlated set: the rows whose nearest k-means centre, of 300, falls in a random subset of the
centres, whole centres taken until the next would overshoot, then filled to p% from that centre's rows).

For each type the suite names the standard system and the frontier one, beside ours:

| type | ours | standard | frontier | database (full only) |
|---|---|---|---|---|
| exact k-NN | vexact (`knn`) | faiss `IndexFlatIP` | PDX-BOND; faiss Panorama (`IndexFlatIPPanorama`) | pgvector |
| threshold | vexact (`threshold`) | faiss `IndexFlatIP.range_search` | faiss Panorama | pgvector |
| approximate k-NN, filtered | vexact (`knn_approx`) | faiss HNSW + `IDSelectorBitmap` | ACORN-γ | |
| approximate k-NN, unfiltered | VIBE / ANN-Benchmarks: vexact-approx, hnswlib, faiss IVF-PQ fast scan, glass (quick); the suites' default lists (full) |||

## Quick and full

| | quick | full |
|---|---|---|
| datasets | glove-100-angular (ANN-Benchmarks), yahoo-minilm-384-normalized, laion-clip-512-normalized (out of distribution), inaturalist-resnet-2048-cosine (2,048 dimensions), bioclip2-768 (BioCLIP-2, images and text prompts) | VIBE's 9 cosine and normalized sets (bench/vibe/README.md), glove-100-angular, nytimes-256-angular |
| queries | 100 per dataset (BioCLIP: its 150) | 100 |
| exact k | 10, 100 | 1, 10, 100, 1000 |
| threshold shares | 0.01%, 1%, 10% | 0.01%, 0.1%, 1%, 10%; also 30%, 50%, 75% on yahoo and laion |
| filters | none, random-1, random-25 | none, random-1, -10, -25, -50, region-1, region-25 |
| under a filter | exact k = 10; threshold 1% of the visible set; approximate k = 10 | exact k = 10, 100; threshold 0.1% and 1%; approximate k = 10 |
| approximate grids | short (an overlay of the public suites' configs, `suites.py`) | the public suites' own |
| pgvector | no | on yahoo and laion |

Both are plain dicts in `suites.py`; `params` there holds each system's build and query parameters.

## Layout

- `run.py`: the entry point; stages `prep`, `run`, `check`, `report` (default) and `public`.
- `suites.py`: the suites, filters and system parameters. `datasets.py`: the datasets and their sources.
- `prep.py`: per dataset, under `DATA/prep/<name>/`: the corpus normalised in f64 and stored as f32 (`corpus32.npy`,
  row i is entity i) and f16 (`corpus16.npy`, unless `--no-f16`; no system here reads it), the query subset
  (`queries.npy`, f64), on out-of-distribution sets up to 100,000 rows of the learn sample (for the index's dual fit),
  the visible sets, and the truth: entity ids by f64 similarity to the depth the suite needs (`order*.npy`), and per
  query the thresholds at each share and the k-NN boundaries, each the midpoint between the k-th and (k+1)-th f64
  similarity (`thresholds*.npy`, `knn_thr*.npy`), so every exact answer has exactly k items. Similarities are f64
  products of the f64 queries with the stored f32 vectors.
- `runsys.py`: the run stage for one dataset and one system, in its own process (started by `run.py` under `taskset`
  on the build cores). Rows go to `results/<suite>/<dataset>/<system>.csv` (one per query and cell),
  `builds.csv` and `<system>_setup.csv` (the time to set each visible set); answers are dumped under
  `DATA/dumps/` for the check.
- `check.py`: every exact answer against the truth, with the rounding allowance of `../threshold-db/check.py`;
  writes `results/<suite>/<dataset>/check.csv`.
- `report.py`: `results/<suite>/REPORT.md` (a table per type across datasets, the filtered approximate runs, the
  builds) and a plot per type (`exact.png`, `threshold.png`, `approx.png`).
- `public.py`: the public suites' driver (below).
- `systems/`: one adapter per system (`base.py` has the interface). `install/`: builds of PDX-BOND and ACORN.

## Running it

```bash
# once: the bindings (see bench/annb/README.md), faiss, and optionally PDX-BOND and ACORN
cd bench/annb/vexact-py && RUSTFLAGS="-C target-cpu=native" maturin build --release -o /tmp/wheels && pip install /tmp/wheels/vexact_py-*.whl
pip install faiss-cpu h5py matplotlib pyyaml        # faiss 1.13 or later has Panorama; 1.16.0 was used
bash bench/suite/install/pdx.sh ~/src/pdx           # pdxearch with BOND
bash bench/suite/install/acorn.sh ~/src/acorn       # then export ACORN_BENCH=~/src/acorn/acorn_bench

export SUITE_DATA=~/suite-data                      # outside the tree; HDF5 files are downloaded here
python3 -I bench/suite/run.py quick                 # prep, run, check, report on every quick dataset
python3 -I bench/suite/run.py quick --datasets yahoo-minilm-384-normalized --systems vexact,faiss-flat \
    --types exact,threshold --filters none,random-1
python3 -I bench/suite/run.py quick --stages public --vibe ~/vibe --annb ~/ann-benchmarks
```

Options: `--core` (the timing core, default 3) and `--build-cores` (default 0-2); `--hdf5-dirs` (more places to look
for `<name>.hdf5`, such as VIBE's `data/`); `--drop-hdf5` and `--drop-corpus` (keep one large dataset on disk at a
time); `--step N` (time every N-th query only); `--reps`; `--keep-work` (keep the indexes; by default each system's
work directory is removed after its run). BioCLIP needs `--bioclip-raw` (the output of `../threshold-db/fetch.py`)
and `--bioclip-text` (a directory with the BioCLIP-2 text-prompt embeddings `text_q.npy` and `text_insample.npy`, and
optionally `text_train.npy` as the learn sample), since its text queries are not published anywhere.

## The protocol

As `../threshold-db`: queries run one at a time on one thread pinned to one core (`taskset`, the libraries' own thread
settings at 1, and the index's one-thread pool pinned with `core=`), warm (the data in memory), each (query, parameter)
5 times; a query's figure is the median of its 5, and a cell's the median over queries, with the p90 over the same
per-query medians. Builds may use several threads (`--build-cores`) and are recorded apart. Times are taken in Python
around each call, from the query vector to the ids as numpy; ACORN, served by a C++ process over a pipe, reports the
search call's own time, as in `../threshold-db/ann.py`.

Exactness: `check.py` compares every exact answer with the f64 truth. An item that differs is within the rounding
allowance when |s - t| is at most what rounding the stored vectors and the query can move its similarity (f16 systems,
i.e. vexact: 2^-11 on each; f32 systems: 2^-24 on each plus an f32 accumulation bound of n 2^-24 sum_i |q_i x_i|). The
approximate runs report recall against the same truth (the k nearest visible items).

## The public suites (`--stages public`)

`public.py` runs `bench/vibe/run.sh` and `bench/annb/run.sh` on the suite's datasets with the suite's `ALGOS`. For the
quick suite it shortens their parameter grids: each named algorithm's `run_groups` in the checkout's `config.yml` are
replaced by the suite's for the length of the run (values picked from the public grids), and the file is restored
afterwards (`python3 -I public.py quick vibe VIBE_DIR` prints the overlay without running). The suites' code is not
changed and their results stay in the checkouts; plot or export them with the suites' own tools.

## Adding a system

Write a class in `systems/` with `name`, `types` (any of `exact`, `threshold`, `approx`), `rounding` (`f16` or `f32`)
and `build(threads)`, then whichever of `knn(q, k)`, `range(q, t)`, `approx(q, k, param)` its types need, and
`set_visible(ids, name)` if it filters (`settings()` lists variants, such as faiss's id selector against a compacted
copy). Raise `Unavailable` from `build` when a library is missing: the run records it in `builds.csv` and goes on.
Register it in `systems/__init__.py`, add its parameters to `SYSTEM_PARAMS` in `suites.py`, and name it under a role of
the types it serves in each suite's `systems`.

## Adding a dataset

Add it to `datasets.py` (source `vibe` or `annb` take the public HDF5 files; another source needs a reader in
`prep.py`, as `bioclip` has) and to a suite's `datasets`. Every dataset is treated as cosine.

## Systems: versions and forms

- **vexact**: the working tree's `rust/` through `bench/annb/vexact-py` (built with `-C target-cpu=native`), built as
  `bench/annb/vexact/module.py` does: `rust/tools/fit.py --basis dual` (with `--query-sample` from the learn sample on
  out-of-distribution sets), then `vexact_py.Index` (planner `auto`). Exact k-NN is `knn`, the threshold query the
  bindings' `threshold` (`threshold2` through the planner, entity ids), approximate `knn_approx` (nprobe 16, 64, 256 in
  the quick suite), all under `set_visible`.
- **faiss-flat**: pip `faiss-cpu` 1.16.0. `search` and `range_search` on `IndexFlatIP`; under a visible set through
  `IDSelectorBitmap` (`idselector`) and over a compacted `IndexFlatIP` of the visible rows (`compacted`, built when the
  set is set).
- **faiss-panorama**: `IndexFlatIPPanorama`, 8 levels, after an orthogonal PCA rotation of the data and of each query
  (inside the timed call), as `../threshold-db/ann.py`. Under a visible set through `IDSelectorBitmap`.
- **pdx-bond**: `pdxearch` from github.com/cwida/PDX at 91618e0 (the last revision with BOND), `IndexBONDFlat`: L2 over
  the unit vectors, the PDX layout in blocks of 10,240, written by the adapter and mapped (`restore`). k-NN only (PDX
  has no range search); under a visible set `filtered_search` with a selection vector.
- **faiss-hnsw**: `IndexHNSWFlat`, M 16, efConstruction 200, inner product; efSearch 64, 256, 1024 with
  `IDSelectorBitmap`.
- **acorn**: ACORN-γ (stanford-futuredata/ACORN at c259f11 with `../threshold-db/acorn.patch`, the static
  `faiss_avx2` library) through `../threshold-db/acorn_bench.cpp`: M 32, γ 12, M_β 32, L2 over the unit vectors;
  efSearch 64, 256, 1024. The build is one piece on the build cores.
- **pgvector**: PostgreSQL with pgvector, sequential scans, one worker; `ORDER BY v <#> q LIMIT k` and
  `WHERE (v <#> q) <= -t`, a visible set as `id IN (SELECT id FROM vis_...)`. Timed at the client as
  `../threshold-db/dbrun.py`. Needs `psycopg` and `PGVECTOR_DSN`; pin the server to the timing core yourself.

## Status

The quick suite's exact, threshold and filtered cells were run on a 4-core cloud VM (Xeon at 2.1 GHz with AVX-512,
15 GB, timing on core 3, builds on cores 0-2) on yahoo-minilm-384-normalized, laion-clip-512-normalized,
inaturalist-resnet-2048-cosine and glove-100-angular: `results/quick/REPORT.md`. Not run there:

- **ACORN** ran on yahoo only: its build took 1,247 s on 3 cores for 677k vectors of 384 dimensions, and the other
  datasets would each have taken an hour or more. Run it with `--keep-work` (a kept index is reused) on a larger
  machine.
- **bioclip2-768**: the source is implemented, but its text queries are BioCLIP-2 text-prompt embeddings that are not
  published; produce `text_q.npy` and `text_insample.npy` with the BioCLIP-2 text tower (as `agent2/encode_text.py`
  does) and fetch the images with `../threshold-db/fetch.py`.
- **pgvector** (full suite only): the adapter was checked against brute force on a 20,000-row table (PostgreSQL 16,
  pgvector 0.6.0 from Ubuntu's packages), not run on a dataset.
- **The public suites** (`--stages public`): the overlay was checked (applied and restored on VIBE and ANN-Benchmarks
  checkouts), but the images for faiss and glass were not built here, and ANN-Benchmarks' Docker images were absent,
  so the unfiltered approximate k-NN was left for a machine with the images.
