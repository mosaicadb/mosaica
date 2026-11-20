#!/bin/bash
# Dump one dataset and visible set with the study `study_layered_dump` (rust/src/v2/e2e.rs), for cascade.py.
# usage: dump.sh DS(glove|yahoo|laion) SET(none|random-25|...) M [LEVELS]
#   DS_fit: the same fit built with --dlv fit (six deterministic levels), in ix/DS_fit.
#   SCRATCH (default /home/user/scratch-layered) holds the test binary's target directory, the index (ix/DS: the
#   suite's fit, symlinked, rebuilt with this tree's library) and the dumps (dump/DS_SET.*).
set -euo pipefail
S=${SCRATCH:-/home/user/scratch-layered}
case ${1%_fit} in glove) N=glove-100-angular;; yahoo) N=yahoo-minilm-384-normalized;; laion) N=laion-clip-512-normalized;; esac
BIN=${BIN:-$(grep -l "study_layered_dump" "$S"/target/release/deps/vexact-* | grep -v "\.d$" | head -1)}
cd "$(dirname "$0")/../../../../../../rust"
RAYON_NUM_THREADS=3 DIAG_DIR=$S/ix/$1 DIAG_PREP=/home/user/suite-data/prep/$N DIAG_SET=$2 DIAG_OUT=$S/dump/$1_$2 DIAG_M=$3 DIAG_LEVELS=${4:-3} \
  nice taskset -c 0-2 "$BIN" study_layered_dump --ignored --nocapture 2>&1 | grep -v "^  \|^$\|^running\|^test "
