#!/bin/bash
# u_check.sh: study_w_unit on hard-linked copies of the suite's iNaturalist, LAION and yahoo fits (rebuilt by this
# branch), the 100 benchmark queries, first 20 clusters each; cores 0-7 at nice 19; then u_check.py. Deletes the copies.
set -uo pipefail
S=/home/joe/code/vexact-data/suite; C=/home/joe/code/vexact-data/wrule/val
R=/home/joe/code/vexact-wrule/bench/suite/notes/visit-order/results/w-rule
BIN=$(ls -t /home/joe/code/vexact-visitorder-env/target-wrule/release/deps/vexact-* | grep -v "\.d$" | head -1)
mkdir -p $C
for spec in "inaturalist-resnet-2048-cosine inat" "laion-clip-512-normalized laion" "yahoo-minilm-384-normalized yahoo"; do
  set -- $spec
  I=$C/$2; rm -rf $I; cp -al $S/fit/$1 $I
  (cd /home/joe/code/vexact-wrule/rust && env RAYON_NUM_THREADS=8 DIAG_DIR=$I DIAG_PREP=$S/prep/$1 DIAG_OUT=$C/u-$2.txt DIAG_M=20 nice -n 19 taskset -c 0-7 $BIN --ignored --nocapture --exact v2::e2e::study_w_unit 2>&1 | grep -E "panicked|test result")
  rm -rf $I
done
/home/joe/code/vexact-visitorder-env/b99b033c/bin/python -I $R/u_check.py $C/u-inat.txt $C/u-laion.txt $C/u-yahoo.txt
