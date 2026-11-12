#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/dataonly
until grep -q "finished small all" logs/small.log; do sleep 10; done
./go.sh p768 HS 4 4 PROF=h0.25 RPROF=h0.5 ANCH=items
./go.sh p768 H37 4 4 PROF=h0.375 RPROF=h0.375 ANCH=items
./go.sh p768 H12 4 4 PROF=h0.125 RPROF=h0.125 ANCH=items
./go.sh m768 D0 4 4 PROF=Qi RPROF=Qi ANCH=items
./go.sh m768 H25 4 4 PROF=h0.25 RPROF=h0.25 ANCH=items
./go.sh m768 dA 4 4 PROF=Qmix RPROF=Qmix ANCH=items
./go.sh m768 dP 4 4 PROF=Qi RPROF=Qmix ANCH=all
./go.sh m768 dR 4 4 PROF=Qmix RPROF=Qi ANCH=all
echo finished big all
