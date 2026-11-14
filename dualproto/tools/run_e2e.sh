#!/bin/bash
# Timed runs, one at a time, holding the shared timing lock. Usage: run_e2e.sh TAG DATA PROFILE [extra args]
P=/tmp/claude-0/-home-user-mosaica/8c244050-1af9-5b77-a4e8-97638e77c857/scratchpad/vc/proto; LOCK=/tmp/claude-0/-home-user-mosaica/8c244050-1af9-5b77-a4e8-97638e77c857/scratchpad/vc/TIMING.lock
TAG=$1; DATA=$2; PROF=$3; shift 3
touch $LOCK
trap 'rm -f $LOCK' EXIT
sleep 20
echo "uptime before $TAG: $(uptime)" | tee -a $P/logs/uptime.log
VEXACT_CLIPSTATS=1 $P/target/release/vexact --v2 e2e --nq 100 --nflat 10 --data $DATA --profile $PROF --res $P/results/$TAG "$@" > $P/logs/${TAG}_e2e.log 2>&1
echo "uptime after $TAG: $(uptime)" | tee -a $P/logs/uptime.log
rm -f $LOCK
