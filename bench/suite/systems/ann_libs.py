# Approximate k-NN libraries from the public suites, unfiltered (they take no visible set), built and queried as their
# ANN-Benchmarks modules do, over the unit vectors with inner product:
#   hnswlib       hnswlib.Index("ip"), the grid's M and efConstruction; ef per query
#   glass         glass 2.1 (github.com/zilliztech/pyglass, built from source with -march=native; the pip wheel
#                 glassppy needs AVX-512), as VIBE's module: Index("HNSW", quant, R, L), Searcher with its search and
#                 refine quantisers, optimize(1); ef per query
#   pynndescent   NNDescent(metric "dot", low_memory, compressed) with the grid's n_neighbors, leaf_size,
#                 pruning_degree_multiplier and diversify_prob, then prepare(); epsilon per query
# A system with several builds (hnswlib's M, pynndescent's graphs) runs each as a setting of its own, built once.
import numpy as np

from .base import System, Unavailable


class Multi(System):
    """A system whose grid holds several builds: params["builds"] is a list of dicts (build parameters plus "query",
    that build's query grid); each build is a setting named by its parameters, built in build() on several threads."""
    types = ("approx",)
    filters = False

    def build(self, threads):
        self.threads = threads
        self.ixs = {}
        info = {}
        for b in self.params["builds"]:
            name = self.label(b)
            self.ixs[name] = self.build_one({k: v for k, v in b.items() if k != "query"}, threads)
            info[name] = "built"
        return info

    @staticmethod
    def label(b):
        return ",".join(f"{k}={v}" for k, v in b.items() if k != "query")

    def settings(self, qtype, filtered):
        return [self.label(b) for b in self.params["builds"]]

    def approx_grid_for(self, setting):
        b = next(b for b in self.params["builds"] if self.label(b) == setting)
        return b["query"]

    def index_bytes(self):
        return 0


class HnswLib(Multi):
    name = "hnswlib"

    def build_one(self, b, threads):
        try:
            import hnswlib
        except ImportError as e:
            raise Unavailable(f"hnswlib is not installed ({e})")
        X = self.prep.corpus()
        ix = hnswlib.Index(space="ip", dim=self.prep.d)
        ix.init_index(max_elements=self.prep.N, ef_construction=b["efConstruction"], M=b["M"])
        for i in range(0, self.prep.N, 100_000):
            ix.add_items(np.ascontiguousarray(X[i:i + 100_000], dtype=np.float32),
                         np.arange(i, min(i + 100_000, self.prep.N)), num_threads=threads)
        return ix

    def single_thread(self):
        for ix in self.ixs.values():
            ix.set_num_threads(1)

    def approx(self, q, k, p):
        ix = self.ixs[self.setting]
        ix.set_ef(max(p["ef"], k))
        return ix.knn_query(np.asarray(q, dtype=np.float32)[None, :], k=k)[0][0].astype(np.int64)


class Glass(Multi):
    name = "glass"

    def build_one(self, b, threads):
        try:
            import glass
        except ImportError as e:
            raise Unavailable(f"glass is not installed ({e}): build github.com/zilliztech/pyglass from source")
        self.glass = glass
        glass.set_num_threads(threads)
        X = np.ascontiguousarray(self.prep.corpus(), dtype=np.float32)
        g = glass.Index("HNSW", metric="IP", quant=b["quant"], R=b["R"], L=b["L"]).build(X)
        s = glass.Searcher(g, X, "IP", b["search_quant"], b["refine_quant"])
        s.optimize(1)
        return s

    def single_thread(self):
        self.glass.set_num_threads(1)

    def approx(self, q, k, p):
        s = self.ixs[self.setting]
        s.set_ef(max(p["ef"], k))
        return np.asarray(s.search(np.asarray(q, dtype=np.float32), k), dtype=np.int64)


class PyNNDescent(Multi):
    name = "pynndescent"

    def build_one(self, b, threads):
        try:
            import numba
            import pynndescent
        except ImportError as e:
            raise Unavailable(f"pynndescent is not installed ({e})")
        numba.set_num_threads(threads)
        X = np.ascontiguousarray(self.prep.corpus(), dtype=np.float32)
        ix = pynndescent.NNDescent(X, metric="dot", low_memory=True, compressed=True, n_jobs=threads, **b)
        ix.prepare()
        return ix

    def single_thread(self):
        import numba
        numba.set_num_threads(1)

    def approx(self, q, k, p):
        ix = self.ixs[self.setting]
        ind, _ = ix.query(np.asarray(q, dtype=np.float32)[None, :], k=k, epsilon=p["epsilon"])
        return ind[0].astype(np.int64)
