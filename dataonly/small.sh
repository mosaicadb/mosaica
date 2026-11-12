#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/dataonly
until grep -q "finished p768 all" logs/p768.log; do sleep 10; done
./go.sh g100 B 4 4 PROF=Qmix RPROF=Qmix ANCH=all
./go.sh g100 D0 4 4 PROF=Qi RPROF=Qi ANCH=items
for N in g100 te3072 ada1536 n1024; do
  ./go.sh $N H50 4 4 PROF=h0.5 RPROF=h0.5 ANCH=items
  ./go.sh $N H25 4 4 PROF=h0.25 RPROF=h0.25 ANCH=items
done
./go.sh te3072 D0 4 4 PROF=Qi RPROF=Qi ANCH=items
echo finished small all
