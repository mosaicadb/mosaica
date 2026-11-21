#!/bin/bash
# alr_check.sh: W2 as built against W2 with the centre's term taken off (study_w2_alr), per index (iNaturalist, LAION,
# yahoo) on hard-linked copies of the suite's fits rebuilt by this branch, then the sample-when-needed replay (its
# replay.py and table.py, from commit 5dd425a9b) with both as its "W1" (as built) and "W2" (centre off) columns: rows
# past the deterministic levels at 0.9 and 0.95 kept. Cores 0-7 at nice 19. Deletes the copies.
set -uo pipefail
S=/home/joe/code/vexact-data/suite; W=/home/joe/code/vexact-data/sampled; C=/home/joe/code/vexact-data/wrule/alr
R=/home/joe/code/vexact-wrule/bench/suite/notes/visit-order/results/w-rule
BIN=$(ls -t /home/joe/code/vexact-visitorder-env/target-wrule/release/deps/vexact-* | grep -v "\.d$" | head -1)
PY=/home/joe/code/vexact-visitorder-env/b99b033c/bin/python
mkdir -p $C
git -C /home/joe/code/vexact-wrule show 5dd425a9b:bench/suite/notes/visit-order/results/sample-when-needed/replay.py > $C/replay.py
git -C /home/joe/code/vexact-wrule show 5dd425a9b:bench/suite/notes/visit-order/results/sample-when-needed/table.py > $C/table.py
for spec in "inaturalist-resnet-2048-cosine inat" "laion-clip-512-normalized laion" "yahoo-minilm-384-normalized yahoo"; do
  set -- $spec
  I=$C/$2; rm -rf $I; cp -al $S/fit/$1 $I
  (cd /home/joe/code/vexact-wrule/rust && env RAYON_NUM_THREADS=8 DIAG_DIR=$I DIAG_PREP=$S/prep/$1 DIAG_REF=$W/dump/w-$2-test.txt DIAG_OUT=$C/w-$2.txt nice -n 19 taskset -c 0-7 $BIN --ignored --nocapture --exact v2::e2e::study_w2_alr 2>&1 | grep -E "level|panicked|test result")
  rm -rf $I
  nice -n 19 taskset -c 0-7 $PY -I $C/replay.py $2 $S/rand-dump/$2 $S/prep/$1 $W/dump/$2-data $W/prep/$1-data/query_rows.npy $W/dump/$2-real --costs $2 --w $C/w-$2.txt --json $C/replay-$2.json > $C/replay-$2.txt 2>&1
done
$PY -I $C/table.py $C/replay-inat.json $C/replay-laion.json $C/replay-yahoo.json --rules=W1,W2
