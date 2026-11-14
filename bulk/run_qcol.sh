#!/bin/bash
# Runs the quantised columnar experiment for each basis in turn, one thread, one process at a time, gated on the timing lock.
cd "$(dirname "$0")"
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
V=..
for b in ${BASES:-pca sq_item sq_text}; do
  while [ -e $V/TIMING.lock ]; do sleep 5; done
  if [ "$b" = pca ]; then B=pca; else B=$V/$b.npy; fi
  BASIS=$B TSTEP=${TSTEP:-16} NQI=${NQI:-14} $V/venv/bin/python qcol.py qcol_$b.json > qcol_$b.log 2>&1
done
