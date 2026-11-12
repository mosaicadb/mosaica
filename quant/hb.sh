#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
until [ -f out/m768_F_w0.npz ] && [ -f out/m768_F_w1.npz ]; do sleep 20; done
F="PROF=Qmix SPH=1 L0=0 NORM=4 RSC=cheap ISC=1 SCHED=4,8,12,16,24,32 RLV=40,48,56,64,72,80,96,112,128,144,160,192,224,256,320,384,480,576"
for N in p768 g100 ada1536 te3072 n1024; do
  ./go.sh $N hbase 2 PROF=Qmix
  ./go.sh $N hbasefr 2 PROF=Qmix DCODE=fr
  ./go.sh $N hF 2 $F
  ./go.sh $N hFfr 2 $F DCODE=fr
  echo done $N
done
echo finished hb
