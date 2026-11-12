#!/bin/bash
cd /tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/entropy
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
for d in bioclip768 glove100 nyt256 bioclip1024 ada1536 te3_3072; do [ -f data/$d.npy ] || [ ${d:0:3} = bio ] || sleep 1; python3 scripts/nn.py $d > logs/nn_$d.log 2>&1; done
while pgrep -f runall.sh > /dev/null; do sleep 10; done
export OMP_NUM_THREADS=2 OPENBLAS_NUM_THREADS=2
for d in bioclip768 glove100 nyt256 bioclip1024 ada1536 te3_3072; do
  read K G < <(python3 -c "
import numpy as np; s=np.load('logs/spec_$d.npz'); print(int(s['K']), int(s['G']))")
  for cfg in "$((K*4)) $G" "$K $((G*4))" "$((K*4)) $((G*4))" "$((K*16)) $G"; do set -- $cfg
    python3 scripts/ent.py $d --K $1 --G $2 --rdonly > logs/rd_${d}_K$1_G$2.log 2>&1; done
done
