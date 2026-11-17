#!/bin/bash
# Build and install PDX with BOND (the `pdxearch` Python package) into the Python that runs the suite.
# usage: bash bench/suite/install/pdx.sh DIR [PYTHON]
#   DIR: where to clone github.com/cwida/PDX. BOND (exact pruned search, no index) left PDX's main branch in v0.3
#   (b87726a, "Bye BOND"); 91618e0 is the last revision with it and with filtered search. Needs clang++ (17 or later)
#   and cmake; FFTW is optional (BOND does not use it). About 20 s to build.
set -euo pipefail
DIR=$1
PY=${2:-python3}
REV=91618e01e574e594e27c71abfe3b1d5094657d53
[ -e "$DIR/.git" ] || git clone https://github.com/cwida/PDX.git "$DIR"
cd "$DIR"
git checkout -q "$REV"
git submodule update --init --depth 1 extern/pybind11 extern/Eigen
"$PY" -m pip install pybind11 setuptools wheel scipy
CXX=${CXX:-clang++} CC=${CC:-clang} "$PY" -m pip install --no-build-isolation --no-deps .
"$PY" -c "from pdxearch.compiled import IndexBONDFlat; print('pdxearch with BOND: ok')"
