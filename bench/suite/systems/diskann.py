# DiskANN's SSD index (Subramanya et al., NeurIPS 2019), from microsoft/DiskANN's C++ release 0.7.0 (the code the
# diskannpy 0.7.0 wheel wraps) through ../native/diskann_bench.cpp; install/diskann.sh builds it. The binary:
# params["bin"], else $DISKANN_BENCH, else diskann_bench on PATH.
#
# The index keeps PQ codes in memory and the graph with the full f32 vectors on disk, read with Linux AIO (O_DIRECT,
# no node cache); L2 over the unit vectors (the order of inner product). R 64 and build list L 100, as
# ../threshold-db/capped/diskann_capped.py. The PQ codes are sized for the smallest cap: their memory budget is
# params["pq_fraction"] (0.05) of the dataset's size (N x d x 4 bytes), half the 10% cap, so one index serves every
# cap; the budget chosen is recorded with the build. Approximate k-NN with search list L from the grid, beam width 2,
# unfiltered only (the suite's visible sets are given at query time, which the SSD index does not take).
import json
import os
import shutil
import subprocess

import numpy as np

from .base import System, Unavailable


class DiskANN(System):
    name = "diskann"
    types = ("approx",)
    disk = True
    filters = False

    def _bin(self):
        b = self.params.get("bin") or os.environ.get("DISKANN_BENCH") or shutil.which("diskann_bench")
        if not b or not os.path.exists(b):
            raise Unavailable("diskann_bench is not built (install/diskann.sh) or not given ($DISKANN_BENCH)")
        return b

    def prefix(self):
        return os.path.join(self.work, "disk")

    def build(self, threads):
        b = self._bin()
        os.makedirs(self.work, exist_ok=True)
        N, d = self.prep.N, self.prep.d
        binf = os.path.join(self.work, "corpus32.bin")
        # a corpus file already in place is used as it is: written ahead, it lets corpus32.npy go before a build that
        # needs the disk (about 2 + 2 + 4 GB at a million 512-d vectors: this file, the graph build's copy, the index)
        if not (os.path.exists(binf) and os.path.getsize(binf) == 8 + 4 * N * d):
            X = self.prep.corpus()
            with open(binf, "wb") as f:
                f.write(np.array([N, d], "<i4").tobytes())
                for i in range(0, N, 100_000):
                    f.write(np.ascontiguousarray(X[i:i + 100_000], dtype="<f4").tobytes())
        pq_gb = self.params.get("pq_fraction", 0.05) * self.prep.dataset_bytes() / 1e9
        R, L = self.params.get("R", 64), self.params.get("L", 100)
        cmd = [b, "build", binf, self.prefix(), str(R), str(L), f"{pq_gb:.6f}", str(self.params.get("build_ram_gb", 4)),
               str(threads)]
        r = subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        os.remove(binf)
        if r.returncode:
            raise Unavailable(f"diskann_bench build failed ({r.returncode}): {r.stderr[-2000:]}")
        info = dict(R=R, L=L, pq_ram_gb=round(pq_gb, 4), pq_fraction=self.params.get("pq_fraction", 0.05))
        with open(os.path.join(self.work, "build.json"), "w") as f:
            json.dump(info, f)
        return info

    def load(self):
        b = self._bin()
        info = json.load(open(os.path.join(self.work, "build.json")))
        cmd = [b, "serve", self.prefix(), self.prep.f("queries.npy")]
        if self.core is not None:
            cmd = ["taskset", "-c", str(self.core)] + cmd
        self.srv = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                    env=dict(os.environ, OMP_NUM_THREADS="1"))
        # one search before returning, so that the index is loaded (in this process's cgroup) before a cap is set
        self.qi = 0
        self.approx(None, 1, dict(L=10, beam=1))
        self.own_ms = None
        return info

    def io_pids(self):
        return [os.getpid()] + ([self.srv.pid] if getattr(self, "srv", None) else [])

    def single_thread(self):
        if getattr(self, "srv", None) is None:
            self.load()

    def index_bytes(self):
        return sum(os.path.getsize(os.path.join(self.work, f)) for f in os.listdir(self.work))

    def approx(self, q, k, p):
        self.srv.stdin.write(f"q {self.qi} {k} {max(p['L'], k)} {p.get('beam', 2)}\n".encode())
        self.srv.stdin.flush()
        head = self.srv.stdout.read(16)
        if len(head) < 16:
            raise RuntimeError("diskann_bench stopped")
        self.own_ms = float(np.frombuffer(head[:8], np.float64)[0])
        n = int(np.frombuffer(head[8:], np.int64)[0])
        ids = np.frombuffer(self.srv.stdout.read(8 * n), np.int64)
        st = np.frombuffer(self.srv.stdout.read(48), np.float64)
        # DiskANN's own counters for this search: reads, rounds (each waits for its beam's reads), distances, the
        # time in I/O and in CPU (us), the bytes read
        self.qstats = dict(reads=int(st[0]), rounds=int(st[1]), cmps=int(st[2]), io_us=round(float(st[3]), 1),
                           cpu_us=round(float(st[4]), 1), read_kb=round(float(st[5]) / 1024, 1))
        return ids

    def query_stats(self):
        return getattr(self, "qstats", None)

    def close(self):
        if getattr(self, "srv", None):
            self.srv.stdin.close()
            self.srv.wait()
            self.srv = None
