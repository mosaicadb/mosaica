#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
until grep -q "finished n1024" logs/all_main.log; do sleep 20; done
V="PROF=Qmix SPH=1 L0=0 NORM=4 RSC=cheap ISC=1 SCHED=4,8,12,16,24,32"
for N in p768 ada1536; do
  ./go.sh $N ra 2 $V RLV=48,64,96,128,192,256,384,576
  ./go.sh $N rb 2 $V RLV=48,64,80,96,128,160,192,256,384,576
done
echo finished rlv
