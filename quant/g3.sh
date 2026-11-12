#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
until grep -q "finished rlv2" logs/rlv2.log; do sleep 20; done
./go.sh g100 bdet16 2 RLV=9999 SCHED=4,8,16
./go.sh g100 bdet32 2 RLV=9999
./go.sh g100 s2det 2 PROF=Qmix SPH=1 L0=0 NORM=4 RSC=cheap ISC=1 SCHED=4,8,12,16,24,32 RLV=9999
echo finished g3
