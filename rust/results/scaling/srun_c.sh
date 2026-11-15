#!/bin/bash
# usage: srun.sh IDX TAG [extra commands...]: one scaling process: the NDL 2 binary with --dlv 16,32, s3 configuration,
# fixed plan constants, the scaling item queries; cores 1-3, timing pinned to core 2; end-to-end at six answer sizes and
# the profiled breakdown, after any extra commands (e.g. verify, which runs first); peak RSS at the end of the output.
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
P=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv/rust/results/bounds/plan_fixed.txt
IDX=$1; TAG=$2; shift 2
taskset -c 1-3 python3 -I $D/scripts/peakrss.py ${BIN:-$D/bin/vexact_ndl2k} --v2 --int --no-approx --plan-file $P --index $IDX ${DLV:---dlv 16,32} \
  --queries $D/text/text_q.npy --item-queries ${ITEMQ:-$D/scale_item_q.npy} --results $D/res/$TAG --gtab --flags repack --pin 2 --threads 1 \
  "$@" e2e --nq 20 --runs 3 --knn 1 --knn-k 10,100 --sizes 0.0001,0.001,0.01,0.1,0.5,0.75 breakdown --bfracs ${BFRACS:-0.0001,0.01,0.5} > $D/res/$TAG.out 2>&1
echo "exit $?"
grep -E "^VERIFY2 40|^VERIFY2 checker|peak RSS|panicked|error" $D/res/$TAG.out | cut -c1-220
