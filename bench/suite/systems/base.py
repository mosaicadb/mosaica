# The adapter interface. A system builds over the prepared corpus and answers, one query at a time on one thread:
#   knn(q, k)              exact k-NN, entity ids best first                      (type "exact")
#   range(q, t)            every entity id with q.x >= t                          (type "threshold")
#   approx(q, k, param)    approximate k-NN at one of the grid's settings        (type "approx")
# all under the visible set last given to set_visible(ids) (None: everything). `types` says which it supports;
# `settings(qtype, filtered)` lists variants run as separate rows (e.g. a flat index under a visible set through an
# id selector and over a compacted copy), chosen with use(setting) before the queries.
# `rounding` is the precision of the stored vectors and of the query as the system computes with them, for the check's
# rounding allowance: "f16" (vectors and query rounded to f16, products accumulated exactly) or "f32" (f32 vectors and
# query, f32 accumulation).
#
# Under a memory cap (capmem.py, runsys.py --cap): a system with `disk = True` builds in one uncapped process
# (build(), leaving its index in the work directory), then each capped process calls load() inside its cgroup, which
# opens that index without rebuilding. data_files() are the files that hold its data (dropped from the page cache before
# the cap is set), and evict_cold() drops what a cold query must read again (default: the cgroup's page cache).
# A system that keeps its data in anonymous memory (`disk = False`) is built inside each capped process and the cap set
# after, so a cap below what it holds is refused: it does not fit. `filters = False`: the system takes no visible set; `unfiltered = False`: it runs the
# filtered cells only.
import os
import time

import numpy as np


class Unavailable(Exception):
    """The system cannot run here (a library missing or not built); the message says why."""


class System:
    name = ""
    types = ()
    rounding = "f32"
    disk = False
    filters = True
    unfiltered = True
    # the Python that runs this system ("" this one); e.g. DiskANN's own venv
    python_env = ""

    def __init__(self, prep, work, params, core=None):
        self.prep, self.work, self.params, self.core = prep, work, params, core
        self.vis = None
        self.setting = None

    # ---- build (several threads allowed) ----
    def build(self, threads):
        raise NotImplementedError

    def single_thread(self):
        """Called after the build, before any timed query: one thread from here on."""

    def load(self):
        """Opens the index build() left in the work directory (disk systems)."""
        raise NotImplementedError

    def data_files(self):
        return [self.work]

    def counters(self):
        """The system's own cumulative counters, read before and after a query under a cap (numbers only)."""
        return {}

    def query_stats(self):
        """Statistics of the last query the system reports itself (a dict), or None."""
        return None

    def io_pids(self):
        """The processes whose storage reads count as this system's (a server it drives, too)."""
        return [os.getpid()]

    def evict_cold(self, cg):
        cg.reclaim()

    def index_bytes(self):
        return 0

    # ---- queries ----
    def set_visible(self, ids, name=None):
        """ids: sorted int32 entity ids, or None; name: the visible set's name (its file is prep's vis_<name>.npy)."""
        self.vis = ids

    def settings(self, qtype, filtered):
        return ["default"]

    def use(self, setting):
        self.setting = setting

    def knn(self, q, k):
        raise NotImplementedError

    def range(self, q, t):
        raise NotImplementedError

    def approx(self, q, k, param):
        raise NotImplementedError

    def approx_grid(self):
        return self.params.get("approx", [])

    # a system that times its own queries (a server, say) sets this after each call, in ms
    own_ms = None

    def close(self):
        pass


def pin_thread(core):
    """Pins the calling thread to one core (Linux: pid 0 is the calling thread)."""
    if core is not None:
        os.sched_setaffinity(0, {core})


def faiss_bitmap(ids, N):
    import faiss
    mask = np.zeros(N, bool)
    mask[ids] = True
    bits = np.packbits(mask, bitorder="little")
    return faiss.IDSelectorBitmap(N, faiss.swig_ptr(bits)), bits


class Timer:
    def __init__(self):
        self.t = time.time()

    def s(self):
        return time.time() - self.t
