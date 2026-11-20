#!/bin/bash
# The group count timed (layered.md 8.8 and 8.9): exact knn2 and the approximate k-NN at G groups, unfiltered and
# random-25, on yahoo and LAION (`study_layered_group_timing`, rust/src/v2/e2e.rs). Fits and encodes on cores 0-2,
# timing on core 3, one encode at a time, deleted after. RUN ONLY WHEN THE TIMING CORE IS FREE.
# usage: group_timing.sh "8 64" [NQ] [REPS]
#   SCRATCH (default /home/user/scratch-layered) holds the fits and target/ (cargo build --release and
#   cargo test --release --no-run with CARGO_TARGET_DIR=$SCRATCH/target, built beforehand).
set -u
S=${SCRATCH:-/home/user/scratch-layered}
W=$(cd "$(dirname "$0")/../../../../../.." && pwd)
R=$W/bench/suite/notes/visit-order/hierarchy/layered/results
BIN=$(grep -l "study_layered_group_timing" $S/target/release/deps/vexact-* | grep -v "\.d$" | head -1)
for ds in yahoo laion; do
  case $ds in yahoo) N=yahoo-minilm-384-normalized; QS="";; laion) N=laion-clip-512-normalized; QS="--query-sample /home/user/suite-data/prep/$N/learn.npy";; esac
  for G in $1; do
    F=$S/fit_${ds}_$G
    rm -rf "$F"
    OMP_NUM_THREADS=3 OPENBLAS_NUM_THREADS=3 nice taskset -c 0-2 python3 $W/rust/tools/fit.py /home/user/suite-data/prep/$N/corpus32.npy $F --metric angular --basis dual --groups $G $QS > $S/fit_${ds}_$G.log 2>&1
    (cd $W/rust && RAYON_NUM_THREADS=3 nice taskset -c 0-2 $S/target/release/vexact --v2 --index $F --rebuild > $S/enc_${ds}_$G.log 2>&1)
    for st in none random-25; do
      (cd $W/rust && RAYON_NUM_THREADS=1 DIAG_DIR=$F DIAG_PREP=/home/user/suite-data/prep/$N DIAG_SET=$st DIAG_NQ=${2:-50} DIAG_REPS=${3:-2} DIAG_PSIS=4,5,6 taskset -c 3 $BIN study_layered_group_timing --ignored --nocapture 2>&1 | grep -v "^$\|^running\|^test \|^successes\|^failures" > $R/timing_${ds}_G${G}_$st.txt)
    done
    rm -rf "$F"
  done
done
