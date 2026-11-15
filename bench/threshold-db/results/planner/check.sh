#!/bin/bash
# usage: check.sh BIN TAG [extra args...]: verify and the float64 oracle on the 50k four-level index (200 item, 200 text
# queries), s3 configuration, on the cores in $CORES (default 3).
D=/home/joe/code/vexact-combined-data; R=/home/joe/code/vexact-planner-target/res
BIN=$1; TAG=$2; shift 2
taskset -c ${CORES:-3} $BIN --v2 --int --no-approx --index ${IDX:-/home/joe/code/vexact-const-data/c50k_r4} --queries /home/joe/code/vexact-data/bioclip2-text/text_q.npy --item-queries $D/c50k/item_q.npy --results $R/$TAG --gtab --flags ${FLAGS:-repack} "$@" verify oracle --vitem ${VI:-200} --vtext ${VT:-200} > $R/$TAG.verify.out 2>&1
echo "exit $?"
grep -E '^VERIFY2|panicked|error|randomised constants|query budget' $R/$TAG.verify.out | cut -c1-900
OPENBLAS_NUM_THREADS=1 taskset -c ${CORES:-3} ${PY:-/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/venv/bin/python} -I /home/joe/code/vexact-combined/bench/oracle_thr.py ${IDX:-/home/joe/code/vexact-const-data/c50k_r4} $R/$TAG/oracle 2>&1 | tail -5
