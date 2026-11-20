# PyNNDescent (pip pynndescent): an NN-descent k-NN graph searched from random-projection trees, inner product over the
# unit vectors ("dot"). Approximate k-NN only, and unfiltered only: its query takes no visible set. The build runs on the
# build cores through numba's thread pool; queries run on one thread. The search is compiled at the end of the build
# (prepare(compile_search=True) and one query), so no timed query pays for numba's compilation.
import numpy as np

from .base import System, Unavailable


class PyNNDescent(System):
    name = "pynndescent"
    types = ("approx",)
    filters = False

    def build(self, threads):
        try:
            import numba
            import pynndescent
        except ImportError as e:
            raise Unavailable(f"pynndescent is not installed ({e}): pip install pynndescent")
        self.numba = numba
        numba.set_num_threads(threads)
        p = self.params
        X = np.ascontiguousarray(self.prep.corpus(), dtype=np.float32)
        self.ix = pynndescent.NNDescent(X, metric="dot", n_neighbors=p.get("n_neighbors", 30),
                                        pruning_degree_multiplier=p.get("pruning_degree_multiplier", 1.5),
                                        diversify_prob=p.get("diversify_prob", 1.0), n_jobs=threads,
                                        random_state=p.get("seed", 0), low_memory=True)
        self.ix.prepare(compile_search=True)
        self.ix.query(X[:1], k=10, epsilon=0.1)
        return dict(n_neighbors=p.get("n_neighbors", 30), pruning_degree_multiplier=p.get("pruning_degree_multiplier", 1.5),
                    diversify_prob=p.get("diversify_prob", 1.0), version=pynndescent.__version__)

    def single_thread(self):
        self.numba.set_num_threads(1)

    def index_bytes(self):
        g = getattr(self.ix, "_search_graph", None)
        graph = (g.data.nbytes + g.indices.nbytes + g.indptr.nbytes) if g is not None else 0
        return self.prep.N * self.prep.d * 4 + graph

    def approx(self, q, k, p):
        I, _ = self.ix.query(np.asarray(q, dtype=np.float32)[None, :], k=k, epsilon=p["epsilon"])
        return I[0][I[0] >= 0]
