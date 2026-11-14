#!/bin/bash
# Stage 1b re-measurement: every timing on one CPU (default 2), one job at a time; preparation on CPUs 0-7. Resumable:
# each finished piece leaves a marker in $DATA/<name>/results2/done/.
# usage: bash bench/remeasure.sh [CORE]
set -uo pipefail
CORE=${1:-2}
HERE=$(cd "$(dirname "$0")" && pwd)
DATA=${DATA:-/home/joe/code/vexact-data}
PY="$HERE/.venv/bin/python3 -I"
B=${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}/release/vexact
piece() {  # piece NAME DATASET COMMAND...
  local name=$1 d=$2; shift 2
  local m=$DATA/$d/results2/done/$name
  [ -f "$m" ] && return 0
  mkdir -p "$(dirname "$m")"
  echo "[$(date +%T)] $d $name" >&2
  "$@" > "$DATA/$d/results2/log_$name.txt" 2>&1 && touch "$m" || echo "FAILED $d $name" >&2
}
for d in glove-100-angular nytimes-256-angular; do
  D=$DATA/$d; R=$D/results2; BL=$D/baselines
  F=$(ls "$D"/filters/vis_*.npy | tr '\n' ',' | sed 's/,$//')
  if [ $d = glove-100-angular ]; then
    G=8:0,16:0,32:0,64:0,128:0,256:0,512:0,128:0.01,256:0.01,16:0:soar,32:0:soar,64:0:soar,128:0:soar
  else
    G=2:0,4:0,8:0,16:0,32:0,64:0,128:0,32:0.01,64:0.01,8:0:soar,16:0:soar,32:0:soar,64:0:soar
  fi
  O="--v2 --index $D/index --queries $D/test.npy --results $R --name $d --core $CORE --reps 2"
  piece truth $d taskset -c 0-7 $B --v2 --index $D/index --queries $D/test.npy --results $R --filters "$F" ann-truth
  piece calib $d taskset -c $CORE $B $O --nq 1 --methods none ann
  P="--plan-file $R/plan.txt"
  piece exact_all $d taskset -c $CORE $B $O $P --methods exact ann
  piece flat_all $d taskset -c $CORE $B $O $P --flat --methods flat ann
  piece filtered $d taskset -c $CORE $B $O $P --filters "$F" --no-all --flat --methods exact,flat ann
  piece approx $d taskset -c $CORE $B $O $P --fit-dial --soar --dial $G --methods none ann
  piece score $d $PY "$HERE/score.py" "$D" "$R"
  piece oracle $d taskset -c 0-7 $PY "$HERE/oracle.py" "$D" "$R"
  piece flat_b $d taskset -c $CORE $PY "$HERE/baselines.py" query "$D" "$BL" faiss-flat --reps 2
  for lib in hnswlib faiss-hnsw; do
    for m in 4 8 12 16 24 36 48 64 96; do
      piece ${lib}_M-$m $d taskset -c $CORE $PY "$HERE/baselines.py" query "$D" "$BL" $lib --reps 2 --builds M-$m
    done
  done
  for nl in 512 1204 2048 4096; do
    piece ivf_$nl $d taskset -c $CORE $PY "$HERE/baselines.py" query "$D" "$BL" faiss-ivfpqfs --reps 2 --builds nlist-$nl
  done
  piece flat_b_filtered $d taskset -c $CORE $PY "$HERE/baselines.py" query "$D" "$BL" faiss-flat --reps 2 --filtered flat
  for fr in 0.5 0.1 0.01 0.001; do
    piece hnsw_f$fr $d taskset -c $CORE $PY "$HERE/baselines.py" query "$D" "$BL" faiss-hnsw --reps 2 --filtered M-16 --nq 2000 \
      --fracs $fr --efs 10,40,120,400,1000,2000,5000,10000
    piece ivf_f$fr $d taskset -c $CORE $PY "$HERE/baselines.py" query "$D" "$BL" faiss-ivfpqfs --reps 2 --filtered nlist-2048 \
      --nq 2000 --fracs $fr --nprobes 1,10,50,200,500,1000,2048
  done
done
echo "[$(date +%T)] all done" >&2
