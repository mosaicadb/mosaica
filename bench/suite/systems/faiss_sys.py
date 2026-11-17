# faiss (pip faiss-cpu): the flat index (the standard exact system), Panorama (a frontier exact system) and HNSW (the
# standard approximate system under a visible set). Forms as in ../threshold-db/ann.py: inner product over the unit
# vectors; a visible set through IDSelectorBitmap, and for the flat index also over a compacted copy of the visible rows
# (built when the visible set is set, as a session would; its time is recorded with the setup).
import numpy as np

from .base import System, Unavailable, faiss_bitmap


def _faiss():
    try:
        import faiss
    except ImportError as e:
        raise Unavailable(f"faiss is not installed ({e}): pip install faiss-cpu")
    return faiss


def _add(ix, X, transform=None):
    for i in range(0, X.shape[0], 100_000):
        B = np.ascontiguousarray(X[i:i + 100_000], dtype=np.float32)
        ix.add(np.ascontiguousarray(B @ transform) if transform is not None else B)


class FaissBase(System):
    def build(self, threads):
        self.faiss = _faiss()
        self.faiss.omp_set_num_threads(threads)
        self.sel = self.compact = None
        return self._build()

    def single_thread(self):
        self.faiss.omp_set_num_threads(1)

    def set_visible(self, ids, name=None):
        self.vis, self.compact = ids, None
        if ids is None:
            self.sel = self.bits = None
            return
        self.sel, self.bits = faiss_bitmap(ids, self.prep.N)

    def _params(self):
        return self.faiss.SearchParameters(sel=self.sel) if self.sel is not None else None

    def _q(self, q):
        return np.ascontiguousarray(q, dtype=np.float32)[None, :]


class FaissFlat(FaissBase):
    name = "faiss-flat"
    types = ("exact", "threshold")

    def _build(self):
        self.ix = self.faiss.IndexFlatIP(self.prep.d)
        _add(self.ix, self.prep.corpus())
        return {}

    def index_bytes(self):
        return self.prep.N * self.prep.d * 4

    def settings(self, qtype, filtered):
        return self.params.get("settings", ["idselector"]) if filtered else ["default"]

    def use(self, setting):
        self.setting = setting
        if setting == "compacted" and self.compact is None and self.vis is not None:
            self.compact = self.faiss.IndexFlatIP(self.prep.d)
            self.compact.add(np.ascontiguousarray(self.prep.corpus()[self.vis], dtype=np.float32))

    def knn(self, q, k):
        if self.setting == "compacted":
            _, I = self.compact.search(self._q(q), k)
            return self.vis[I[0][I[0] >= 0]]
        _, I = self.ix.search(self._q(q), k, params=self._params())
        return I[0][I[0] >= 0]

    def range(self, q, t):
        if self.setting == "compacted":
            _, _, I = self.compact.range_search(self._q(q), float(t))
            return self.vis[I]
        _, _, I = self.ix.range_search(self._q(q), float(t), params=self._params())
        return I


class FaissPanorama(FaissBase):
    """IndexFlatIPPanorama after an orthogonal PCA rotation of data and queries (Panorama prunes on the leading
    dimensions; faiss's PCAMatrix centres the data, which changes inner products). The rotation of each query is part
    of its timed call."""
    name = "faiss-panorama"
    types = ("exact", "threshold")

    def _build(self):
        if not hasattr(self.faiss, "IndexFlatIPPanorama"):
            raise Unavailable(f"faiss {self.faiss.__version__} has no IndexFlatIPPanorama (it came in 1.13)")
        X = self.prep.corpus()
        N, d = X.shape
        rng = np.random.default_rng(1)
        S = np.asarray(X[np.sort(rng.choice(N, min(N, 200_000), replace=False))], dtype=np.float64)
        _, V = np.linalg.eigh(np.cov(S, rowvar=False))
        self.rot = np.ascontiguousarray(V[:, ::-1]).astype(np.float32)
        del S
        self.ix = self.faiss.IndexFlatIPPanorama(d, self.params.get("levels", 8))
        _add(self.ix, X, self.rot)
        return {}

    def index_bytes(self):
        return self.prep.N * self.prep.d * 4

    def settings(self, qtype, filtered):
        return ["idselector"] if filtered else ["default"]

    def knn(self, q, k):
        _, I = self.ix.search(self._q(q) @ self.rot, k, params=self._params())
        return I[0][I[0] >= 0]

    def range(self, q, t):
        _, _, I = self.ix.range_search(self._q(q) @ self.rot, float(t), params=self._params())
        return I


class FaissHNSW(FaissBase):
    name = "faiss-hnsw"
    types = ("approx",)

    def _build(self):
        p = self.params
        self.ix = self.faiss.IndexHNSWFlat(self.prep.d, p.get("M", 16), self.faiss.METRIC_INNER_PRODUCT)
        self.ix.hnsw.efConstruction = p.get("efConstruction", 200)
        _add(self.ix, self.prep.corpus())
        return dict(M=p.get("M", 16), efConstruction=p.get("efConstruction", 200))

    def index_bytes(self):
        return self.prep.N * (self.prep.d * 4 + self.params.get("M", 16) * 2 * 4)

    def approx(self, q, k, p):
        sp = self.faiss.SearchParametersHNSW(efSearch=max(p["ef"], k))
        if self.sel is not None:
            sp.sel = self.sel
        _, I = self.ix.search(self._q(q), k, params=sp)
        return I[0][I[0] >= 0]
