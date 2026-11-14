#!/bin/bash
# usage: e2e.sh BIN TAG [extra args...]: run.sh's end-to-end threshold timing at the six answer sizes, 20 queries, 3 runs.
B=/home/joe/code/vexact-bounds-target/bin/$1; T=$2; shift 2
until ! pgrep -f "vexact-bounds-target/bin/.*--pin 10" >/dev/null; do sleep 5; done
$(dirname $0)/run.sh $B $T e2e --nq 20 --runs 3 --knn 0 --sizes 0.0001,0.001,0.01,0.1,0.5,0.75 "$@" | head -1
