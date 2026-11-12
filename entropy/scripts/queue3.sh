#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/entropy
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
for d in nyt256 glove100 bioclip768 bioclip1024 ada1536 te3_3072; do python3 scripts/nn.py $d > logs/nn_$d.log 2>&1; done
