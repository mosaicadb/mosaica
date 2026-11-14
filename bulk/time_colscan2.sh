#!/bin/bash
# Times version 2 of the columnar kernel under the shared timing lock; this agent's own python run is paused meanwhile.
cd "$(dirname "$0")"; V=..
while true; do
  while [ -e $V/TIMING.lock ]; do sleep 3; done
  if ( set -o noclobber; echo bulk > $V/TIMING.lock ) 2>/dev/null; then break; fi
done
trap 'pkill -CONT -f "bulk_py_marker|qcol.py|adapt.py|quant.py|floor.py" ; rm -f $V/TIMING.lock' EXIT
pkill -STOP -f "qcol.py|adapt.py|quant.py|floor.py"
sleep 1
{
  date
  gcc -O3 -march=native -o colscan2 colscan2.c 2>&1 | head -20
  for a in "1000 8 24 5 1" "1000 16 48 5 1" "1000 32 64 5 1" "1000 16 32 5 1" "1000 16 48 5 0" "4000 16 48 3 1"; do taskset -c 2 ./colscan2 $a; done
} > colscan2.log 2>&1
