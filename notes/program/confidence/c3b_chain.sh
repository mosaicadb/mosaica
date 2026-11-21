#!/bin/bash
# Runs the eight configurations of c3b.py one after another on core 4.
cd "$(dirname "$0")"
for g in 12 6; do for c in 8 4; do for curve in ideal built; do
  nice -n 10 taskset -c 4 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 -W ignore c3b.py $g $c $curve > c3b_g${g}_c${c}_${curve}.log 2>&1
done; done; done
