#!/bin/bash
# usage: check.sh BIN TAG [extra args...]: results/bounds/check.sh with this machine's paths
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
BIN=$1; TAG=$2; shift 2
$BIN --v2 --int --no-approx --index ${IDX:-$D/c50k} --queries $D/text/text_q.npy --item-queries $D/c50k_item_q.npy --results $D/res/$TAG --gtab --flags ${FLAGS:-repack} "$@" verify oracle --vitem ${VI:-200} --vtext ${VT:-200} > $D/res/$TAG.verify.out 2>&1
echo "exit $?"
grep -E '^VERIFY2|panicked|error' $D/res/$TAG.verify.out | cut -c1-900
python3 -I /tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv/bench/oracle_thr.py ${IDX:-$D/c50k} $D/res/$TAG/oracle 2>&1 | tail -5
