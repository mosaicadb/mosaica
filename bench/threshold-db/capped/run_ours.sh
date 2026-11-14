#!/usr/bin/env bash
# Our index under a memory cap. The process builds the index's codes in memory (uncapped scope, no swap), writes the
# row-ordered f16 vectors to FULL and maps them, then lowers its own cgroup's memory.max to CAP and reclaims the page
# cache before the first query.
# usage: bash run_ours.sh TAG RESULTS_DIR CAP_BYTES "EXTRA dbcmp ARGS"   (env RLV=56,96,160,320 for another schedule,
#        CORES=8-11, THREADS for the build's rayon pool)
set -euo pipefail
TAG=$1 RES=$2 CAP=$3 EXTRA=${4:-}
D=/home/joe/code/vexact-data/bioclip2-1m/prep
IDX=/home/joe/code/vexact-benchup-data/index_r4
BIN=/home/joe/code/vexact-capped-target/release/vexact
FULL=${FULL:-/home/joe/code/vexact-capped-data/full.f16}
PLAN=/home/joe/code/vexact-benchup/bench/threshold-db/results/ours/plan.txt
mkdir -p "$RES"
RLVARG=""; [ -n "${RLV:-}" ] && RLVARG="--rlv $RLV"
RAYON_NUM_THREADS=${THREADS:-4} systemd-run --user --scope --collect -q -p MemoryMax=9G -p MemorySwapMax=0 -- \
    taskset -c ${CORES:-8-11} $BIN --v2 --int --gtab --flags repack --no-approx $RLVARG --plan-file $PLAN \
    --index $IDX --queries $D/queries.npy --results "$RES" dbcmp --thr $D/thresholds.npy --tag "$TAG" \
    --full-file "$FULL" --cap "$CAP" --full-advice random --willneed $EXTRA 2>&1 \
    | grep -E "^dbcmp|^\[mem|^\[cap|^\[disk|built v2|panic|error"
