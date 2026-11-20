#!/bin/bash
# The library's own reader with the levels' half-widths at psi rho (`study_layered_kernel`, rust/src/v2/e2e.rs):
# counts only. usage: kernel.sh DS(glove|yahoo|laion) SET(none|random-25|...) RHO(comma separated, per level) [PSIS]
set -euo pipefail
S=${SCRATCH:-/home/user/scratch-layered}
case ${1%_fit} in glove) N=glove-100-angular;; yahoo) N=yahoo-minilm-384-normalized;; laion) N=laion-clip-512-normalized;; esac
BIN=${BIN:-$(grep -l "study_layered_kernel" "$S"/target/release/deps/vexact-* | grep -v "\.d$" | head -1)}
cd "$(dirname "$0")/../../../../../../rust"
RAYON_NUM_THREADS=3 DIAG_DIR=$S/ix/$1 DIAG_PREP=/home/user/suite-data/prep/$N DIAG_SET=$2 DIAG_RHO=$3 DIAG_PSIS=${4:-3,4,5,6,7,8} \
  nice taskset -c 0-2 "$BIN" study_layered_kernel --ignored --nocapture 2>&1 | grep -v "^$\|^running\|^test \|^successes\|^failures"
