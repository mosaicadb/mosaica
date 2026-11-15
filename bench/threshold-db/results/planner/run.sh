#!/bin/bash
# usage: run.sh TAG SETS WHATS [extra args]: dbcmp at 1M, one thread pinned to core 4, the index opened on cores 6-7.
# SETS: r (random 1/10/50%), g (genus 1/10/50%), or a comma list of .npy files.
TAG=$1; SETS=$2; WHATS=$3; shift 3
D=/home/joe/code/vexact-filter-data/prep
case $SETS in
  r) V=$D/vis_1.npy,$D/vis_10.npy,$D/vis_50.npy ;;
  g) V=$D/gen_1.npy,$D/gen_10.npy,$D/gen_50.npy ;;
  *) V=$SETS ;;
esac
H=$(cd "$(dirname "$0")" && pwd)
if [ "${PLAN:-}" = calib ]; then PF="--calib 16"; else PF="--plan-file ${PLAN:-$H/plan_r4.txt}"; fi
cd $H
taskset -c 6,7 ${BIN:-/home/joe/code/vexact-planner-target/release/vexact} --v2 --int --gtab --flags repack --no-approx \
  --index /home/joe/code/vexact-benchup-data/index_r4 --queries $D/queries.npy --results $H/runs \
  --core 4 --pin 4 $PF dbcmp --thr $D/thresholds.npy --cmp-vis $V --skip-all \
  --threads 1 --step 10 --reps 5 --tag $TAG --cmp-what $WHATS "$@" > $H/runs/$TAG.out 2>&1
echo "exit $?"
grep -E "^dbcmp|panicked|error" $H/runs/$TAG.out
