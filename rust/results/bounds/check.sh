#!/bin/bash
# usage: check.sh BIN TAG [extra args...]: verify and the float64 oracle on the 50k four-level index (200 item, 200 text
# queries), s3 configuration, cores 9-11.
D=/home/joe/code/vexact-combined-data; R=/home/joe/code/vexact-bounds-target/res
BIN=$1; TAG=$2; shift 2
taskset -c 9-11 $BIN --v2 --int --no-approx --index ${IDX:-/home/joe/code/vexact-rand-data/c50k_r4} --queries /home/joe/code/vexact-data/bioclip2-text/text_q.npy --item-queries $D/c50k/item_q.npy --results $R/$TAG --gtab --flags ${FLAGS:-repack} "$@" verify oracle --vitem ${VI:-200} --vtext ${VT:-200} > $R/$TAG.verify.out 2>&1
echo "exit $?"
grep -E '^VERIFY2|panicked|error' $R/$TAG.verify.out | cut -c1-900
python3 /home/joe/code/vexact-combined/bench/oracle_thr.py ${IDX:-/home/joe/code/vexact-rand-data/c50k_r4} $R/$TAG/oracle 2>&1 | tail -5
