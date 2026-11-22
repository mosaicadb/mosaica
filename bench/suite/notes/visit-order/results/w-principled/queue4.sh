#!/bin/bash
# the fine grid: psi 2 to 9 by 0.25, the main rules, unfiltered and region-25, on the four datasets
set -x
M=corr,built,height,tail,corrx1.2,corrx1.5
PS=$(python3 -c "print(','.join(f'{2+0.25*i:g}' for i in range(29)))")
DIAG_PSIS=$PS DIAG_SETS=none,region-25 DIAG_MODES=$M DIAG_MISS=1 ./run_study.sh inaturalist-resnet-2048-cosine $PWD/costs-inat-92afad9.kv $PWD/fine-inat.txt > fine-inat.log 2>&1
DIAG_PSIS=$PS DIAG_SETS=none,region-25 DIAG_MODES=$M DIAG_MISS=1 ./run_study.sh laion-clip-512-normalized $PWD/costs-laion-92afad9.kv $PWD/fine-laion.txt > fine-laion.log 2>&1
DIAG_PSIS=$PS DIAG_SETS=none,region-25 DIAG_MODES=$M DIAG_MISS=1 ./run_study.sh arxiv-nomic-768-normalized $PWD/costs-arxiv-6f86a1c.kv $PWD/fine-arxiv.txt > fine-arxiv.log 2>&1
DIAG_PSIS=$PS DIAG_SETS=none,region-25 DIAG_MODES=$M DIAG_MISS=1 ./run_study.sh yahoo-minilm-384-normalized $PWD/costs-yahoo-6f86a1c.kv $PWD/fine-yahoo.txt > fine-yahoo.log 2>&1
