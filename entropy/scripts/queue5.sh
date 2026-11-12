#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/entropy
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
while pgrep -f queue4.sh > /dev/null; do sleep 5; done
for d in ada1536 bioclip768 te3_3072 nyt256 bioclip1024 glove100; do python3 scripts/nn_levels.py $d > logs/nnlev_$d.log 2>&1; done
