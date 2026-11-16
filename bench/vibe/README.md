# The index in VIBE

[VIBE](https://github.com/vector-index-bench/vibe) (Vector Index Benchmark for Embeddings) is an ANN-Benchmarks
derivative with modern embedding datasets, including out-of-distribution ones in which text queries search image
embeddings. It runs each algorithm in an Apptainer (Singularity) image on one core, with k = 100 by default.

The adapter is the same as for ANN-Benchmarks: `bench/annb/vexact/module.py` over the bindings in
`bench/annb/vexact-py`. This folder holds only what differs: the image definition, the parameter grid and the
scripts.

- `vexact/image.def`: builds the bindings on VIBE's base image. `install.sh` fills in the path of the sources.
- `vexact/config.yml`: `vexact-exact` and `vexact-approx` (nprobe 1 to 512) for the `cosine` and `normalized` datasets.
- `install.sh`: copies the algorithm into a checkout, with `rust/` taken from the revision pinned in
  `bench/annb/install.sh`, and builds `images/vexact.sif`.
- `run.sh`: every algorithm on every dataset, then VIBE's result export.

## Running it

```bash
git clone https://github.com/vector-index-bench/vibe.git ~/vibe
cd ~/vibe && git checkout 1e7f776    # the revision these files were checked against
for a in hnswlib faiss scann glass pynndescent diskann; do ./install.sh --algorithm $a; done
bash /path/to/mosaica/bench/vibe/install.sh ~/vibe
PARALLELISM=6 bash /path/to/mosaica/bench/vibe/run.sh ~/vibe
./plot.sh --dataset yahoo-minilm-384-normalized     # after building plot.sif; see VIBE's README
```

VIBE needs Apptainer (or Singularity); its README gives the install. Datasets download from Hugging Face on first use.
`run.sh` takes dataset names after the checkout, and reads `ALGOS`, `PARALLELISM`, `RUNS` and `TIMEOUT` from the
environment. VIBE pins each parallel run to its own core and recommends turning SMT off and about 16 GB of memory per
core in use.

## Datasets

The index supports cosine similarity only, so the default list is VIBE's `cosine` and `normalized` datasets of up to
1.5M vectors. The `euclidean` and `ip` datasets are left out.

| Dataset | Vectors | Dimensions | Kind |
|---|---|---|---|
| yahoo-minilm-384-normalized | 677k | 384 | text |
| glove-200-cosine | 1.19M | 200 | words |
| imagenet-clip-512-normalized | 1.28M | 512 | images |
| arxiv-nomic-768-normalized | 1.34M | 768 | text |
| landmark-dino-768-cosine | 761k | 768 | images |
| inaturalist-resnet-2048-cosine | 499k | 2,048 | images |
| laion-clip-512-normalized | 1.0M | 512 | text queries against images (out of distribution) |
| imagenet-align-640-normalized | 1.28M | 640 | text queries against images (out of distribution) |
| yandex-200-cosine | 1.0M | 200 | text queries against images (out of distribution) |

Larger ones are run by naming them: msmarco-qwen-1024-normalized (8.8M), hotpotqa-harrier-640-normalized (5.2M) and
dpr-jina-768-normalized (21M).

## Comparison libraries

The default list matches the ANN-Benchmarks one: hnswlib, faiss HNSW, faiss IVF-PQ fast scan, ScaNN, glass,
PyNNDescent and DiskANN (Vamana). VIBE's own images are used unchanged. Its DiskANN image already installs
`diskannpy` 0.7.0, so it needs no patch. VIBE has others worth adding through `ALGOS`: `roargraph` (a graph built for
out-of-distribution queries), `ivf-rabitq(faiss)`, `vamana-lvq(svs)` and `lorann`.

## Out-of-distribution datasets

The index is built with `fit.py --basis dual`. VIBE gives an algorithm marked `ood: true` a sample of the query
distribution (`fit_ood`), and on those datasets the adapter passes it to `fit.py --query-sample`, which fits the
basis's query side to where the queries point. On the in-distribution datasets the basis is fitted to the corpus alone.
