#!/bin/bash
# Times the columnar kernel under the shared timing lock: waits until the lock is free, takes it atomically, runs, releases it.
cd "$(dirname "$0")"; V=..
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
while true; do
  while [ -e $V/TIMING.lock ]; do sleep 3; done
  if ( set -o noclobber; echo bulk > $V/TIMING.lock ) 2>/dev/null; then break; fi
done
trap 'rm -f $V/TIMING.lock' EXIT
sleep 2
{
  date
  for a in "1000 8 24 5" "1000 16 48 5" "1000 32 64 5" "1000 16 32 5" "1000 4 16 5"; do taskset -c 2 ./colscan $a; done
} > colscan.log 2>&1
