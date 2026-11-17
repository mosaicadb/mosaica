// A stand-in for Intel MKL's header when DiskANN (C++, tag 0.7.0) is built against OpenBLAS and LAPACKE: DiskANN uses
// only cblas_sgemm, cblas_snrm2, LAPACKE_sgesdd (OPQ) and mkl_set_num_threads. Built without MKL_ILP64, so MKL_INT is
// the 32-bit int of OpenBLAS's LP64 interface.
#pragma once
#include <cblas.h>
#include <lapacke.h>
typedef int MKL_INT;
static inline void mkl_set_num_threads(int n) { openblas_set_num_threads(n); }
