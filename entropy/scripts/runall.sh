#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/entropy
export OMP_NUM_THREADS=2 OPENBLAS_NUM_THREADS=2
for d in bioclip768 nyt256 bioclip1024 ada1536 te3_1536; do python3 scripts/ent.py $d > logs/ent_$d.log 2>&1; done
python3 scripts/ent.py bioclip768 --control > logs/ent_bioclip768_control.log 2>&1
python3 scripts/ent.py te3_3072 > logs/ent_te3_3072.log 2>&1
