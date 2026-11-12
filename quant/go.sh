#!/bin/bash
# usage: go.sh NAME TAG NW [env assignments...]   runs NW workers, two at a time
NAME=$1; TAG=$2; NW=$3; shift 3
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
mkdir -p logs
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
for ((w=0; w<NW; w+=2)); do
  env "$@" python3 run2.py $NAME $w $NW $TAG > logs/${NAME}_${TAG}_w$w.log 2>&1 &
  if (( w+1 < NW )); then env "$@" python3 run2.py $NAME $((w+1)) $NW $TAG > logs/${NAME}_${TAG}_w$((w+1)).log 2>&1 & fi
  wait
done
