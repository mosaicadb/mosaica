#!/bin/bash
# All three questions on one dataset, timing on core 3 (nothing else runs meanwhile): the scratch indexes (scratch.sh,
# build.py on cores 0-2), then read_costs.py, ab_exact.py and ab_sub.py, each into results/DATASET/.
# usage: bash run_ds.sh DATASET
set -euo pipefail
DS=$1
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$HERE/results/$DS
mkdir -p "$OUT"
bash "$HERE/scratch.sh" "$DS" >/dev/null
DEF=/home/user/scratch-sub/def/$DS
C=/home/user/scratch-sub/c/$DS
taskset -c 0-2 python3 -I "$HERE/build.py" "$DS" 2>&1 | grep --line-buffered -v '^\[build2\]' > "$OUT/build.txt"
export SUITE_DATA=/home/user/suite-data
taskset -c 3 python3 -I "$HERE/read_costs.py" "$DS" "$DEF" "$C" 2>&1 | grep --line-buffered -v '^\[' > "$OUT/read_costs.txt"
taskset -c 3 python3 -I "$HERE/ab_exact.py" "$DS" "$DEF" "$C" --reps 5 2>&1 | grep --line-buffered -v '^\[' > "$OUT/ab_exact.txt"
taskset -c 3 python3 -I "$HERE/ab_sub.py" "$DS" "$DEF" "$C" --reps 3 --psi ${PSI:-2.5,3,3.5,4,4.5,5,5.5,6,7} \
  --arms spread,adaptive,adaptive-c,sub-a-filter,sub-b-filter,sub-c-filter,sub-c,sub-c-own-filter 2>&1 | grep --line-buffered -v '^\[' > "$OUT/ab_sub.txt"
python3 -I "$HERE/../scripts/ab_curves.py" "$OUT/ab_sub.txt" --targets 0.8,0.9,0.95,0.99 > "$OUT/ab_sub_curves.txt"
echo "done $DS"
