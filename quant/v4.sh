#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
until grep -q "finished rd" logs/rd.log; do sleep 30; done
./go.sh p768 v4 2 PROF=Qmix SPH=1 L0=0 NORM=4
echo finished v4
