#!/bin/bash
# HISTORICAL: the record of the 15 Nov runs in REPORT_PLANNER.md, on that machine and binary. It does not run with the
# current binary: since "The planner without timing" (rust/REPORT_BUILD.md) `--calib` is gone and a plan file takes
# only r_rows, r_kappa, r_seed, io_us and io_gbps, so plan_r4.txt's timed constants are refused. To rerun these sets
# now, leave PLAN unset and drop $PF (the planner needs no constants in memory), or give a plan file of those names.
if [ -z "${HISTORICAL_OK:-}" ]; then echo "run.sh is historical (see its header); set HISTORICAL_OK=1 to run it anyway" >&2; exit 2; fi
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
