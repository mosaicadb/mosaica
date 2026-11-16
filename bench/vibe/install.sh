#!/bin/bash
# Add the fast-scan index to a VIBE checkout as the algorithm `vexact` and build its Apptainer image.
# usage: bash bench/vibe/install.sh VIBE_DIR [REV]
#   rust/ and combined/ come from REV (default: the pin in bench/annb/install.sh); the bindings and module.py come
#   from this working tree and are shared with the ANN-Benchmarks integration. The image is built with
#   -C target-cpu=native: build it on the machine that runs the benchmark.
set -euo pipefail
PIN=$(sed -n 's/^PIN=//p' "$(dirname "$0")/../annb/install.sh")
VIBE=$(cd "$1" && pwd)
REV=${2:-$PIN}
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(git -C "$HERE" rev-parse --show-toplevel)
ALG=$VIBE/vibe/algorithms/vexact
[ -f "$VIBE/install.sh" ] && [ -d "$VIBE/vibe/algorithms" ] || { echo "$VIBE is not a VIBE checkout (no vibe/algorithms)" >&2; exit 2; }
rm -rf "$ALG"
mkdir -p "$ALG/src/bench/annb"
cp "$HERE/../annb/vexact/module.py" "$HERE/vexact/config.yml" "$ALG/"
touch "$ALG/__init__.py"
sed "s|@SRC@|$ALG/src|" "$HERE/vexact/image.def" > "$ALG/image.def"
git -C "$ROOT" archive "$REV" rust/build.rs rust/src rust/tools rust/Cargo.toml rust/Cargo.lock combined/core.py | tar -x -C "$ALG/src"
cp -r "$HERE/../annb/vexact-py" "$ALG/src/bench/annb/"
echo "$REV" > "$ALG/src/REV"
cd "$VIBE"
./install.sh --algorithm vexact --force
