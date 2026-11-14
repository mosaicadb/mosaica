#!/bin/bash
# Runs the order experiment for each basis in turn, one thread, one process at a time.
cd "$(dirname "$0")"
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
V=..
for b in pca sq_item sq_text sq_mix; do
  if [ "$b" = pca ]; then B=pca; else B=$V/$b.npy; fi
  BASIS=$B ORDERS=fix,prod TSTEP=4 NQI=60 $V/venv/bin/python adapt.py adapt_$b.json > adapt_$b.log 2>&1
done
