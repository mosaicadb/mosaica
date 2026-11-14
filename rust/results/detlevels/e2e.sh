#!/bin/bash
# usage: e2e.sh BIN TAG [extra args...]: end-to-end threshold timing at the six answer sizes, 20 queries, 3 runs
/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data/scripts/run.sh $1 $2 "${@:3}" e2e --nq 20 --runs 3 --knn 0 --sizes 0.0001,0.001,0.01,0.1,0.5,0.75
