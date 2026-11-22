#!/bin/bash
# build_wheel.sh REV VENV: the bindings from the worktree's HEAD against rust/ and combined/ at REV, into VENV
set -euo pipefail
REV=$1; V=$2; W=/home/joe/code/vexact-visitorder; E=/home/joe/code/vexact-visitorder-env
T=$E/src-$REV; rm -rf $T; mkdir -p $T/bench/annb
git -C $W archive $REV rust/build.rs rust/src rust/tools rust/Cargo.toml rust/Cargo.lock combined/core.py | tar -x -C $T
git -C $W archive ${BIND:-HEAD} bench/annb/vexact-py | tar -x -C $T
cd $T/bench/annb/vexact-py
CARGO_TARGET_DIR=$E/target-$REV RUSTFLAGS="-C target-cpu=native" nice -n 19 taskset -c 0-3 $E/base/bin/maturin build --release -o $E/wheels-$REV 2>&1 | tail -2
$V/bin/pip -q install --force-reinstall --no-deps $E/wheels-$REV/vexact_py-*.whl
echo "$REV" > $V/REV
