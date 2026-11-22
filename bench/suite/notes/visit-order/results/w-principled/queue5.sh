#!/bin/bash
# the library's knob against the study rule it implements, and the oracle cross-form rule, on the four datasets
# (each index rebuilt first, since the library changed)
set -x
PS=$(python3 -c "print(','.join(f'{2+0.25*i:g}' for i in range(29)))")
run() { # DS COSTS TAG
  DIAG_PSIS=3,4,5,6,7 DIAG_SETS=none,region-25 DIAG_MODES=lib:w2,corr,lib:height,height ./run_study.sh $1 $PWD/$2 $PWD/lib-$3.txt > lib-$3.log 2>&1
  DIAG_PSIS=$PS DIAG_SETS=none DIAG_MODES=cross DIAG_MISS=1 ./run_study.sh $1 $PWD/$2 $PWD/cross-$3.txt > cross-$3.log 2>&1
}
run inaturalist-resnet-2048-cosine costs-inat-92afad9.kv inat
run laion-clip-512-normalized costs-laion-92afad9.kv laion
run arxiv-nomic-768-normalized costs-arxiv-6f86a1c.kv arxiv
run yahoo-minilm-384-normalized costs-yahoo-6f86a1c.kv yahoo
DIAG_SETS=none DIAG_MODES=built ./run_study.sh laion-clip-512-normalized $PWD/costs-laion-6f86a1c.kv $PWD/laion-c6f-all.txt > laion-c6f-all.log 2>&1
