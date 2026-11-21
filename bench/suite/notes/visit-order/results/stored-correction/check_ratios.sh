#!/bin/bash
# check_ratios.sh: per index (iNaturalist, yahoo, LAION), a hard-linked copy of the suite's fit rebuilt by this branch's
# study_level_ratios (the build's level ratios and their time in the log), the ratios from the replay's own training
# queries, and check_ratios.py against the replay's training dump; cores 0-7 at nice 19. Deletes each copy after.
set -uo pipefail
S=/home/joe/code/vexact-data/suite; W=/home/joe/code/vexact-data/sampled; C=/home/joe/code/vexact-data/corr
R=/home/joe/code/vexact-corr/bench/suite/notes/visit-order/results/stored-correction
BIN=/home/joe/code/vexact-visitorder-env/target-corr/release/deps/vexact-d231cc53a1400794
mkdir -p $C/val
for spec in "inaturalist-resnet-2048-cosine inat" "yahoo-minilm-384-normalized yahoo" "laion-clip-512-normalized laion"; do
  set -- $spec
  I=$C/val/$2; rm -rf $I; cp -al $S/fit/$1 $I
  (cd /home/joe/code/vexact-corr/rust && env RAYON_NUM_THREADS=8 DIAG_DIR=$I DIAG_PREP=$W/prep/$1-data DIAG_OUT=$C/val/$2 nice -n 19 taskset -c 0-7 $BIN --ignored --nocapture --exact v2::e2e::study_level_ratios 2>&1 | grep -E "level ratios|reading the ratios|build2\] (saved|assembled)|panicked|test result") > $R/ratios-$2.log
  python3 -I $R/check_ratios.py $2 $W/dump/$2-data $W/prep/$1-data/query_rows.npy $C/val/$2 >> $R/ratios-$2.log 2>&1
  rm -rf $I
done
cat $R/ratios-*.log
