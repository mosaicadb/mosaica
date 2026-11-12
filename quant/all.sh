#!/bin/bash
# cumulative ablation for one dataset: usage all.sh NAME NW
N=$1; W=$2; cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
./go.sh $N base $W
./go.sh $N v1 $W PROF=Qmix
./go.sh $N v2 $W PROF=Qmix SPH=1
./go.sh $N v3 $W PROF=Qmix SPH=1 L0=0
./go.sh $N v4 $W PROF=Qmix SPH=1 L0=0 NORM=4
./go.sh $N v5 $W PROF=Qmix SPH=1 L0=0 NORM=4 RSC=cheap
./go.sh $N v6 $W PROF=Qmix SPH=1 L0=0 NORM=4 RSC=cheap ISC=1
./go.sh $N s2 $W PROF=Qmix SPH=1 L0=0 NORM=4 RSC=cheap ISC=1 SCHED=4,8,12,16,24,32
echo finished $N
