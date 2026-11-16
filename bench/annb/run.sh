#!/bin/bash
# Run the ANN-Benchmarks comparison: every algorithm on every dataset, one container at a time, then a plot and a
# CSV per dataset in ANN_BENCHMARKS_DIR/results/.
# usage: bash bench/annb/run.sh ANN_BENCHMARKS_DIR [DATASET ...]
#   Before the first run: bash bench/annb/install.sh ANN_BENCHMARKS_DIR, and
#   python install.py --algorithm A for each image in IMAGES (in the checkout). ALGOS and DATASETS can be overridden.
#   Results already present are kept; delete ANN_BENCHMARKS_DIR/results/<dataset>/10/<algorithm> to re-run one.
set -euo pipefail
AB=$(cd "$1" && pwd)
shift
DATASETS=${*:-glove-25-angular glove-100-angular nytimes-256-angular coco-t2i-512-angular dbpedia-openai-1000k-angular}
ALGOS=${ALGOS:-vexact-exact vexact-approx bruteforce-blas hnswlib hnsw(faiss) faiss-ivfpqfs scann glass pynndescent vamana(diskann)}
PY=${PYTHON:-python3}
cd "$AB"
for d in $DATASETS; do
  # coco and dbpedia are not hosted at ann-benchmarks.com; the checkout builds them (dbpedia: about 6 GB of
  # embeddings from Hugging Face, then brute-force ground truth)
  [ -f "data/$d.hdf5" ] || $PY create_dataset.py --dataset "$d"
  for a in $ALGOS; do
    $PY run.py --dataset "$d" --algorithm "$a" --runs "${RUNS:-2}" --parallelism 1 --timeout "${TIMEOUT:-14400}"
  done
  $PY plot.py --dataset "$d" --output "results/$d.png" --x-scale logit --y-scale log
done
$PY data_export.py --output results/export.csv
