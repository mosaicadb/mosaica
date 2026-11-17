# The index (rust/) through the bindings in bench/annb/vexact-py (install their wheel into the Python that runs the
# suite). Built as bench/annb/vexact/module.py builds it: rust/tools/fit.py --basis dual on the corpus (with
# --query-sample from the learn sample on out-of-distribution sets), then vexact_py.Index, which encodes the fit and
# saves the index in the work directory. Queries run on the index's own one-thread pool, pinned to the timing core.
import os
import subprocess
import sys
import time

import numpy as np

from .base import System, Unavailable
from common import REPO

MIN_DIM = 64  # the randomised levels need at least 50 dimensions; narrower vectors get zero columns


class Vexact(System):
    name = "vexact"
    types = ("exact", "threshold", "approx")
    rounding = "f16"

    def build(self, threads):
        try:
            import vexact_py
        except ImportError as e:
            raise Unavailable(f"vexact_py is not installed ({e}); build bench/annb/vexact-py with maturin")
        os.makedirs(self.work, exist_ok=True)
        d = self.prep.d
        self.dp = max(d, MIN_DIM)
        src = self.prep.f("corpus32.npy")
        tmp = []
        if self.dp != d:
            src = self._padded("train.npy", self.prep.corpus())
            tmp.append(src)
        out = os.path.join(self.work, "index")
        args = [sys.executable, os.path.join(REPO, "rust", "tools", "fit.py"), src, out, "--metric", "angular",
                "--basis", self.params.get("basis", "dual")]
        learn = self.prep.learn()
        if learn is not None and self.params.get("basis", "dual") == "dual":
            args += ["--query-sample", self._padded("sample.npy", learn)]
            tmp.append(args[-1])
        t = time.time()
        env = dict(os.environ, OPENBLAS_NUM_THREADS=str(threads), OMP_NUM_THREADS=str(threads),
                   MKL_NUM_THREADS=str(threads))
        subprocess.run(args, check=True, stdout=subprocess.DEVNULL, env=env)
        self.fit_s = time.time() - t
        for f in tmp:
            os.remove(f)
        t = time.time()
        self.ix = vexact_py.Index(out, core=self.core)
        self.encode_s = time.time() - t
        # fit.py stores the vectors zero-padded to a multiple of 32 dimensions, and the index takes queries that wide
        self.qbuf = np.zeros(-(-self.dp // 32) * 32, np.float32)
        self.ids = None
        return dict(fit_s=round(self.fit_s, 1), encode_s=round(self.encode_s, 1), sample=learn is not None)

    def _padded(self, name, X):
        p = os.path.join(self.work, name)
        out = np.lib.format.open_memmap(p, mode="w+", dtype=np.float32, shape=(X.shape[0], self.dp))
        for i in range(0, X.shape[0], 65536):
            out[i:i + 65536, :X.shape[1]] = X[i:i + 65536]
        out.flush()
        return p

    def index_bytes(self):
        tot = 0
        for root, _, files in os.walk(self.work):
            tot += sum(os.path.getsize(os.path.join(root, f)) for f in files)
        return tot

    def set_visible(self, ids, name=None):
        self.vis = ids
        self.ix.set_visible(None if ids is None else ids.astype(np.int64))

    def q(self, v):
        self.qbuf[:len(v)] = v
        return self.qbuf

    def knn(self, q, k):
        return self.ix.knn(self.q(q), k)

    def range(self, q, t):
        return self.ix.threshold(self.q(q), t)

    def approx(self, q, k, p):
        return self.ix.knn_approx(self.q(q), k, p["nprobe"], p.get("eps", 0.0))

    def approx_grid(self):
        g = self.params.get("approx", [])
        if any(p.get("eps", 0) > 0 for p in g):
            for k in (10, 100):
                self.ix.fit_dial(k)
        return g

    def close(self):
        self.ix = None
