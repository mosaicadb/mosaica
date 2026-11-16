# The fast-scan index (rust/, through bench/annb/vexact-py) as an ANN-Benchmarks and VIBE algorithm. fit() runs
# rust/tools/fit.py on the training vectors and builds the index; queries run one at a time on one thread.
# VEXACT_SRC is where the image keeps the sources (rust/ and combined/); VEXACT_WORK is where fit() writes.
# The build parameter `basis` (pca or dual) is fit.py's --basis. With dual on a dataset that supplies a sample of its
# queries (VIBE's out-of-distribution datasets, through fit_ood), the sample also shapes the basis (--query-sample).
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


def save_padded(path, X, d):
    Xp = np.zeros((X.shape[0], d), np.float32)
    Xp[:, : X.shape[1]] = X
    np.save(path, Xp)


class Vexact(BaseANN):
    def __init__(self, metric, method_param=None):
        if metric not in ("angular", "cosine", "normalized"):
            raise ValueError(f"vexact supports cosine similarity only (angular, cosine, normalized), not {metric}")
        # ANN-Benchmarks passes the build arguments as a dict, VIBE as values in order
        self.basis = method_param.get("basis", "pca") if isinstance(method_param, dict) else method_param or "pca"
        if self.basis not in ("pca", "dual"):
            raise ValueError(f"basis is pca or dual, not {self.basis}")
        self.index = None

    def fit(self, X):
        self.build(X, None)

    def fit_ood(self, X, X_learn, X_learn_neighbors):
        self.build(X, X_learn if self.basis == "dual" else None)

    def build(self, X, sample):
        work = tempfile.mkdtemp(prefix="vexact-", dir=os.environ.get("VEXACT_WORK"))
        d = max(X.shape[1], MIN_DIM)
        self.qbuf = np.zeros(d, np.float32)
        train = os.path.join(work, "train.npy")
        args = [sys.executable, os.path.join(SRC, "rust", "tools", "fit.py"), train, os.path.join(work, "index"),
                "--metric", "angular", "--basis", self.basis]
        save_padded(train, X, d)
        if sample is not None:
            args += ["--query-sample", os.path.join(work, "sample.npy")]
            save_padded(args[-1], sample, d)
        subprocess.run(args, check=True)
        for f in ("train.npy", "sample.npy"):
            if os.path.exists(os.path.join(work, f)):
                os.remove(os.path.join(work, f))
        self.index = vexact_py.Index(os.path.join(work, "index"))
        self.sampled = sample is not None

    def q(self, v):
        self.qbuf[: len(v)] = v
        return self.qbuf


class VexactExact(Vexact):
    def __init__(self, metric, method_param=None):
        super().__init__(metric, method_param)
        self.name = f"vexact-exact (basis={self.basis})"

    def set_query_arguments(self, mode="exact"):
        self.name = f"vexact-exact (basis={self.basis}, query sample={self.sampled}, mode={mode})"

    def query(self, v, n):
        return self.index.knn(self.q(v), n)


class VexactApprox(Vexact):
    def __init__(self, metric, method_param=None):
        super().__init__(metric, method_param)
        self.nprobe, self.eps = 1, 0.0

    def set_query_arguments(self, nprobe, eps):
        self.nprobe, self.eps = nprobe, eps
        if eps > 0:
            # the dial is fitted per k; ANN-Benchmarks asks for 10 neighbours and VIBE for 100
            self.index.fit_dial(10)
            self.index.fit_dial(100)
        self.name = f"vexact-approx (basis={self.basis}, query sample={self.sampled}, nprobe={nprobe}, eps={eps})"

    def query(self, v, n):
        return self.index.knn_approx(self.q(v), n, self.nprobe, self.eps)
