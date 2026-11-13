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
APPROX=${APPROX:-1:100:5,2:100:5,4:100:5,8:100:5,16:100:5,32:100:5,64:100:5,128:100:5,256:100:5,1:1000:5,2:1000:5,4:1000:5,8:1000:5,16:1000:5,32:1000:5,64:1000:5,128:1000:5,256:1000:5}

mkdir -p "$D"
[ -f "$D/$NAME.hdf5" ] || curl -sSfL -o "$D/$NAME.hdf5" "https://ann-benchmarks.com/$NAME.hdf5"
[ -f "$D/train.npy" ] || $PY "$HERE/convert.py" "$D/$NAME.hdf5"
[ -f "$D/filters/vis_0.001.npy" ] || $PY "$HERE/filters.py" "$D"
[ -f "$D/index/meta.json" ] || $PY "$HERE/../rust/tools/fit.py" "$D/train.npy" "$D/index" --metric angular

F=$(ls "$D"/filters/vis_*.npy | tr '\n' ',' | sed 's/,$//')
R=$D/results
$B --v2 --index "$D/index" --queries "$D/test.npy" --results "$R" --filters "$F" ann-truth
$B --v2 --index "$D/index" --queries "$D/test.npy" --results "$R" --filters "$F" --name "$NAME" --core "$CORE" --reps 2 --flat --approx "$APPROX" ann
$PY "$HERE/score.py" "$D" "$R"

for lib in hnswlib faiss-hnsw faiss-ivfpqfs; do
  taskset -c 0-7 $PY "$HERE/baselines.py" build "$D" "$D/baselines" $lib --threads 8
done
for lib in hnswlib faiss-hnsw faiss-ivfpqfs; do
  taskset -c "$CORE" $PY "$HERE/baselines.py" query "$D" "$D/baselines" $lib --reps 2
done
taskset -c "$CORE" $PY "$HERE/baselines.py" query "$D" "$D/baselines" faiss-hnsw --reps 2 --filtered M-16
taskset -c "$CORE" $PY "$HERE/baselines.py" query "$D" "$D/baselines" faiss-ivfpqfs --reps 2 --filtered nlist-2048
# hnswlib filters through a Python callback per visited node: first 500 queries, four ef values
taskset -c "$CORE" $PY "$HERE/baselines.py" query "$D" "$D/baselines" hnswlib --reps 2 --filtered M-16 --nq 500 --efs 10,40,120,400
