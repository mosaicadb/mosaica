#!/bin/bash
# usage: runB.sh <pid to wait for> <core> <dataset K>...
pid=$1; core=$2; shift 2
while kill -0 $pid 2>/dev/null; do sleep 20; done
cd /home/joe/code/vexact-probe-clusters/probe
while [ $# -gt 0 ]; do
  d=$1; K=$2; shift 2
  OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 taskset -c $core .venv/bin/python -I expB.py $d out $K > expB_$(basename $d .hdf5).log 2>&1
done
