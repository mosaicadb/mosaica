#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
until grep -q "finished floor" logs/floor.log; do sleep 30; done
F="PROF=Qmix SPH=1 L0=0 NORM=4 RSC=cheap ISC=1 SCHED=4,8,12,16,24,32"
for N in p768 ada1536; do ./go.sh $N rd 2 $F RLV=32,44,56,64,72,80,96,112,128,144,160,192,224,256,320,384,480,576; done
echo finished rd
