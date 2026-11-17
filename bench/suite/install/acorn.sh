#!/bin/bash
# Build ACORN (stanford-futuredata/ACORN, a faiss 1.7.3 fork) with ../../threshold-db/acorn.patch, and its driver
# ../../threshold-db/acorn_bench.cpp, as ../../threshold-db/REPORT.md describes. Point the suite at the binary with
# ACORN_BENCH=DIR/acorn_bench.
# usage: bash bench/suite/install/acorn.sh DIR
#   Needs cmake, a C++17 compiler with OpenMP, and BLAS/LAPACK (the system's reference libraries do: only their .so.3
#   files are needed). About 2 minutes on 4 cores.
set -euo pipefail
DIR=$1
HERE=$(cd "$(dirname "$0")" && pwd)
TDB=$HERE/../../threshold-db
[ -e "$DIR/.git" ] || git clone https://github.com/stanford-futuredata/ACORN.git "$DIR"
cd "$DIR"
git checkout -q c259f11cdbec880671658eaa0d8747e2cc9de79d
git apply --check "$TDB/acorn.patch" 2>/dev/null && git apply "$TDB/acorn.patch"
BLAS=${BLAS:-$(ls /usr/lib/x86_64-linux-gnu/libblas.so* /usr/lib/x86_64-linux-gnu/libopenblas.so* 2>/dev/null | head -1)}
LAPACK=${LAPACK:-$(ls /usr/lib/x86_64-linux-gnu/liblapack.so* 2>/dev/null | head -1)}
mkdir -p build
cmake -S . -B build -DFAISS_ENABLE_GPU=OFF -DFAISS_ENABLE_PYTHON=OFF -DBUILD_TESTING=OFF -DFAISS_OPT_LEVEL=avx2 \
  -DCMAKE_SKIP_INSTALL_RULES=ON -DCMAKE_BUILD_TYPE=Release -DBLA_VENDOR=Generic \
  -DBLAS_LIBRARIES="$BLAS" -DLAPACK_LIBRARIES="$LAPACK" > build/cmake.log
make -C build -j"$(nproc)" faiss_avx2 > build/make.log
g++ -O3 -std=c++17 -mavx2 -mfma -mf16c -fopenmp -I. "$TDB/acorn_bench.cpp" build/faiss/libfaiss_avx2.a \
  "$BLAS" "$LAPACK" -o acorn_bench
echo "built $DIR/acorn_bench"
