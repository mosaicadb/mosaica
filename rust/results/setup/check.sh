#!/bin/bash
# usage: check.sh BIN TAG: verify and the float64 oracle on the 50k index (200 item, 200 text queries), s3 configuration.
D=/home/joe/code/vexact-combined-data; R=/home/joe/code/vexact-setup-target/res
BIN=$1; TAG=$2; shift 2
taskset -c 9-11 $BIN --v2 --int --no-approx --index $D/c50k/dual --queries /home/joe/code/vexact-data/bioclip2-text/text_q.npy --item-queries $D/c50k/item_q.npy --results $R/$TAG --gtab --flags ${FLAGS:-repack} "$@" verify oracle --vitem 200 --vtext 200 > $R/$TAG.verify.out 2>&1
echo "exit $?"
grep -E '^VERIFY2|panicked|error' $R/$TAG.verify.out | cut -c1-900
python3 /home/joe/code/vexact-combined/bench/oracle_thr.py $D/c50k/dual $R/$TAG/oracle 2>&1 | tail -5
