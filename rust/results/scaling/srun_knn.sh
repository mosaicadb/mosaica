#!/bin/bash
# usage: srun_knn.sh IDX TAG: exact k-NN (k = 10, 100) through the e2e harness, same binary, pinning and protocol as
# srun.sh (median over 3 runs of the median over 20 queries); one tiny threshold cell is timed alongside.
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
P=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv/rust/results/bounds/plan_fixed.txt
IDX=$1; TAG=$2; shift 2
taskset -c 1-3 python3 -I $D/scripts/peakrss.py ${BIN:-$D/bin/vexact_ndl2k} --v2 --int --no-approx --plan-file $P --index $IDX ${DLV:---dlv 16,32} \
  --queries $D/text/text_q.npy --item-queries $D/scale_item_q.npy --results $D/res/$TAG --gtab --flags repack --pin 2 --threads 1 \
  "$@" e2e --nq 20 --runs 3 --knn 1 --knn-k 10,100 --sizes 0.0001 > $D/res/$TAG.out 2>&1
echo "exit $?"
grep -E "^e2e2 a,.*knn|peak RSS|panicked|error" $D/res/$TAG.out | cut -c1-200
