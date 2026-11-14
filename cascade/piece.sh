#!/bin/bash
# usage: piece.sh <name> <lo> <hi>: queries [lo, hi) split over two workers on cores 10-11, one thread each.
set -e
name=$1; lo=$2; hi=$3; step=$(( (hi - lo + 1) / 2 ))
cd /home/joe/code/vexact-probe-clusters/cascade
pids=()
for w in 0 1; do
  a=$(( lo + w * step )); b=$(( a + step )); [ $b -gt $hi ] && b=$hi
  OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 timeout 590 taskset -c $(( 10 + w )) ../probe/.venv/bin/python -I -W ignore run.py \
    $name /home/joe/code/vexact-data/$name/cascade $a $b out/${name}_$(printf %05d $a)_$(printf %05d $b).npz > logs/run_${name}_$a.log 2>&1 &
  pids+=($!)
done
rc=0; for p in "${pids[@]}"; do wait $p || rc=1; done
tail -n 1 logs/run_${name}_*.log | grep -c done; exit $rc
