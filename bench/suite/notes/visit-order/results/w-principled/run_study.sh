#!/bin/bash
# run_study.sh DS COSTS OUT [env...]: study_w_principled on the hard-linked fit copy of DS, cores 0-3 at nice 19
# (CORES overrides), with the constants file COSTS; DIAG_* from the environment.
set -euo pipefail
DS=$1; COSTS=$2; OUT=$3
BIN=/home/joe/code/vexact-visitorder-env/target-wfix/release/deps/vexact-d231cc53a1400794
cd /home/joe/code/vexact-wfix/rust
DIAG_DIR=/home/joe/code/vexact-data/wfix/$DS DIAG_PREP=/home/joe/code/vexact-data/suite/prep/$DS DIAG_COSTS=$COSTS DIAG_OUT=$OUT \
  taskset -c ${CORES:-0-3} nice -n 19 $BIN v2::e2e::study_w_principled --ignored --exact --nocapture
