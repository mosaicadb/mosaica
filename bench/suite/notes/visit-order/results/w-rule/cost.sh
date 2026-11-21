#!/bin/bash
# cost.sh TAG: study_w2_cost on iNaturalist and LAION (hard-linked copies of the suite's fits, rebuilt on cores 0-7 at
# nice 19 when stale; the timing pinned to core 11), appended to cost-TAG.txt; deletes the copies.
set -uo pipefail
TAG=$1; S=/home/joe/code/vexact-data/suite; C=/home/joe/code/vexact-data/wrule/cost
R=/home/joe/code/vexact-wrule/bench/suite/notes/visit-order/results/w-rule
BIN=$(ls -t /home/joe/code/vexact-visitorder-env/target-wrule/release/deps/vexact-* | grep -v "\.d$" | head -1)
mkdir -p $C
for spec in "inaturalist-resnet-2048-cosine inat" "laion-clip-512-normalized laion"; do
  set -- $spec
  I=$C/$2; rm -rf $I; cp -al $S/fit/$1 $I
  # build first (on 0-7), then time alone on core 11
  (cd /home/joe/code/vexact-wrule/rust && env RAYON_NUM_THREADS=8 DIAG_DIR=$I DIAG_PREP=$S/prep/$1 DIAG_M=1 DIAG_REPS=1 nice -n 19 taskset -c 0-7 $BIN --ignored --exact v2::e2e::study_w2_cost > /dev/null 2>&1)
  (cd /home/joe/code/vexact-wrule/rust && env RAYON_NUM_THREADS=1 DIAG_DIR=$I DIAG_PREP=$S/prep/$1 taskset -c 11 $BIN --ignored --nocapture --exact v2::e2e::study_w2_cost 2>&1 | grep -E "per call|panicked") | sed "s#$C/##" >> $R/cost-$TAG.txt
  rm -rf $I
done
rmdir $C 2>/dev/null; cat $R/cost-$TAG.txt
