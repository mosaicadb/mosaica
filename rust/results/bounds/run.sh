#!/bin/bash
# usage: run.sh BIN TAG [extra args...]: the s3 configuration at 1M (four randomised levels), cores 9-11, timing pinned
# to core 10.
D=/home/joe/code/vexact-combined-data; R=/home/joe/code/vexact-bounds-target/res
BIN=$1; TAG=$2; shift 2
taskset -c 9-11 $BIN --v2 --int --no-approx --plan-file /home/joe/code/vexact-bounds/rust/results/bounds/plan_fixed.txt --index ${IDX:-/home/joe/code/vexact-rand-data/c1m_r4} --queries /home/joe/code/vexact-data/bioclip2-text/text_q.npy --item-queries $D/c1m/item_q.npy --results $R/$TAG --gtab --flags ${FLAGS:-repack} --pin 10 --threads 1 "$@" > $R/$TAG.out 2>&1
echo "exit $?"
grep -E "^setup2|^breakdown2|^e2e2 [a-c],|panicked|error" $R/$TAG.out | cut -c1-400
