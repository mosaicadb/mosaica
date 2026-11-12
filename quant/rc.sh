#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
until grep -q "finished g3" logs/g3.log; do sleep 20; done
V="PROF=Qmix SPH=1 L0=0 NORM=4 RSC=cheap ISC=1 SCHED=4,8,12,16,24,32"
for N in p768 ada1536 te3072; do
  ./go.sh $N rc 2 $V RLV=40,48,56,64,72,80,96,112,128,144,160,192,224,256,320,384,480,576
done
echo finished rc
