#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/entropy
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
while pgrep -f queue5.sh > /dev/null; do sleep 5; done
for d in ada1536 bioclip768 bioclip1024 te3_3072; do python3 scripts/ica.py $d 384 > logs/ica384_$d.log 2>&1; done
