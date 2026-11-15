#!/bin/bash
# usage: run.sh TAG SETS(comma list of .npy) [extra args]; logs other processes over 20% CPU every 10 s to TAG.load
S=$(dirname $(readlink -f $0)); TAG=$1; V=$2; shift 2; D=/home/joe/code/vexact-filter-data/prep
( while true; do echo "$(date +%T) $(cut -d' ' -f1 /proc/loadavg) $(ps -eo pid,pcpu,comm --sort=-pcpu | awk -v p=$PPID 'NR>1 && $2>20 {printf "%s/%s:%s ",$1,$3,$2}')"; sleep 10; done ) > $S/$TAG.load 2>&1 &
LP=$!
taskset -c 10,11 /home/joe/code/vexact-planner-target/release/vexact --v2 --int --gtab --flags repack --no-approx \
  --index /home/joe/code/vexact-benchup-data/index_r4 --queries $D/queries.npy --results $S \
  --core 10 --pin 10 "$@" dbcmp --thr $D/thresholds.npy --cmp-vis $V --skip-all \
  --threads 1 --step 10 --reps 5 --tag $TAG --cmp-what flat,index,cvindex > $S/$TAG.out 2>&1
echo exit $?
kill $LP
grep -E "^dbcmp|panicked|error" $S/$TAG.out
