#!/bin/bash
# usage: one.sh SET: wait up to 5 min for 60 s of quiet, run the set, say CLEAN if no other process passed 20% CPU
S=$(dirname $(readlink -f $0)); SET=$1; D=/home/joe/code/vexact-filter-data/prep
q=0; for i in $(seq 1 30); do if bash $S/busy.sh; then q=0; else q=$((q+1)); fi; [ $q -ge 6 ] && break; sleep 10; done
[ $q -ge 6 ] || { echo "NOT QUIET $(uptime)"; exit 1; }
TAG=s_${SET}_$(date +%H%M%S)
echo "start $(uptime)"
bash $S/run.sh $TAG $D/$SET.npy --plan-file $S/plan.txt | grep -E "exit|compacted|answers|panicked|error"
others=$(awk '{for(i=3;i<=NF;i++) print $i}' $S/$TAG.load | cut -d: -f1 | sort -u | wc -l)
maxl=$(awk '{print $2}' $S/$TAG.load | sort -n | tail -1)
if [ "$others" -le 1 ]; then echo "CLEAN $TAG maxload $maxl"; else echo "DIRTY $TAG"; cat $S/$TAG.load | awk 'NF>3'; fi
