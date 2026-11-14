#!/bin/bash
# usage: run.sh BIN TAG [extra args...]: results/bounds/run.sh with this machine's paths; the process on cores 1-3,
# timing pinned to core 2
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
BIN=$1; TAG=$2; shift 2
taskset -c 1-3 $BIN --v2 --int --no-approx --plan-file /tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv/rust/results/bounds/plan_fixed.txt --index ${IDX:-$D/c1m} --queries $D/text/text_q.npy --item-queries $D/c1m_item_q.npy --results $D/res/$TAG --gtab --flags ${FLAGS:-repack} --pin 2 --threads 1 "$@" > $D/res/$TAG.out 2>&1
echo "exit $?"
grep -E "^setup2|^breakdown2|^e2e2 [a-c],|panicked|error" $D/res/$TAG.out | cut -c1-400
