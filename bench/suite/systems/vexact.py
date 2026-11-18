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
        src = self.prep.corpus_path()
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
        self.ix = vexact_py.Index(out, core=self.core, **self.index_opts())
        self.encode_s = time.time() - t
        if self.params.get("disk_mode"):
            # at scale: reopen the saved index mapped, with the full vectors on file (load), not on the heap
            self.ix = None
            info = self.load()
            self.disk_info = info
        # fit.py stores the vectors zero-padded to a multiple of 32 dimensions, and the index takes queries that wide
        if not self.params.get("disk_mode"):
            self.qbuf = np.zeros(-(-self.dp // 32) * 32, np.float32)
        self.ids = None
        return dict(fit_s=round(self.fit_s, 1), encode_s=round(self.encode_s, 1), sample=learn is not None)

    disk = True

    def load(self):
        """Opens the saved index (v2::open maps it; nothing is encoded) with the full vectors moved to a file of their
        own in disk mode (params["disk"]: advice, read, willneed, direct_min, dontcache_min; the defaults are those of
        ../threshold-db/REPORT_CAPPED.md's runs: random advice, io_uring reads priced by the planner, willneed, O_DIRECT
        from 8 KiB)."""
        import vexact_py
        dk = dict(advice="random", read="uring", willneed=True, direct_min=8192, dontcache_min=None)
        dk.update(self.params.get("disk", {}))
        if dk["read"] == "uring" and not vexact_py.Index.uring_available():
            dk["read"] = "mmap"
        self.disk_opts = dk
        full = os.path.join(self.work, "full.f16")
        if os.path.exists(full):
            os.remove(full)
        self.ix = vexact_py.Index(os.path.join(self.work, "index"), core=self.core, full_file=full, **dk,
                                  **self.index_opts())
        self.qbuf = np.zeros(self.ix.dim, np.float32)
        return dict(dk, full_resident=self.ix.full_resident())

    def index_opts(self):
        # the deterministic levels (`dlv`, cumulative bytes): None, the library's default (two, at 16 and 32 bytes);
        # [] the fit's six
        dlv = self.params.get("dlv")
        return {} if dlv is None else dict(dlv=list(dlv))

    def counters(self):
        # the reader's reads (io_uring, disk mode): count, bytes, and the time spent waiting for them
        import vexact_py
        n, b, w = vexact_py.Index.io_counters()
        return dict(reads=n, read_kb=b / 1024, wait_us=w / 1e3)

    def evict_cold(self, cg):
        # the full vectors only: the codes and the rest of the index stay where they are
        self.ix.evict_full()

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

    def settings(self, qtype, filtered):
        # the approximate mode's cluster orders to compare (`orders`, e.g. ["radius", "spread"]), one setting each
        return list(self.params.get("orders", ["default"])) if qtype == "approx" else ["default"]

    def use(self, setting):
        self.setting = setting
        if setting != "default":
            self.ix.set_order(setting)

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

