# faiss (pip faiss-cpu): the flat index (the standard exact system), Panorama (a frontier exact system) and HNSW (the
# standard approximate system under a visible set). Forms as in the earlier threshold-db/ann.py: inner product over the unit
# vectors; a visible set through IDSelectorBitmap, and for the flat index also over a compacted copy of the visible rows
# (built when the visible set is set, as a session would; its time is recorded with the setup).
import os

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


def _train_sample(X):
    rng = np.random.default_rng(1)
    return np.ascontiguousarray(X[np.sort(rng.choice(X.shape[0], min(X.shape[0], 200_000), replace=False))],
                                dtype=np.float32)


def _pca_rotation(X):
    """An orthogonal rotation onto the principal axes of a sample of X, largest variance first (Panorama prunes on the
    leading dimensions; faiss's PCAMatrix centres the data, which changes inner products)."""
    S = _train_sample(X).astype(np.float64)
    _, V = np.linalg.eigh(np.cov(S, rowvar=False))
    return np.ascontiguousarray(V[:, ::-1]).astype(np.float32)


def _nlist(N):
    """IVF lists: 4096 at a million vectors and more, else the power of two nearest 4 sqrt(N)."""
    return 4096 if N >= 1_000_000 else int(2 ** round(np.log2(4 * np.sqrt(N))))


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
        self.rot = _pca_rotation(X)
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
        f = self.faiss
        if p.get("storage", "f32") == "f16":  # at scale, where only f16 vectors are stored: IndexHNSWSQ QT_fp16
            self.ix = f.IndexHNSWSQ(self.prep.d, f.ScalarQuantizer.QT_fp16, p.get("M", 16), f.METRIC_INNER_PRODUCT)
            self.ix.train(np.ascontiguousarray(self.prep.corpus()[:1000], dtype=np.float32))
        else:
            self.ix = f.IndexHNSWFlat(self.prep.d, p.get("M", 16), f.METRIC_INNER_PRODUCT)
        self.ix.hnsw.efConstruction = p.get("efConstruction", 200)
        _add(self.ix, self.prep.corpus())
        return dict(M=p.get("M", 16), efConstruction=p.get("efConstruction", 200), storage=p.get("storage", "f32"))

    def index_bytes(self):
        vec = 2 if self.params.get("storage", "f32") == "f16" else 4
        return self.prep.N * (self.prep.d * vec + self.params.get("M", 16) * 2 * 4)

    def approx(self, q, k, p):
        sp = self.faiss.SearchParametersHNSW(efSearch=max(p["ef"], k))
        if self.sel is not None:
            sp.sel = self.sel
        _, I = self.ix.search(self._q(q), k, params=sp)
        return I[0][I[0] >= 0]


class FaissIVFPanorama(FaissBase):
    """IndexIVFFlatPanorama (faiss 1.14 and later): IVF-Flat whose lists keep Panorama's level layout and prune on
    partial products, after the same orthogonal PCA rotation as faiss-panorama (inside the timed call for each query).
    Inner product; nlist as faiss-ivf-disk, trained on 200,000 rows. Approximate k-NN at the grid's nprobe; a visible
    set through IDSelectorBitmap."""
    name = "faiss-ivf-panorama"
    types = ("approx",)

    def _build(self):
        f = self.faiss
        if not hasattr(f, "IndexIVFFlatPanorama"):
            raise Unavailable(f"faiss {f.__version__} has no IndexIVFFlatPanorama")
        X = self.prep.corpus()
        N, d = X.shape
        self.rot = _pca_rotation(X)
        nlist = self.params.get("nlist") or _nlist(N)
        levels = self.params.get("levels", 8)
        self.quantizer = f.IndexFlatIP(d)
        self.ix = f.IndexIVFFlatPanorama(self.quantizer, d, nlist, levels, f.METRIC_INNER_PRODUCT)
        self.ix.train(_train_sample(X) @ self.rot)
        _add(self.ix, X, self.rot)
        return dict(nlist=nlist, levels=levels)

    def index_bytes(self):
        return self.prep.N * self.prep.d * 4

    def approx(self, q, k, p):
        sp = self.faiss.SearchParametersIVF(nprobe=p["nprobe"])
        if self.sel is not None:
            sp.sel = self.sel
        _, I = self.ix.search(self._q(q) @ self.rot, k, params=sp)
        return I[0][I[0] >= 0]


class FaissIVFPQfs(FaissBase):
    """faiss IVF-PQ fast scan with a flat refine, as ANN-Benchmarks' faiss-ivfpqfs and VIBE's ivfpqfs(faiss):
    index_factory "IVF{nlist},PQ{d/2}x4fs" (inner product) under IndexRefineFlat; the grid's nprobe and k_factor (the
    refine reranks k x k_factor candidates on the f32 vectors). Unfiltered only, as in the public suites."""
    name = "faiss-ivfpqfs"
    types = ("approx",)
    filters = False

    def _build(self):
        f = self.faiss
        X = self.prep.corpus()
        N, d = X.shape
        nlist = self.params.get("nlist", 1024)
        self.base = f.index_factory(d, f"IVF{nlist},PQ{d // 2}x4fs", f.METRIC_INNER_PRODUCT)
        self.base.train(_train_sample(X))
        self.ix = f.IndexRefineFlat(self.base)
        _add(self.ix, X)
        return dict(nlist=nlist, pq=f"PQ{d // 2}x4fs")

    def index_bytes(self):
        return self.prep.N * (self.prep.d * 4 + self.prep.d // 4)

    def approx(self, q, k, p):
        sp = self.faiss.IndexRefineSearchParameters(
            k_factor=float(p["k_factor"]), base_index_params=self.faiss.SearchParametersIVF(nprobe=p["nprobe"]))
        _, I = self.ix.search(self._q(q), k, params=sp)
        return I[0][I[0] >= 0]


class FaissFlatMmap(FaissBase):
    """The scan baseline under a memory cap: IndexFlatIP (or, with params storage "f16", IndexScalarQuantizer QT_fp16,
    as at scale) written to a file and opened memory-mapped (IO_FLAG_MMAP_IFC), so its vectors are page cache, not
    anonymous memory. search / range_search; a visible set through IDSelectorBitmap."""
    name = "faiss-flat-mmap"
    types = ("exact", "threshold")
    disk = True

    def path(self):
        return os.path.join(self.work, "flat.index")

    def _build(self):
        import os as _os
        _os.makedirs(self.work, exist_ok=True)
        d = self.prep.d
        if self.params.get("storage", "f32") == "f16":
            self.rounding = "f16"
            ix = self.faiss.IndexScalarQuantizer(d, self.faiss.ScalarQuantizer.QT_fp16, self.faiss.METRIC_INNER_PRODUCT)
        else:
            ix = self.faiss.IndexFlatIP(d)
        _add(ix, self.prep.corpus())
        self.faiss.write_index(ix, self.path())
        del ix
        self.ix = self.faiss.read_index(self.path(), self.faiss.IO_FLAG_MMAP_IFC)
        return dict(storage=self.params.get("storage", "f32"))

    def load(self):
        self.faiss = _faiss()
        self.sel = self.compact = None
        self.ix = self.faiss.read_index(self.path(), self.faiss.IO_FLAG_MMAP_IFC)
        return dict(storage=self.params.get("storage", "f32"))

    def index_bytes(self):
        return os.path.getsize(self.path())

    def knn(self, q, k):
        _, I = self.ix.search(self._q(q), k, params=self._params())
        return I[0][I[0] >= 0]

    def range(self, q, t):
        _, _, I = self.ix.range_search(self._q(q), float(t), params=self._params())
        return I


class FaissIVFDisk(FaissBase):
    """faiss IVF with its inverted lists on disk (OnDiskInvertedLists, mapped by faiss), as
    ../threshold-db/capped/faiss_capped.py: IVF-SQ QT_fp16 (params storage "f32": IVF-Flat), nlist 4096 at a million
    vectors and more (else the power of two nearest 4 sqrt(N)), trained on 200,000 rows. Approximate k-NN at the
    grid's nprobe; a visible set through IDSelectorBitmap. Opened without IO_FLAG_MMAP (on top of the on-disk lists it
    crashed the first search in faiss 1.16.0); the lists are mapped all the same."""
    name = "faiss-ivf-disk"
    types = ("approx",)
    disk = True

    def path(self):
        return os.path.join(self.work, "ivf_ondisk.index")

    def _build(self):
        os.makedirs(self.work, exist_ok=True)
        f = self.faiss
        X = self.prep.corpus()
        N, d = X.shape
        nlist = self.params.get("nlist") or _nlist(N)
        q = f.IndexFlatIP(d)
        if self.params.get("storage", "f16") == "f16":
            ix = f.IndexIVFScalarQuantizer(q, d, nlist, f.ScalarQuantizer.QT_fp16, f.METRIC_INNER_PRODUCT)
        else:
            ix = f.IndexIVFFlat(q, d, nlist, f.METRIC_INNER_PRODUCT)
        ix.train(_train_sample(X))
        _add(ix, X)
        od = f.OnDiskInvertedLists(ix.nlist, ix.code_size, os.path.join(self.work, "ivf_ondisk.ivfdata"))
        v = f.InvertedListsPtrVector()
        v.push_back(ix.invlists)
        od.merge_from_multiple(v.data(), v.size(), False)
        ix.replace_invlists(od, True)
        od.this.disown()
        f.write_index(ix, self.path())
        del ix
        self.load()
        return dict(nlist=nlist, storage=self.params.get("storage", "f16"))

    def load(self):
        self.faiss = _faiss()
        self.sel = self.compact = None
        cwd = os.getcwd()
        os.chdir(self.work)  # the index file names its .ivfdata relative to where it was written
        try:
            self.ix = self.faiss.read_index(self.path())
        finally:
            os.chdir(cwd)
        return dict(nlist=self.ix.nlist)

    def index_bytes(self):
        return sum(os.path.getsize(os.path.join(self.work, f)) for f in os.listdir(self.work))

    def approx(self, q, k, p):
        sp = self.faiss.SearchParametersIVF(nprobe=p["nprobe"])
        if self.sel is not None:
            sp.sel = self.sel
        _, I = self.ix.search(self._q(q), k, params=sp)
        return I[0][I[0] >= 0]
