#!/bin/bash
# usage: go.sh NAME TAG NW PAR [env assignments...]   runs NW workers of run3.py, PAR at a time
NAME=$1; TAG=$2; NW=$3; PAR=$4; shift 4
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/dataonly
mkdir -p logs
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
F="DCODE=fr SPH=1 L0=0 NORM=4 RSC=cheap ISC=1 SCHED=4,8,12,16,24,32 RLV=40,48,56,64,72,80,96,112,128,144,160,192,224,256,320,384,480,576"
for ((w=0; w<NW; w+=PAR)); do
  for ((v=w; v<w+PAR && v<NW; v++)); do
    env $F "$@" python3 run3.py $NAME $v $NW $TAG > logs/${NAME}_${TAG}_w$v.log 2>&1 &
  done
  wait
done
echo "finished $NAME $TAG"
