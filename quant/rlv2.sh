#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
until grep -q "finished rlv" logs/rlv.log; do sleep 20; done
V="PROF=Qmix SPH=1 L0=0 NORM=4 RSC=cheap ISC=1 SCHED=4,8,12,16,24,32"
./go.sh te3072 ra 2 $V RLV=48,64,96,128,192,256,384,576
./go.sh te3072 rb 2 $V RLV=48,64,80,96,128,160,192,256,384,576
echo finished rlv2
