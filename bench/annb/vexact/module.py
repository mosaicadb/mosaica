# The fast-scan index (rust/, through bench/annb/vexact-py) as an ANN-Benchmarks and VIBE algorithm. fit() runs
# rust/tools/fit.py on the training vectors and builds the index; queries run one at a time on one thread.
# VEXACT_SRC is where the image keeps the sources (rust/ and combined/); VEXACT_WORK is where fit() writes.
import os
import subprocess
import sys
import tempfile

import numpy as np
import vexact_py

from ..base.module import BaseANN

SRC = os.environ.get("VEXACT_SRC", "/home/app/vexact")
# The index's randomised levels need at least 50 dimensions; narrower vectors get zero columns, which keep every cosine.
MIN_DIM = 64


class Vexact(BaseANN):
    def __init__(self, metric, method_param=None):
        if metric not in ("angular", "cosine", "normalized"):
            raise ValueError(f"vexact supports cosine similarity only (angular, cosine, normalized), not {metric}")
        self.method_param = method_param
        self.index = None

    def fit(self, X):
        work = tempfile.mkdtemp(prefix="vexact-", dir=os.environ.get("VEXACT_WORK"))
        train = os.path.join(work, "train.npy")
        d = max(X.shape[1], MIN_DIM)
        self.qbuf = np.zeros(d, np.float32)
        Xp = np.zeros((X.shape[0], d), np.float32)
        Xp[:, : X.shape[1]] = X
        np.save(train, Xp)
        del Xp
        fit = os.path.join(SRC, "rust", "tools", "fit.py")
        subprocess.run([sys.executable, fit, train, os.path.join(work, "index"), "--metric", "angular"], check=True)
        os.remove(train)
        self.index = vexact_py.Index(os.path.join(work, "index"))

    def q(self, v):
        self.qbuf[: len(v)] = v
        return self.qbuf


class VexactExact(Vexact):
    def __init__(self, metric, method_param=None):
        super().__init__(metric, method_param)
        self.name = "vexact-exact"

    def set_query_arguments(self, mode="exact"):
        self.name = f"vexact-exact (mode={mode})"

    def query(self, v, n):
        return self.index.knn(self.q(v), n)


class VexactApprox(Vexact):
    def __init__(self, metric, method_param=None):
        super().__init__(metric, method_param)
        self.nprobe, self.eps = 1, 0.0

    def set_query_arguments(self, nprobe, eps):
        self.nprobe, self.eps = nprobe, eps
        if eps > 0:
            self.index.fit_dial(10)
        self.name = f"vexact-approx (nprobe={nprobe}, eps={eps})"

    def query(self, v, n):
        return self.index.knn_approx(self.q(v), n, self.nprobe, self.eps)
