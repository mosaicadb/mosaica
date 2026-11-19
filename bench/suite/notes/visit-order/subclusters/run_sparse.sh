#!/bin/bash
# The 1% visible sets again, after `knn_sub` kept its priced clusters' projections in flat arrays instead of a hash map
# (its fixed work per priced cluster, which the 1% sets' short queries feel): rebuilds the scratch indexes with the
# current wheel and runs ab_sub.py on random-1 and region-1 into results/DATASET/ab_sub_sparse.txt.
# usage: bash run_sparse.sh DATASET
set -euo pipefail
DS=$1
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$HERE/results/$DS
bash "$HERE/scratch.sh" "$DS" >/dev/null
DEF=/home/user/scratch-sub/def/$DS
C=/home/user/scratch-sub/c/$DS
taskset -c 0-2 python3 -I "$HERE/build.py" "$DS" 2>&1 | grep --line-buffered -v '^\[build2\]' > "$OUT/build_sparse.txt"
export SUITE_DATA=/home/user/suite-data
taskset -c 3 python3 -I "$HERE/ab_sub.py" "$DS" "$DEF" "$C" --reps 3 --psi 2.5,3,3.5,4,4.5,5,5.5,6,7 --filters random-1,region-1 \
  --arms adaptive,adaptive-c,sub-a-filter,sub-c-filter,sub-c-own-filter 2>&1 | grep --line-buffered -v '^\[' > "$OUT/ab_sub_sparse.txt"
python3 -I "$HERE/../scripts/ab_curves.py" "$OUT/ab_sub_sparse.txt" --targets 0.8,0.9,0.95,0.99 > "$OUT/ab_sub_sparse_curves.txt"
rm -rf "$DEF" "$C"
echo "done $DS"
