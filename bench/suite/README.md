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

- `run.py`: the entry point; stages `prep`, `run`, `check`, `report` (default), `public` and `cap`.
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
- `check.py`: every exact answer against the truth, with the rounding allowance of the earlier `threshold-db/check.py`;
  writes `results/<suite>/<dataset>/check.csv`.
- `report.py`: `results/<suite>/REPORT.md` (a table per type across datasets, the filtered approximate runs, the
  builds) and its figures (`plotting.py`, in the style of the earlier threshold-db and topk plots: exact methods solid,
  approximate methods dashed at their cheapest setting reaching a recall target, hollow where none did, names at the
  line ends): per dataset `<dataset>.png` (k-NN at k = 10 and threshold queries against the share of items visible,
  and threshold queries against the answer size); `recall_qps.png`, the ANN-Benchmarks view (per dataset and
  filter, recall@10 against queries per second, approximate methods through their Pareto frontier and exact methods
  as points); at scale `scale.png` (time against N per filter, approximate k-NN
  at recall 0.9 and 0.95) and `scale_build.png` (build time, index size and peak memory against N).
- `public.py`: the public suites' driver (below).
- `capmem.py`: the optional memory cap (cgroup v1 and v2) and its evidence; `prep_scale.py`: the LAION scale suites'
  prep; `machine.py`: the machine record a run keeps with its results.
- `systems/`: one adapter per system (`base.py` has the interface). `install/`: builds of PDX-BOND, ACORN and DiskANN;
  `native/`: DiskANN's driver and the header that builds it against OpenBLAS.

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
bash bench/suite/install/diskann.sh ~/src/diskann  # then export DISKANN_BENCH=~/src/diskann/diskann_bench
python3 -I bench/suite/run.py quick --stages prep,cap,report      # optional: the synthetic cap axis (see below)
python3 -I bench/suite/run.py scale-test                           # the scale path on 200k / 500k / 1M (2 GB download)
python3 -I bench/suite/run.py scale --download /big/disk/laion     # 10M / 40M / 100M (see the table below)
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
search call's own time, as in the earlier `threshold-db/ann.py`.

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
  out-of-distribution sets), then `vexact_py.Index` (planner `auto`) with the library's default build: clusters and
  groups by N, two deterministic levels at 16 and 32 bytes (the parameter `dlv`; `dlv=[]` builds the fit's six). Exact k-NN is `knn`, the threshold query the
  bindings' `threshold` (`threshold2` through the planner, entity ids), approximate `knn_approx` (nprobe 16, 64, 256 in
  the quick suite), all under `set_visible`.
- **faiss-flat**: pip `faiss-cpu` 1.16.0. `search` and `range_search` on `IndexFlatIP`; under a visible set through
  `IDSelectorBitmap` (`idselector`) and over a compacted `IndexFlatIP` of the visible rows (`compacted`, built when the
  set is set).
- **faiss-panorama**: `IndexFlatIPPanorama`, 8 levels, after an orthogonal PCA rotation of the data and of each query
  (inside the timed call), as the earlier `threshold-db/ann.py`. Under a visible set through `IDSelectorBitmap`.
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
  the earlier `threshold-db/dbrun.py`. Needs `psycopg` and `PGVECTOR_DSN`; pin the server to the timing core yourself.

## The memory-cap axis (`--stages cap`): optional and synthetic

How memory-constrained runs are meant to be done is the scale suite below: on whatever machine runs it, its RAM is the
cap. The cap axis is an optional synthetic test besides it, not part of any suite's default stages (run it with
`--stages cap`). Read its figures with care:

- It is meaningful only on a normal machine or VM whose storage is not cached by a hypervisor. A VM's host often caches
  its virtual disk (and WSL2's vhdx may be cached by Windows), so reads the cgroup forces out of the guest's page cache
  come back from the host's memory, not from the device. This sandbox's disk is such a disk: 4 KiB O_DIRECT reads of a
  recently written or read file took about 33 us at the median, of a large file nobody had read about 650-750 us.
- A system that reads with O_DIRECT (DiskANN) bypasses the page cache, so a cgroup cap does not constrain its reads at
  all: for it the result is the peak memory it needed (the evidence table), not a time under pressure.

A cap is a fraction of the dataset's size, defined as **N x d x 4 bytes** (its f32 vectors as distributed): 50%, 25%
and 10%, plus `uncapped` (the same code path with no limit). Every system on a dataset gets the same cap, for its whole
query process, page cache included, with no swap, so that only the page cache gives way. The quick suite runs the axis
on yahoo-minilm-384 and laion-clip-512, the full suite on every dataset and `scale-test` on laion-s200k when asked
(`suites.py`, `caps`). Under a cap each warm query's last repetition records its storage reads (the system's own
counters, the bytes its processes read, major faults), reported per cell with the peak memory.

**How it is applied** (`capmem.py`, `runsys.py --cap`). Each system builds once, uncapped, on the build cores, and keeps
its index in the work directory. Each cap then runs in a fresh process that first moves into a memory cgroup of its own
before it allocates anything: on cgroup v1 (as in a container whose memory controller is writable) a child of its own
group; on cgroup v2 the scope `run.py` starts it in, `systemd-run --user --scope -p MemoryMax=infinity
-p MemorySwapMax=0` (as the earlier `threshold-db/REPORT_CAPPED.md` did). It opens the kept index without rebuilding,
drops every file of the system from the page cache (fsync, then `posix_fadvise(DONTNEED)`; pages another group left in
the cache would otherwise serve it unseen) and reclaims its group's cache, then sets the limit: v1
`memory.limit_in_bytes` and `memory.memsw.limit_in_bytes` (memory plus swap), v2 `memory.max` and
`memory.swap.max = 0`. A server the system drives (DiskANN's) is started by that process, so it is inside the group.
After each cell the run logs the group's usage, anonymous and file memory, peak, limit hits (v1 `failcnt`, v2
`max` events), OOM kills, major faults and the bytes read from storage (`caps/<system>.cap-<cap>.mem.csv`). On this
container's kernel (6.18, cgroup v1) `failcnt` stayed at 0 although the groups ran at their limit; the evidence that a
cap held is the peak equal to the limit, the usage pinned there, and the storage reads (the mapped faiss scan, for
instance, read its whole file again on every query under every cap).

**Does not fit.** A limit the kernel refuses (v1 cannot shrink the group below what it cannot reclaim, EBUSY), and a
process the OOM killer stops under the cap, are recorded as `does not fit` in `caps/status.csv`; the cells a run
finished before an OOM kill are kept. An in-memory system named on the axis (faiss HNSW) is built inside each capped
process and the cap set after it, so a cap below what it holds is refused or ends in an OOM kill: `does not fit`.

**Cold and warm.** Each cell first times one cold query (query 0, once): vexact drops its full vectors (the codes stay),
the other systems their group's page cache (anonymous memory, such as faiss IVF's centroids and DiskANN's PQ codes,
stays). Then the warm repetitions run as everywhere (20 queries, 5 repetitions; the mapped faiss scan, which reads its
whole file per query once the file no longer fits the cache, 5 queries once).

**The systems under a cap** (`disk = True` in `systems/`):

- **vexact, disk mode**: the saved index opened with `v2::open` (its codes are mapped from their files, so they are
  page cache too, not anonymous memory) and the full vectors moved to a file of their own (`full_file`:
  `Rows::to_file`, a fresh file outside the saved index's `encoded-*` directory, which `to_file` refuses), read as the
  earlier capped runs read them: `MADV_RANDOM`, pread through io_uring with `willneed` ahead of each batch, O_DIRECT for
  extents from 8 KiB, and the planner pricing those reads (`disk_constants`, timed once per machine and cached in the
  index directory). The bindings' options mirror `rust/src/run2.rs`'s `--full-file`, `--full-advice`, `--full-read`,
  `--willneed`, `--direct-min` and `--dontcache-min`.
- **faiss-flat-mmap**: `IndexFlatIP` written to a file and opened with `IO_FLAG_MMAP_IFC` (the scan baseline).
- **faiss-ivf-disk**: IVF4096 (fewer lists below 1M vectors) with QT_fp16 codes and `OnDiskInvertedLists`, the earlier
  `threshold-db/capped/faiss_capped.py`; approximate, nprobe 16, 64, 256.
- **diskann**: DiskANN's SSD index (microsoft/DiskANN C++ 0.7.0 through `native/diskann_bench.cpp`; R 64, L 100,
  Linux AIO with O_DIRECT, no node cache), approximate and unfiltered (it takes no query-time visible set), search list
  20, 50, 100, 200, beam width 2. Its PQ codes are the part that must stay in memory; one index per dataset is built with
  a PQ budget of 5% of the dataset's size (half the 10% cap), so it fits every cap; the budget is recorded with the
  build (`caps/builds.csv`). The diskannpy wheel wraps the same release but times through Python and has no wheel for
  newer Pythons, so the C++ library is built directly (`install/diskann.sh`, against OpenBLAS: DiskANN's CMake insists
  on Intel MKL, 2.2 GB, for four BLAS/LAPACK calls). One change was needed: with one search thread, `load()` deadlocks
  in 0.7.0 (it holds the only I/O context while reading the medoids), so the server opens two contexts.
- **pgvector** (full suite): the adapter is defined; under a cap its server must run inside the group (restart the
  postmaster from the capped process, with `shared_buffers` under the cap). Not run here.

## The LAION scale suite (`scale`, `scale-test`)

Speed only: "validate correctness at smaller scales, speed at larger ones". LAION-400M's CLIP ViT-B/32 embeddings,
512 dimensions, at 10M, 40M and 100M (`laion-10m`, `laion-40m`, `laion-100m`), with text embeddings as
out-of-distribution queries, as VIBE's laion-clip-512.

**Source.** LAION's own release at `deploy.laion.ai/8f83b608504d46bb81708ec86e912220/embeddings/` (`img_emb/`,
`text_emb/`, `metadata/`): 410 parts of 1,000,448 rows (the last 518,720), f16, about 1 GB each. Part 0's images are
exactly VIBE's laion-clip-512 corpus (checked: within 9e-8 of VIBE's `train` after normalisation), and VIBE draws its
queries and learn sample from `text_emb_0.npy`, so the scale sets extend VIBE's. The address VIBE's `datasets.py`
uses, `the-eye.eu`, serves an expired TLS certificate (and 403 over plain HTTP); `cat-state/clip-embeddings` on Hugging
Face holds only one 9,212-row sample part; `laion/relaion400m` holds metadata only (and is gated).

**Sets.** A population is the first 100 parts (100,044,800 rows). Each size takes the same uniform random fraction of
every part's rows (10%, 40%, 100%), the smaller sizes a prefix of the same per-part permutation, so 10M is inside 40M
inside 100M and each is stratified by part, as `rust/REPORT_SCALING.md` built R1M inside R3M. Queries: 100 text rows of
part 0; the learn sample: 100,000 other text rows (for the dual fit). Visible sets: random 1%.

**Prep** (`prep_scale.py`) streams: one part at a time is downloaded, its rows for every size are normalised and
written as f16 (`corpus16.npy`; no f32 copy at scale), and the part is deleted. The same pass keeps, per query, the
running top K of the f32 similarities (K = the largest answer the shares need, plus one), so `order.npy` (the top 100),
the k-NN boundaries and the thresholds are exact up to f32 rounding, computed without an N x nq matrix (the f64 prep
would need 80 GB at 100M). No answer dumps and no check stage: exact k-NN is compared with the streaming truth, the
threshold answer sizes are recorded, and approximate k-NN gets its recall.

Items of an exact k-NN answer that differ from that truth are examined with check.py's rounding allowance (f16 vector
and query, 2^-11 each), plus n 2^-24 for the truth's f32 sums (`scale_diffs.csv`, marked `~` or `WRONG` in the report).
The streaming truth itself applies no allowance. In the scale test the few that differ are exact ties (two rows with
bit-identical stored vectors, one in the answer and one not) or pairs within about 2e-6 of each other whose order flips
when the query is rounded to f16, as vexact rounds it: with the f16 query vexact's item is the larger one.

**Systems**: vexact (exact, threshold, approximate; disk mode: its saved index mapped, the full vectors on file),
faiss flat over f16 (`IndexScalarQuantizer` QT_fp16, mapped; 10 queries once: a scan at 100M reads 100 GB), and for
approximate k-NN faiss IVF on disk, DiskANN's SSD index and faiss HNSW over f16 (`IndexHNSWSQ` QT_fp16, M 16,
efConstruction 200; in memory). `scale-test` is the same code on two parts: 200k inside 500k inside 1M.

**The machine is the cap.** The scale suite runs on whatever machine it is given, with no cgroup limit: the machine's
RAM is the cap. `run.py` records the machine with the results (`results/<suite>/machine.json`, shown at the top of the
report): CPU model, cores and SIMD flags (AVX2, AVX-512), RAM and swap, free space on the data disk, kernel, and
virtualisation (`systemd-detect-virt --vm` and `--container`, else /sys/class/dmi and the CPU's hypervisor flag). The
report labels each size's regime against that RAM: the f16 vectors and each system's footprint (its index on disk, or
what an in-memory system holds; and its build's peak RSS) `fits` (at most half the RAM), `partly fits` (up to the
RAM) or `exceeds RAM`. The in-memory systems run too: one the OOM killer stops, or that refuses (DiskANN when its
build would not fit on the disk), is reported as `does not fit`, not skipped. Each query's last repetition records its
storage reads (KiB its processes read, major faults, vexact's io_uring reads, DiskANN's reads and rounds), in the
`stats` column and the report's "Storage reads per query". The sizes are fixed, so runs on different machines share
data points.

**What each size needs** (from the 1M-scale measurements here, 512 dimensions; scale linearly):

Measured here at 200k, 500k and 1M (`results/scale-test/`, 3 build threads on a 2.1 GHz Xeon, 15 GB), extrapolated
linearly above that; nothing above 3M has been measured for the index (`rust/REPORT_SCALING.md`).

| | per million vectors (measured) | 10M | 40M | 100M |
|---|---|---|---|---|
| download (streamed, one 1 GB part on disk at a time) | all 100 parts whatever the size, 100 GB, plus 1 GB of text | 101 GB | 101 GB | 101 GB |
| prep time (download-bound here: 60-110 s a part) | | 2-3 h | 2-3 h | 2-3 h |
| corpus16.npy | 1.0 GB | 10 GB | 41 GB | 102 GB |
| vexact files: Xs.npy (fit) + saved index (vectors.bin, codes) + the disk-mode copy of the vectors | 2.3 GB + 1.0 GB | 34 GB | 136 GB | 340 GB |
| vexact codes alone (on file in disk mode, so page cache under a cap) | 0.3-0.4 GB (0.43 GiB at 768-d, threshold-db) | 4 GB | 16 GB | 40 GB |
| vexact query process in disk mode: anonymous memory at open | 152 MiB at 1M (90 MiB at yahoo's 677k) | ~1.5 GiB | ~6 GiB | ~15 GiB |
| vexact build: peak RSS (fit.py, then the encode, which holds the f16 vectors on the heap) | 2.0 / 2.4 / 3.0 GiB at 0.2 / 0.5 / 1M | ~15 GiB | ~54 GiB | ~130 GiB |
| vexact build time (3 threads) | 350 s (fit 139 s, encode 127 s) | ~1 h | ~4 h | ~10 h |
| vexact exact k-NN, k = 10, one thread | 53 ms (11 ms at 200k, 27 ms at 500k) | | | |
| faiss flat f16, mapped: file / one scan | 1.0 GB / 150 ms from the page cache | 10 GB / 1.5 s | 41 GB / 6 s | 102 GB / 15 s from the cache, about 50 s from a 2 GB/s disk |
| DiskANN: index / disk during the build (f32 corpus file, the graph build's copy, the index) / build peak RSS / build time | 6.0 GB / 8.4 GB / 4.1 GiB at 500k / 1,040 s (at 500k: 519 s) | 60 GB / 84 GB | 240 GB / 340 GB | 600 GB / 840 GB |
| faiss HNSW f16 (in memory): index / build peak RSS / build time | 1.07 GB / 2.8 GiB / 230 s | 11 GB / ~25 GiB / ~40 min | 43 GB / ~100 GiB | 107 GB / ~250 GiB: exceeds most machines' RAM |
| faiss IVF on disk: files / build peak RSS (lists are built in memory, then moved to disk) | 1.0 GB / 3.3 GiB | 10 GB / ~30 GiB | 41 GB / ~130 GiB | 102 GB / ~330 GiB: build in shards (faiss's `merge_ondisk`) |

The vexact rows assume the in-memory build (`v2::rebuild`) that the bindings use; at 100M its heap copy of the vectors
alone is 100 GB, so build where memory allows or with a build that writes the vectors to file as it goes (the earlier
capped runs built in memory, then moved the vectors to a file).

**Known limits of the index at scale** (flagged, not fixed here, since `rust/` is out of bounds for this work):

- Cluster size: 1,000 items per cluster by default, so 10,000 clusters at 10M and 100,000 at 100M; `--per-cluster by-n`
  (research/levels-clusters' rule) is untested above 3M.
- The approximate mode stores row positions within a cluster as u16: clusters over 65,536 rows overflow, which the
  by-n rule reaches from about 32M.
- `fit.py` fits its k-means on a 200,000-row sample, whatever N.
- Build memory above 3M is unmeasured (`rust/REPORT_SCALING.md` stops there).

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
- **The cap axis**: its runs here (yahoo-minilm-384 and laion-clip-512, at 50%, 25% and 10%) were removed from the
  results. They measured this VM's host cache, not storage: the files the cgroup pushed out of the guest's cache came
  back from the host's memory in about 33 us per 4 KiB read, against 650-750 us for a file nobody had read (the 32 us
  that `../threshold-db/capped/diskbench.c` reported here was that cache). They are in the history at 10d3605. The
  mechanics were checked again once on laion-s200k (`scale-test --stages cap`) and not committed.
- **The scale suite** was tested end to end on its small sizes (`scale-test`: 200k, 500k and 1M from two LAION parts;
  `results/scale-test/REPORT.md`, which records the machine); `scale` itself (10M to 100M) is for a machine with the
  disk and memory above.
- **vexact in `results/quick` is on `rust/` at d999606** (the adaptive approximate order over psi, the library's
  default levels) for glove-100-angular, yahoo-minilm-384-normalized and laion-clip-512-normalized, rerun alone
  (`--systems vexact`); the other systems' rows are from the earlier run, as their code did not change.
  **inaturalist-resnet-2048-cosine is not rerun**: its prep (a 4.1 GB download and a 4.1 GB corpus beside a 4.4 GB
  index) did not fit on the test machine's disk, so its vexact rows are still those of the earlier defaults (1,000
  items per cluster, the fit's six deterministic levels, the nprobe order) and are not comparable with the others.

## The scripts this suite replaced

The suite replaced these scripts, which were removed after its first quick run; they are in the history at 3a384da:

- `bench/`: `run.sh` (the Rust-timed filtered runs: `rust/src/ann.rs`, scored by `score.py`), `annlib.py`,
  `baselines.py`, `convert.py`, `filters.py`, `score.py`, `report.py`, `remeasure.sh`.
- `bench/threshold-db/`: `ann.py`, `check.py`, `dbrun.py`, `load.py`, `clickhouse.sh`, `summarise.py`, `plot.py`,
  `plot_filtered.py`, `table_filtered.py` (ClickHouse is replaced by pgvector).
- `bench/topk/`: `ann_topk.py`, `plot.py`.

Kept: `threshold-db/prep.py` (its output is what `rust/src/run2.rs` and `threshold-db/capped/` read),
`ch-user-files.xml` (mounted by `capped/clickhouse_capped.sh`), `fetch.py`, `fetch_species.py`, `taxa.py`,
`clusters.py`, `acorn_bench.cpp`, `acorn.patch`, and `bench/oracle.py` and `oracle_thr.py` (they check the Rust runner's
truth files). The REPORT files and results directories are the historical record of those runs.
