#!/bin/bash
# the first round: iNaturalist's posterior at psi 5 and its as-built constants; LAION, yahoo and arxiv as built and
# centre-corrected, unfiltered and region-25, with misses classed
set -x
DIAG_SETS=none DIAG_MODES=corr DIAG_PSIS=5 DIAG_POST=5 ./run_study.sh inaturalist-resnet-2048-cosine $PWD/costs-inat-92afad9.kv $PWD/inat-post.txt > inat-post.log 2>&1
DIAG_SETS=none DIAG_MODES=built,corr DIAG_MISS=1 ./run_study.sh inaturalist-resnet-2048-cosine $PWD/costs-inat-6f86a1c.kv $PWD/inat-c6f.txt > inat-c6f.log 2>&1
DIAG_SETS=none,region-25 DIAG_MODES=built,corr DIAG_MISS=1 DIAG_POST=5 ./run_study.sh laion-clip-512-normalized $PWD/costs-laion-92afad9.kv $PWD/laion-c92.txt > laion-c92.log 2>&1
DIAG_SETS=none,region-25 DIAG_MODES=built,corr DIAG_MISS=1 ./run_study.sh yahoo-minilm-384-normalized $PWD/costs-yahoo-6f86a1c.kv $PWD/yahoo-c6f.txt > yahoo-c6f.log 2>&1
DIAG_SETS=none,region-25 DIAG_MODES=built,corr DIAG_MISS=1 ./run_study.sh arxiv-nomic-768-normalized $PWD/costs-arxiv-6f86a1c.kv $PWD/arxiv-c6f.txt > arxiv-c6f.log 2>&1
DIAG_SETS=region-25 DIAG_MODES=built,corr DIAG_MISS=1 ./run_study.sh inaturalist-resnet-2048-cosine $PWD/costs-inat-92afad9.kv $PWD/inat-c92-r25.txt > inat-c92-r25.log 2>&1
