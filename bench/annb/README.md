# The index in ANN-Benchmarks

This folder adds the fast-scan index (`rust/`) to [ANN-Benchmarks](https://github.com/erikbern/ann-benchmarks) as the
algorithm `vexact`. It then runs the index against the harness's own builds of hnswlib, faiss, ScaNN, glass, PyNNDescent,
DiskANN and BLAS brute force on the harness's datasets. Filtered, exact and threshold queries are in `bench/suite`.
This folder covers only the unfiltered protocol, run by the harness itself.

- `vexact-py/`: Python bindings (pyo3) over the `vexact` library: `Index(dir)`, `knn`, `knn_approx`, `threshold`,
  `set_visible`, `fit_dial`. They depend on `rust/` by path and need no change to it. `threshold` and the disk mode
  (`full_file` and its read options, `evict_full`, `full_resident`; used by `bench/suite`'s memory-cap axis) are not
  part of the harnesses' protocol.
- `vexact/`: the algorithm folder the harness loads: `module.py`, `config.yml`, `Dockerfile`.
- `install.sh`: copies the algorithm folder into a checkout, with `rust/` and `combined/core.py` taken from a pinned
  revision, and builds the `ann-benchmarks-vexact` image.
- `run.sh`: every algorithm on every dataset, then a plot per dataset and `results/export.csv`.
- `diskann.patch`: the harness's DiskANN image does not build as published; this patch is applied to the checkout.

## Running it

```bash
git clone https://github.com/erikbern/ann-benchmarks.git ~/ann-benchmarks
cd ~/ann-benchmarks && git checkout 2e081ad    # the revision these files were checked against
python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
git apply /path/to/mosaica/bench/annb/diskann.patch
for a in hnswlib faiss faiss_hnsw scann glass pynndescent bruteforce diskann; do .venv/bin/python install.py --algorithm $a; done
bash /path/to/mosaica/bench/annb/install.sh ~/ann-benchmarks
PYTHON=~/ann-benchmarks/.venv/bin/python bash /path/to/mosaica/bench/annb/run.sh ~/ann-benchmarks
```

`run.sh` takes dataset names after the checkout to run fewer, and reads `ALGOS`, `RUNS` (default 2) and `TIMEOUT`
(seconds per algorithm instance, default 4 hours) from the environment. Results already present are kept.

The default datasets are glove-25, glove-100, nytimes-256, coco-t2i-512 (CLIP text queries against image embeddings)
and dbpedia-openai-1000k (1M OpenAI embeddings, 1,536 dimensions). All are angular: the index supports no other metric
yet. coco and dbpedia are not hosted at ann-benchmarks.com, and the checkout builds them on first use. dbpedia needs
about 6 GB of download and 20 GB of memory to build, and the brute-force ground truth takes a while.

## The pinned revision

`install.sh` copies `rust/` from the revision named `PIN` in the script, not from the working tree, so the
benchmarked index stays fixed while `rust/` changes. `install.sh DIR REV` builds another revision. To move the pin, set
`PIN`, rebuild, and fix `vexact-py/src/lib.rs` if the library's API has moved. The bindings follow the library's
`create` / `open` / `rebuild` API (`v2::open` with `OpenOptions`, `v2::saved_options`, `v2::rebuild` with
`BuildOptions`). They use only those, `Plan` (`Plan::default`, `Plan::check`), `pseudo_queries`, `Approx::fit`,
`Dial::fit`, `Vis2::new`, `knn2` and `knn2_fast`, for `threshold` `prep2`, `threshold2`, `Opts2` and `to_bitmap2`, and
for the disk mode `Full2`, `Index2::own_rows`, `disk::Rows` (`to_file`, `policy`, `evict`, `resident`, `file_path`),
`disk::uring_available` and `q::disk_constants`. The pin is `research/exact-vector-search` with the
create/open/rebuild API, the AVX-512 kernels, and the owner's build defaults (clusters by N, deterministic levels 16
and 32, the dual basis); the bindings build the library's default levels unless given `dlv`.

The image compiles with `-C target-cpu=native`, so build it on the machine that will run the benchmark.

## How the index is run

- **Build** (`fit`): `rust/tools/fit.py --basis dual` on the training vectors (the basis SYSTEM.md measures and `fit.py`'s
  default; the build argument `basis: pca` selects pca), then `vexact_py.Index` on its output (encoding, the
  approximate mode's cluster order fitted on 1,000 pseudo-queries). The bindings open the index saved in the fit when it was built with the same
options, else encode the fit and save the index there (`v2::rebuild`; about 1.8 GB at 1M items, in the work directory, removed at exit), so the queries run on the index
mapped from its files, as `v2::open` gives it. The planner times nothing: in memory it is the support table and the rule
in rows (`Plan::default`). Without a `dlv` build argument the bindings build the library's default deterministic
  levels (two, at 16 and 32 B); `dlv: fit` builds the fit's six (4 / 8 / 12 / 16 / 24 / 32 B), the default of the runs
  before the library's defaults changed. One directory holds one index, so two builds with different `dlv` on one fit replace each other's saved
  index; each variant needs its own directory (`cp -al` of the fit).
  The harness pins the container to one CPU, so the build runs single-threaded as every other algorithm's does.
- **vexact-exact**: `knn2`, the exact k-NN through the planner. One configuration.
- **vexact-approx**: `knn2_fast` over the first `nprobe` clusters (1 to 512), with the dial off (`eps` 0) or fitted
  for a miss rate of 0.01 or 0.03 per level. The dials for k = 10 and k = 100 are fitted in `set_query_arguments`,
  outside the timed queries. At 64 dimensions (glove-25) the dial changes nothing, because reading a probed cluster's
  full vectors is cheaper than its codes. At 384 dimensions (VIBE's yahoo-minilm-384, k = 100) `eps` 0.03 is 1.46 times
  faster at nprobe 64 and 1.6 times at nprobe 128, for recall 0.001–0.002 lower.
- **vexact-approx-lc**: as vexact-approx on research/levels-clusters' candidate builds (REPORT_BUILD.md, "the
  levels-clusters defaults"): A, clusters by N (build argument `clusters: by-n`, `fit.py --per-cluster by-n --groups
  by-n`); B, two deterministic levels (`dlv: "16.32"`); C, both. Not in `run.sh`'s default list; add it with `ALGOS`.
- Each query is one call from Python, which adds a few microseconds to each query. This is small at the speeds measured
  here, but matters above about 10,000 QPS.

## Deviations from the harness

- Vectors narrower than 64 dimensions (glove-25) are padded with zero columns to 64, which leaves every cosine
  unchanged. The index's randomised levels need at least 50 dimensions.
