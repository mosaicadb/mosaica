#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
until grep -q "finished hb" logs/hb.log; do sleep 20; done
F="PROF=Qmix SPH=1 L0=0 NORM=4 RSC=cheap ISC=1 SCHED=4,8,12,16,24,32 RLV=40,48,56,64,72,80,96,112,128,144,160,192,224,256,320,384,480,576"
OMP_NUM_THREADS=2 OPENBLAS_NUM_THREADS=2 python3 floor2.py p768 > logs/floor_p768.log 2>&1
OMP_NUM_THREADS=2 OPENBLAS_NUM_THREADS=2 python3 floor2.py n1024 > logs/floor_n1024.log 2>&1
./go.sh m768 hbase 2 PROF=Qmix
./go.sh m768 hFfr 2 $F DCODE=fr
echo finished hm
