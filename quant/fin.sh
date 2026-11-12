#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
until grep -q "finished rc" logs/rc.log; do sleep 20; done
F="PROF=Qmix SPH=1 L0=0 NORM=4 RSC=cheap ISC=1 SCHED=4,8,12,16,24,32 RLV=40,48,56,64,72,80,96,112,128,144,160,192,224,256,320,384,480,576"
./go.sh g100 F 2 $F
./go.sh g100 bdet48 2 RLV=9999 SCHED=4,8,16,32,48
./go.sh n1024 F 2 $F
echo finished small
free -g >> logs/fin.log
./go.sh m768 base 2
./go.sh m768 F 2 $F
./go.sh m768 Fqi 2 $F PROF=Qi
echo finished fin
