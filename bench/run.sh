#!/bin/bash
# The whole ANN-Benchmarks run for one dataset, in order. Data goes to $DATA/<name>/ (outside the tree); the HDF5 file
# is an untrusted download, read only by python3 -I.
# usage: bash bench/run.sh NAME [CORE]   (NAME: glove-100-angular | nytimes-256-angular; CORE: the timing CPU, default 6)
set -euo pipefail
NAME=$1
CORE=${2:-6}
HERE=$(cd "$(dirname "$0")" && pwd)
DATA=${DATA:-/home/joe/code/vexact-data}
D=$DATA/$NAME
PY="$HERE/.venv/bin/python3 -I"
B=${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}/release/vexact
# the approximate mode's settings, np:eps (clusters probed : the dial's miss rate per level, 0 off)
DIAL=${DIAL:-1:0,2:0,4:0,8:0,16:0,32:0,64:0,128:0,256:0,8:0.01,16:0.01,32:0.01,64:0.01,128:0.01}

mkdir -p "$D"
[ -f "$D/$NAME.hdf5" ] || curl -sSfL -o "$D/$NAME.hdf5" "https://ann-benchmarks.com/$NAME.hdf5"
[ -f "$D/train.npy" ] || $PY "$HERE/convert.py" "$D/$NAME.hdf5"
[ -f "$D/filters/vis_0.001.npy" ] || $PY "$HERE/filters.py" "$D"
[ -f "$D/index/meta.json" ] || $PY "$HERE/../rust/tools/fit.py" "$D/train.npy" "$D/index" --metric angular

F=$(ls "$D"/filters/vis_*.npy | tr '\n' ',' | sed 's/,$//')
R=$D/results
$B --v2 --index "$D/index" --queries "$D/test.npy" --results "$R" --filters "$F" ann-truth
$B --v2 --index "$D/index" --queries "$D/test.npy" --results "$R" --filters "$F" --name "$NAME" --core "$CORE" --reps 2 --flat --dial "$DIAL" ann
$PY "$HERE/score.py" "$D" "$R"

for lib in hnswlib faiss-hnsw faiss-ivfpqfs; do
  taskset -c 0-7 $PY "$HERE/baselines.py" build "$D" "$D/baselines" $lib --threads 8
done
for lib in faiss-flat hnswlib faiss-hnsw faiss-ivfpqfs; do
  taskset -c "$CORE" $PY "$HERE/baselines.py" query "$D" "$D/baselines" $lib --reps 2
done
# filtered: brute force over the visible rows, and grids that reach ef ~ k / selectivity and nprobe = nlist, on the
# first 2,000 queries. hnswlib is left out: its Python bindings filter through a Python callback per visited node,
# which would be timed instead of the index.
taskset -c "$CORE" $PY "$HERE/baselines.py" query "$D" "$D/baselines" faiss-flat --reps 2 --filtered flat
taskset -c "$CORE" $PY "$HERE/baselines.py" query "$D" "$D/baselines" faiss-hnsw --reps 2 --filtered M-16 --nq 2000 \
  --efs 10,40,120,400,1000,2000,5000,10000
taskset -c "$CORE" $PY "$HERE/baselines.py" query "$D" "$D/baselines" faiss-ivfpqfs --reps 2 --filtered nlist-2048 --nq 2000 \
  --nprobes 1,10,50,200,500,1000,2048
