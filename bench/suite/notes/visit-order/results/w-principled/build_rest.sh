#!/bin/bash
# builds the other datasets' indexes (one query, one psi) once the iNaturalist run (pid 593750) ends
while kill -0 593750 2>/dev/null; do sleep 5; done
for ds in laion-clip-512-normalized yahoo-minilm-384-normalized arxiv-nomic-768-normalized; do
  DIAG_NQ=1 DIAG_PSIS=2 DIAG_MODES=corr DIAG_THREADS=1 ./run_study.sh $ds $PWD/costs-laion-92afad9.kv /tmp/claude-1000/build-$ds.txt > build-$ds.log 2>&1
done
