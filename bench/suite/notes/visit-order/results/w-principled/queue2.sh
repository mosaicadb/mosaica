#!/bin/bash
# the second round: the height rules and level multipliers on corr, unfiltered, on the four datasets
set -x
M=height,tail,tailp,corrx1.2,corrx1.5,corrx2
DIAG_SETS=none DIAG_MODES=$M DIAG_MISS=1 ./run_study.sh inaturalist-resnet-2048-cosine $PWD/costs-inat-92afad9.kv $PWD/inat-c92-r2.txt > inat-c92-r2.log 2>&1
DIAG_SETS=none DIAG_MODES=$M DIAG_MISS=1 ./run_study.sh laion-clip-512-normalized $PWD/costs-laion-92afad9.kv $PWD/laion-c92-r2.txt > laion-c92-r2.log 2>&1
DIAG_SETS=none DIAG_MODES=$M DIAG_MISS=1 ./run_study.sh arxiv-nomic-768-normalized $PWD/costs-arxiv-6f86a1c.kv $PWD/arxiv-c6f-r2.txt > arxiv-c6f-r2.log 2>&1
DIAG_SETS=none DIAG_MODES=$M DIAG_MISS=1 ./run_study.sh yahoo-minilm-384-normalized $PWD/costs-yahoo-6f86a1c.kv $PWD/yahoo-c6f-r2.txt > yahoo-c6f-r2.log 2>&1
