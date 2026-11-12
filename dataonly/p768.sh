#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/dataonly
./go.sh p768 B 4 4 PROF=Qmix RPROF=Qmix ANCH=all
./go.sh p768 D0 4 4 PROF=Qi RPROF=Qi ANCH=items
./go.sh p768 dP 4 4 PROF=Qi RPROF=Qmix ANCH=all
./go.sh p768 dR 4 4 PROF=Qmix RPROF=Qi ANCH=all
./go.sh p768 dA 4 4 PROF=Qmix RPROF=Qmix ANCH=items
./go.sh p768 dPR 4 4 PROF=Qi RPROF=Qi ANCH=all
./go.sh p768 H50 4 4 PROF=h0.5 RPROF=h0.5 ANCH=items
./go.sh p768 H25 4 4 PROF=h0.25 RPROF=h0.25 ANCH=items
./go.sh p768 Q50 4 4 PROF=Qi50 RPROF=Qi50 ANCH=items
echo finished p768 all
