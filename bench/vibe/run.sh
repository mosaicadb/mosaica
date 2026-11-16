#!/bin/bash
# Run the VIBE comparison: every algorithm on every dataset, then export the results.
# usage: bash bench/vibe/run.sh VIBE_DIR [DATASET ...]
#   Before the first run: bash bench/vibe/install.sh VIBE_DIR, and ./install.sh --algorithm A in the checkout for each
#   comparison library. ALGOS overrides the algorithms; PARALLELISM (default 1) runs that many instances at once, each
#   pinned to its own core. Results already present are kept.
set -euo pipefail
VIBE=$(cd "$1" && pwd)
shift
# the datasets with cosine or normalized vectors, up to 1.5M vectors (the index supports no other metric yet);
# laion, imagenet-align and yandex are out-of-distribution (text queries against images)
DATASETS=${*:-yahoo-minilm-384-normalized glove-200-cosine imagenet-clip-512-normalized arxiv-nomic-768-normalized \
  landmark-dino-768-cosine inaturalist-resnet-2048-cosine laion-clip-512-normalized imagenet-align-640-normalized \
  yandex-200-cosine}
ALGOS=${ALGOS:-vexact-exact vexact-approx hnswlib hnsw(faiss) ivfpqfs(faiss) scann glass pynndescent vamana(diskann)}
cd "$VIBE"
for d in $DATASETS; do
  for a in $ALGOS; do
    python3 run.py --dataset "$d" --algorithm "$a" --runs "${RUNS:-2}" --parallelism "${PARALLELISM:-1}" --timeout "${TIMEOUT:-14400}"
  done
done
./export_results.sh --parallelism "${PARALLELISM:-1}"
