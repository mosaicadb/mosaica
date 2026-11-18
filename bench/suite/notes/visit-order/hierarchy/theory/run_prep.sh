#!/bin/sh
# Prepare the three datasets: sh run_prep.sh SCRATCH_DIR  (about 5 GB of scores and labels; a few minutes on 2 threads)
S=$1
H=$(dirname "$0")
mkdir -p "$S"
for d in glove-100-angular yahoo-minilm-384-normalized laion-clip-512-normalized; do
  OMP_NUM_THREADS=2 OPENBLAS_NUM_THREADS=2 python3 "$H/prep.py" $d "$S/$d"
done
