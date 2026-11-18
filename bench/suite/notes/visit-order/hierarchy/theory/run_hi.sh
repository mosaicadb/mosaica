#!/bin/sh
# Sections H and I for the datasets named after the scratch directory: sh run_hi.sh SCRATCH_DIR SECTIONS DATASET...
S=$1; SEC=$2; shift 2
H=$(dirname "$0")
for d in "$@"; do
  OMP_NUM_THREADS=2 OPENBLAS_NUM_THREADS=2 python3 "$H/hier.py" $d "$S" $SEC 2> "$S/err2_$d.txt" | tail -n +2 >> "$H/out_$d.txt"
done
