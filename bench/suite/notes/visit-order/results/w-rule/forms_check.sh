#!/bin/bash
# forms_check.sh: per index (iNaturalist, LAION, yahoo), a hard-linked copy of the suite's fit rebuilt by this branch's
# study_w2_forms (the build's time for the level ratios and the W2 forms in its log), the stored forms' tau^2 and
# gamma against the replay's study_w_rule output on the 100 benchmark queries (w-DS-test.txt), and the forms' storage;
# cores 0-7 at nice 19, nothing else running. Deletes each copy after.
set -uo pipefail
S=/home/joe/code/vexact-data/suite; W=/home/joe/code/vexact-data/sampled/dump; C=/home/joe/code/vexact-data/wrule/val
R=/home/joe/code/vexact-wrule/bench/suite/notes/visit-order/results/w-rule
BIN=$(ls -t /home/joe/code/vexact-visitorder-env/target-wrule/release/deps/vexact-* | grep -v "\.d$" | head -1)
mkdir -p $C
for spec in "inaturalist-resnet-2048-cosine inat" "laion-clip-512-normalized laion" "yahoo-minilm-384-normalized yahoo"; do
  set -- $spec
  I=$C/$2; rm -rf $I; cp -al $S/fit/$1 $I
  (cd /home/joe/code/vexact-wrule/rust && env RAYON_NUM_THREADS=8 DIAG_DIR=$I DIAG_PREP=$S/prep/$1 DIAG_REF=$W/w-$2-test.txt nice -n 19 taskset -c 0-7 $BIN --ignored --nocapture --exact v2::e2e::study_w2_forms 2>&1 | grep -E "W2 forms|level ratios|assembled|lines of|storage|panicked|test result")
  du -b $I/encoded/wrule_h.bin $I/encoded/wrule_c.bin
  rm -rf $I
done
