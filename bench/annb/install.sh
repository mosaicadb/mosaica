#!/bin/bash
# Add the fast-scan index to an ANN-Benchmarks checkout as the algorithm `vexact` and build its image.
# usage: bash bench/annb/install.sh ANN_BENCHMARKS_DIR [REV]
#   rust/ and combined/ come from REV (default: the pinned revision below), so work on rust/ does not change what is
#   benchmarked until the pin moves; the bindings (vexact-py) and the algorithm files come from this working tree.
#   The image is built with -C target-cpu=native: build it on the machine that runs the benchmark.
set -euo pipefail
PIN=03e81f70bf2eebd2db9d6353dddc94e7e669d751
AB=$(cd "$1" && pwd)
REV=${2:-$PIN}
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(git -C "$HERE" rev-parse --show-toplevel)
ALG=$AB/ann_benchmarks/algorithms/vexact
[ -f "$AB/install.py" ] || { echo "$AB is not an ANN-Benchmarks checkout (no install.py)" >&2; exit 2; }
rm -rf "$ALG"
mkdir -p "$ALG/src/bench/annb"
cp "$HERE"/vexact/{__init__.py,module.py,config.yml,Dockerfile} "$ALG/"
git -C "$ROOT" archive "$REV" rust/src rust/tools rust/Cargo.toml rust/Cargo.lock combined/core.py | tar -x -C "$ALG/src"
cp -r "$HERE/vexact-py" "$ALG/src/bench/annb/"
echo "$REV" > "$ALG/src/REV"
cd "$AB"
docker image inspect ann-benchmarks >/dev/null 2>&1 || docker build --rm -t ann-benchmarks -f ann_benchmarks/algorithms/base/Dockerfile .
docker build --rm -t ann-benchmarks-vexact -f ann_benchmarks/algorithms/vexact/Dockerfile .
