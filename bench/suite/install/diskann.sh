#!/bin/bash
# Build DiskANN's SSD index from microsoft/DiskANN's C++ release 0.7.0 (the code diskannpy 0.7.0 wraps; the
# repository's main branch is now a Rust rewrite), with the driver ../native/diskann_bench.cpp, which times each search
# inside the process (as ../../threshold-db/acorn_bench.cpp does) and runs on the Pythons the wheel does not support.
# DiskANN's CMake requires Intel MKL (2.2 GB as Ubuntu packages); the library uses only cblas_sgemm, cblas_snrm2,
# LAPACKE_sgesdd and mkl_set_num_threads, so it is compiled here directly against OpenBLAS and LAPACKE through the
# stand-in header ../native/mkl.h. Point the suite at the binary with DISKANN_BENCH=DIR/diskann_bench.
# usage: bash bench/suite/install/diskann.sh DIR
#   Needs g++ (C++17, OpenMP) and: libaio-dev libboost-program-options-dev libopenblas-dev liblapacke-dev
#   libgoogle-perftools-dev (apt). About 2 minutes on 4 cores. (-include cstdint: GCC 13 no longer pulls it in
#   where DiskANN 0.7.0 relies on it.)
set -euo pipefail
DIR=$1
HERE=$(cd "$(dirname "$0")" && pwd)
NATIVE=$HERE/../native
[ -e "$DIR/.git" ] || git clone https://github.com/microsoft/DiskANN.git "$DIR"
cd "$DIR"
git checkout -q df225d32da81db87596a3ce7c040ad0f78709a7c   # tag 0.7.0
SRC="abstract_data_store ann_exception disk_utils distance index in_mem_graph_store in_mem_data_store
  linux_aligned_file_reader math_utils natural_number_map natural_number_set memory_mapper partition pq pq_flash_index
  scratch logger utils filter_utils index_factory abstract_index pq_l2_distance pq_data_store"
FLAGS="-std=c++17 -include cstdint -O3 -DNDEBUG -march=native -mavx2 -mfma -msse2 -ftree-vectorize -fopenmp -fopenmp-simd -funroll-loops -DUSE_AVX2"
mkdir -p build-openblas
for s in $SRC; do
  echo "g++ $FLAGS -I$NATIVE -Iinclude -c src/$s.cpp -o build-openblas/$s.o"
done | xargs -P "$(nproc)" -I{} sh -c "{}"
g++ $FLAGS -I"$NATIVE" -Iinclude "$NATIVE/diskann_bench.cpp" build-openblas/*.o -o diskann_bench \
  -lopenblas -llapacke -laio -ltcmalloc -lboost_program_options
echo "built $DIR/diskann_bench"
