#!/bin/bash
# Runs the two-plane columnar experiment for each basis in turn, one thread, one process at a time, gated on the timing lock.
cd "$(dirname "$0")"
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
V=..
run() {  # basis name, orders
  while [ -e $V/TIMING.lock ]; do sleep 5; done
  if [ "$1" = pca ]; then B=pca; else B=$V/$1.npy; fi
  BASIS=$B ORDERS=$2 TSTEP=20 NQI=10 $V/venv/bin/python qcol2.py qcol2_$1.json > qcol2_$1.log 2>&1
}
run sq_item fix,adapt,adapts,adaptP
run pca fix,adapt,adapts
run sq_text fix,adapt,adapts
