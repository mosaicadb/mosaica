# ACORN-gamma (Patel et al., SIGMOD 2024; github.com/stanford-futuredata/ACORN, a faiss 1.7.3 fork, with
# ../threshold-db/acorn.patch): a filtered HNSW whose build ignores predicates, so one index serves any visible set.
# Driven through ../threshold-db/acorn_bench.cpp (install/acorn.sh builds it): `build` inserts the corpus with L2 over
# the unit vectors (the same order as inner product) on several threads; `serve` answers over a pipe, pinned to the
# timing core, and times each search itself (the time reported is the search call's own, without the pipe).
# The binary: params["bin"], else $ACORN_BENCH, else acorn_bench on PATH.
import os
import shutil
import subprocess
import time

import numpy as np

from .base import System, Unavailable


class Acorn(System):
    name = "acorn"
    types = ("approx",)

    def build(self, threads):
        self.bin = self.params.get("bin") or os.environ.get("ACORN_BENCH") or shutil.which("acorn_bench")
        if not self.bin or not os.path.exists(self.bin):
            raise Unavailable("acorn_bench is not built (install/acorn.sh) or not given ($ACORN_BENCH)")
        os.makedirs(self.work, exist_ok=True)
        self.path = os.path.join(self.work, "acorn.index")
        p = self.params
        env = dict(os.environ, OMP_NUM_THREADS=str(threads))
        subprocess.run([self.bin, "build", self.prep.f("corpus32.npy"), self.path, str(p.get("M", 32)),
                        str(p.get("gamma", 12)), str(p.get("M_beta", 32)), "0", str(self.prep.N)],
                       check=True, env=env, stdout=subprocess.DEVNULL)
        return dict(M=p.get("M", 32), gamma=p.get("gamma", 12), M_beta=p.get("M_beta", 32))

    def single_thread(self):
        env = dict(os.environ, OMP_NUM_THREADS="1")
        cmd = [self.bin, "serve", self.path, self.prep.f("queries.npy")]
        if self.core is not None:
            cmd = ["taskset", "-c", str(self.core)] + cmd
        self.srv = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, env=env)
        self.name_vis = "all"

    def index_bytes(self):
        return os.path.getsize(self.path)

    def set_visible(self, ids, name=None):
        self.vis = ids
        if ids is None:
            self.name_vis = "all"
            return
        self.name_vis = name
        self.srv.stdin.write(f"vis {name} {os.path.abspath(self.prep.f(f'vis_{name}.npy'))}\n".encode())
        self.srv.stdin.flush()

    def approx(self, q, k, p):
        self.srv.stdin.write(f"q {self.qi} {k} {max(p['ef'], k)} {self.name_vis}\n".encode())
        self.srv.stdin.flush()
        head = self.srv.stdout.read(16)
        self.own_ms = float(np.frombuffer(head[:8], np.float64)[0])
        n = int(np.frombuffer(head[8:], np.int64)[0])
        return np.frombuffer(self.srv.stdout.read(8 * n), np.int64)

    def close(self):
        if getattr(self, "srv", None):
            self.srv.stdin.close()
            self.srv.wait()
