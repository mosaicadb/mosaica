#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/quant
until grep -q "finished fin" logs/fin.log; do sleep 30; done
export OMP_NUM_THREADS=2 OPENBLAS_NUM_THREADS=2
for N in p768 ada1536 n1024; do python3 floor2.py $N; done
echo finished floor
