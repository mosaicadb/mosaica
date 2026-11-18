#!/bin/sh
# Run the replay checks on the three datasets, one process each: sh run_hier.sh SCRATCH_DIR  (after run_prep.sh)
S=$1
H=$(dirname "$0")
for d in glove-100-angular yahoo-minilm-384-normalized laion-clip-512-normalized; do
  OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 "$H/hier.py" $d "$S" > "$H/out_$d.txt" 2> "$S/err_$d.txt" &
done
wait
