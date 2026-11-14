#!/bin/bash
# usage: run.sh BIN TAG [extra args...]: end-to-end threshold timing on the 1M four-level index, s3 configuration, fixed
# plan constants, six answer sizes, 20 item and 20 text queries, 3 runs; the process on $CORES (default 6,7), timing
# pinned to $PIN (default 7).
D=/home/joe/code/vexact-combined-data; R=/home/joe/code/vexact-const-target/res
BIN=$1; TAG=$2; shift 2
taskset -c ${CORES:-6,7} $BIN --v2 --int --no-approx --plan-file $(dirname $0)/../bounds/plan_fixed.txt --index ${IDX:-/home/joe/code/vexact-const-data/c1m_r4} --queries /home/joe/code/vexact-data/bioclip2-text/text_q.npy --item-queries $D/c1m/item_q.npy --results $R/$TAG --gtab --flags ${FLAGS:-repack} --pin ${PIN:-7} --threads 1 e2e --nq 20 --runs 3 --knn 0 --sizes 0.0001,0.001,0.01,0.1,0.5,0.75 "$@" > $R/$TAG.out 2>&1
echo "exit $?"
grep -E "randomised constants|query budget|^e2e2 [a-c],|panicked|error" $R/$TAG.out | cut -c1-200
