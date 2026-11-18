# The fast-scan index (rust/, through bench/annb/vexact-py) as an ANN-Benchmarks and VIBE algorithm. fit() runs
# rust/tools/fit.py on the training vectors and builds the index; queries run one at a time on one thread.
# VEXACT_SRC is where the image keeps the sources (rust/ and combined/); VEXACT_WORK is where fit() writes.
# The build parameter `basis` (dual, the default, or pca) is fit.py's --basis; fit.py's own default is dual too, and the
# measured index (SYSTEM.md) uses dual. With dual on a dataset that supplies a sample of its
# queries (VIBE's out-of-distribution datasets, through fit_ood), the sample also shapes the basis (--query-sample).
# The build parameter `order` is accepted as corpus only (the approximate k-NN fits nothing on queries; the legacy
# arm's dial is fitted on corpus pseudo-queries).
# The build parameters `clusters` and `dlv` are research/levels-clusters' options (REPORT_BUILD.md, "the levels-clusters
# defaults"): `clusters` 1000 (the default, 1,000 items per cluster) or by-n (fit.py --per-cluster by-n --groups by-n),
# and `dlv` the deterministic levels' cumulative bytes: default (the library's, 16.32), fit (the fit's six) or e.g. "16.32".
# fit() encodes the index and saves it in the work directory (v2::rebuild); the directory is removed at exit.
import atexit
import os
import shutil
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


# VIBE passes the build arguments as values in the order of config.yml's args; these follow basis
BUILD_ARGS = ("order", "clusters", "dlv")


class Vexact(BaseANN):
    def __init__(self, metric, method_param=None, *rest):
        if metric not in ("angular", "cosine", "normalized"):
            raise ValueError(f"vexact supports cosine similarity only (angular, cosine, normalized), not {metric}")
        # ANN-Benchmarks passes the build arguments as a dict, VIBE as values in order
        self.basis = method_param.get("basis", "dual") if isinstance(method_param, dict) else method_param or "dual"
        if self.basis not in ("pca", "dual"):
            raise ValueError(f"basis is pca or dual, not {self.basis}")
        a = method_param if isinstance(method_param, dict) else dict(zip(BUILD_ARGS, rest))
        self.order = a.get("order") or "corpus"
        if self.order != "corpus":
            raise ValueError(f"order is corpus (nothing is fitted on queries any more), not {self.order}")
        self.clusters = str(a.get("clusters") or "1000")
        if not (self.clusters == "by-n" or self.clusters.isdigit()):
            raise ValueError(f"clusters is a number of items per cluster or by-n, not {self.clusters}")
        dlv = str(a.get("dlv") or "default")
        # None: the library's default levels; []: the fit's six (the bindings' convention)
        self.dlv = None if dlv == "default" else [] if dlv == "fit" else [int(x) for x in dlv.replace(",", ".").split(".")]
        self.tag = f"basis={self.basis}, order={self.order}, clusters={self.clusters}, dlv={dlv}"
        self.index = None

    def fit(self, X):
        self.build(X, None)

    def fit_ood(self, X, X_learn, X_learn_neighbors):
        self.build(X, X_learn if self.basis == "dual" else None)

    def build(self, X, sample):
        work = tempfile.mkdtemp(prefix="vexact-", dir=os.environ.get("VEXACT_WORK"))
        atexit.register(shutil.rmtree, work, ignore_errors=True)
        d = max(X.shape[1], MIN_DIM)
        # fit.py stores the vectors zero-padded to a multiple of 32 dimensions, and the index takes queries that wide
        self.qbuf = np.zeros(-(-d // 32) * 32, np.float32)
        train = os.path.join(work, "train.npy")
        args = [sys.executable, os.path.join(SRC, "rust", "tools", "fit.py"), train, os.path.join(work, "index"),
                "--metric", "angular", "--basis", self.basis, "--per-cluster", self.clusters]
        # the group rule named either way (fit.py's default is by-n since the levels-clusters decision)
        args += ["--groups", "by-n" if self.clusters == "by-n" else "by-k8"]
        save_padded(train, X, d)
        if sample is not None:
            args += ["--query-sample", os.path.join(work, "sample.npy")]
            save_padded(args[-1], sample, d)
        subprocess.run(args, check=True)
        for f in ("train.npy", "sample.npy"):
            if os.path.exists(os.path.join(work, f)):
                os.remove(os.path.join(work, f))
        self.index = vexact_py.Index(os.path.join(work, "index"), dlv=self.dlv)
        self.sampled = sample is not None

    def q(self, v):
        self.qbuf[: len(v)] = v
        return self.qbuf


class VexactExact(Vexact):
    def __init__(self, metric, method_param=None, *rest):
        super().__init__(metric, method_param, *rest)
        self.name = f"vexact-exact ({self.tag})"

    def set_query_arguments(self, mode="exact"):
        self.name = f"vexact-exact ({self.tag}, query sample={self.sampled}, mode={mode})"

    def query(self, v, n):
        return self.index.knn(self.q(v), n)


class VexactApprox(Vexact):
    """The approximate k-NN (knn_approx), its knob psi (larger reads more)."""

    def __init__(self, metric, method_param=None, *rest):
        super().__init__(metric, method_param, *rest)
        self.psi = 4.5

    def set_query_arguments(self, psi):
        self.psi = float(psi)
        self.name = f"vexact-approx ({self.tag}, query sample={self.sampled}, psi={psi})"

    def query(self, v, n):
        return self.index.knn_approx(self.q(v), n, self.psi)


class VexactNprobe(Vexact):
    """The legacy approximate arm (the spread order by nprobe, knn_nprobe), kept for comparison."""

    def __init__(self, metric, method_param=None, *rest):
        super().__init__(metric, method_param, *rest)
        self.nprobe, self.eps = 1, 0.0

    def set_query_arguments(self, nprobe, eps):
        self.nprobe, self.eps = nprobe, eps
        if eps > 0:
            # the dial is fitted per k; ANN-Benchmarks asks for 10 neighbours and VIBE for 100
            self.index.fit_dial(10)
            self.index.fit_dial(100)
        self.name = f"vexact-nprobe ({self.tag}, query sample={self.sampled}, nprobe={nprobe}, eps={eps})"

    def query(self, v, n):
        return self.index.knn_nprobe(self.q(v), n, self.nprobe, self.eps)
