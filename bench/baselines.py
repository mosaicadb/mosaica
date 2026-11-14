# Baselines with ANN-Benchmarks' wrappers and parameter grids: hnswlib, faiss HNSW, faiss IVF-PQ fast scan with a
# re-rank (faiss-ivfpqfs), and brute force (faiss-flat: IndexFlatIP over the visible rows, built when the visible set
# is set, as an index's visible set is computed once per session). Build with many threads, query with one, one
# query at a time.
# usage: python3 -I baselines.py build DATASET_DIR OUT_DIR LIB [--threads T]
#        python3 -I baselines.py query DATASET_DIR OUT_DIR LIB [--reps R] [--filtered BUILD] [--nq N] [--efs E,E]
#   LIB: hnswlib | faiss-hnsw | faiss-ivfpqfs | faiss-flat. --filtered BUILD runs the visible sets of filters.py on that one build
#   (e.g. M-16, nlist-2048) instead of the unfiltered grid. Rows go to OUT_DIR/baselines.csv. --builds B,B runs only
#   those builds of the unfiltered grid, --fracs F,F only those visible fractions of a filtered run, --efs and
#   --nprobes replace the ef and nprobe grids.
import argparse, csv, os, sys, time
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from annlib import FRACS, K, load, recall, tag, unit  # noqa: E402

HNSW_M = [4, 8, 12, 16, 24, 36, 48, 64, 96]
HNSW_EF = [10, 20, 40, 80, 120, 200, 400, 600, 800]
IVF_NLIST = [512, 1204, 2048, 4096]
IVF_NPROBE = [1, 5, 10, 50, 100, 200]
IVF_KREORDER = [0, 10, 100, 1000]


def nonzero(X):
    return np.where(np.abs(X).sum(1) > 0)[0]


def builds(lib):
    if lib == "faiss-flat":
        return ["flat"]
    if lib in ("hnswlib", "faiss-hnsw"):
        return [f"M-{m}" for m in HNSW_M]
    return [f"nlist-{n}" for n in IVF_NLIST]


def qargs(lib, efs=None, nprobes=None):
    if lib == "faiss-flat":
        return [dict()]
    if lib in ("hnswlib", "faiss-hnsw"):
        return [dict(ef=e) for e in (efs or HNSW_EF)]
    return [dict(nprobe=p, k_reorder=r) for p in (nprobes or IVF_NPROBE) for r in IVF_KREORDER]


def path(out, lib, b):
    return os.path.join(out, f"{lib}_{b}.index")


def build(ds, out, lib, threads):
    import faiss, hnswlib
    if lib == "faiss-flat":
        return
    X = ds["train"]
    N, d = X.shape
    log = os.path.join(out, "builds.csv")
    for b in builds(lib):
        p = path(out, lib, b)
        if os.path.exists(p):
            continue
        t = time.time()
        v = int(b.split("-")[1])
        if lib == "hnswlib":
            ix = hnswlib.Index(space="cosine", dim=d)
            ix.init_index(max_elements=N, ef_construction=500, M=v)
            ix.set_num_threads(threads)
            ix.add_items(X, np.arange(N))
            ix.save_index(p + ".tmp")
        elif lib == "faiss-hnsw":
            # zero vectors are at distance 1 from every unit vector and become hubs that wreck the faiss graph
            # (recall 0.02 on nytimes); they are left out and ids are mapped back at query time
            faiss.omp_set_num_threads(threads)
            ix = faiss.IndexHNSWFlat(d, v)
            ix.hnsw.efConstruction = 500
            ix.add(np.ascontiguousarray(X[nonzero(X)]))
            faiss.write_index(ix, p + ".tmp")
        else:
            faiss.omp_set_num_threads(threads)
            ix = faiss.index_factory(d, f"IVF{v},PQ{d // 2}x4fs", faiss.METRIC_INNER_PRODUCT)
            ix.train(X)
            ix.add(X)
            faiss.write_index(ix, p + ".tmp")
        os.rename(p + ".tmp", p)
        s = time.time() - t
        with open(log, "a") as f:
            f.write(f"{lib},{b},{threads},{s:.1f},{os.path.getsize(p)}\n")
        print(lib, b, f"built in {s:.0f}s", flush=True)


class Searcher:
    """One loaded build; search(q, k) answers one query, set(**args) sets the query arguments."""

    def __init__(self, ds, out, lib, b):
        import faiss, hnswlib
        self.lib, self.faiss = lib, faiss
        p = path(out, lib, b)
        X = ds["train"]
        self.N = len(X)
        self.sel = None
        self.X = X
        if lib == "faiss-flat":
            faiss.omp_set_num_threads(1)
            self.visible(None)
        elif lib == "hnswlib":
            self.ix = hnswlib.Index(space="cosine", dim=X.shape[1])
            self.ix.load_index(p, max_elements=len(X))
            self.ix.set_num_threads(1)
        else:
            faiss.omp_set_num_threads(1)
            self.base = faiss.read_index(p)
            self.ids = nonzero(X) if lib == "faiss-hnsw" else np.arange(len(X))
            if lib == "faiss-ivfpqfs":
                self.refine = faiss.IndexRefineFlat(self.base, faiss.swig_ptr(X))

    def set(self, **a):
        self.a = a
        if self.lib == "faiss-flat":
            pass
        elif self.lib == "hnswlib":
            self.ix.set_ef(a["ef"])
        elif self.lib == "faiss-hnsw":
            self.base.hnsw.efSearch = a["ef"]
        else:
            self.base.nprobe = a["nprobe"]
            self.refine.k_factor = a["k_reorder"]
        self.params = None
        if self.sel is not None and self.lib not in ("hnswlib", "faiss-flat"):
            f = self.faiss
            if self.lib == "faiss-hnsw":
                self.params = f.SearchParametersHNSW(efSearch=a["ef"], sel=self.sel)
            else:
                ivf = f.SearchParametersIVF(nprobe=a["nprobe"], sel=self.sel)
                self.params = ivf if a["k_reorder"] == 0 else f.IndexRefineSearchParameters(k_factor=a["k_reorder"], base_index_params=ivf)
                self.ivf = ivf

    def visible(self, ids):
        """Restrict searches to entity ids (None: all)."""
        if self.lib == "faiss-flat":
            self.ids = np.arange(self.N) if ids is None else np.sort(np.asarray(ids, np.int64))
            self.base = self.faiss.IndexFlatIP(self.X.shape[1])
            self.base.add(np.ascontiguousarray(self.X[self.ids]))
            self.sel = None
            return
        if ids is None:
            self.sel, self.mask = None, None
            return
        self.mask = np.zeros(self.N, bool)
        self.mask[ids] = True
        if self.lib == "hnswlib":
            self.sel = True
        else:
            self.bits = np.packbits(self.mask[self.ids], bitorder="little")
            self.sel = self.faiss.IDSelectorBitmap(len(self.bits), self.faiss.swig_ptr(self.bits))

    def search(self, q, k):
        if self.lib == "hnswlib":
            if self.sel is None:
                return self.ix.knn_query(q[None], k=k)[0][0]
            m = self.mask
            return self.ix.knn_query(q[None], k=k, filter=lambda i: m[i])[0][0]
        q = q / np.linalg.norm(q) if np.linalg.norm(q) > 0 else q
        ix = self.refine if (self.lib == "faiss-ivfpqfs" and self.a["k_reorder"] > 0) else self.base
        if self.params is None:
            r = ix.search(q[None].astype(np.float32), k)[1][0]
        else:
            r = ix.search(q[None].astype(np.float32), k, params=self.params)[1][0]
        return np.where(r >= 0, self.ids[np.maximum(r, 0)], -1)


def timed(s, Q, reps):
    best = None
    for _ in range(reps):
        ids = np.full((len(Q), K), -1, np.int64)
        ts = np.zeros(len(Q))
        for i, q in enumerate(Q):
            t = time.perf_counter()
            r = s.search(q, K)
            ts[i] = time.perf_counter() - t
            ids[i, :len(r)] = r
        if best is None or ts.sum() < best[1].sum():
            best = (ids, ts)
    return best


def query(ds, out, lib, reps, filtered, nq, name, efs, only_builds=None, only_fracs=None, nprobes=None):
    Q = ds["test"][:nq] if nq else ds["test"]
    ds = dict(ds, test=Q, distances=ds["distances"][:len(Q)])
    fcsv = os.path.join(out, "baselines.csv")
    fresh = not os.path.exists(fcsv)
    f = open(fcsv, "a", newline="")
    w = csv.writer(f)
    if fresh:
        w.writerow(["dataset", "method", "build", "params", "filter", "nq", "reps", "qps", "mean_ms", "p50_ms", "p99_ms", "recall"])
    runs = [(b, None) for b in builds(lib) if not only_builds or b in only_builds] if filtered is None else \
        [(filtered, fr) for fr in FRACS if not only_fracs or fr in only_fracs]
    for b, fr in runs:
        s = Searcher(ds, out, lib, b)
        ftag = "all" if fr is None else tag(fr)
        if fr is not None:
            s.visible(np.load(os.path.join(ds["dir"], "filters", ftag + ".npy")))
        for a in qargs(lib, efs, nprobes):
            s.set(**a)
            ids, ts = timed(s, Q, reps)
            rec = recall(ds, ids, ftag)
            row = [name, lib, b, " ".join(f"{k}={v}" for k, v in a.items()), ftag, len(Q), reps, f"{len(Q) / ts.sum():.1f}",
                   f"{1e3 * ts.mean():.4f}", f"{1e3 * np.median(ts):.4f}", f"{1e3 * np.quantile(ts, 0.99):.4f}", f"{rec:.4f}"]
            w.writerow(row)
            f.flush()
            print(",".join(map(str, row)), flush=True)
        del s


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["build", "query"])
    ap.add_argument("data")
    ap.add_argument("out")
    ap.add_argument("lib", choices=["hnswlib", "faiss-hnsw", "faiss-ivfpqfs", "faiss-flat"])
    ap.add_argument("--threads", type=int, default=8)
    ap.add_argument("--reps", type=int, default=2)
    ap.add_argument("--filtered")
    ap.add_argument("--nq", type=int, default=0, help="first N test queries only")
    ap.add_argument("--efs", help="comma-separated ef values instead of the grid")
    ap.add_argument("--nprobes", help="comma-separated nprobe values instead of the grid")
    ap.add_argument("--builds", help="comma-separated builds of the unfiltered grid to run (default all)")
    ap.add_argument("--fracs", help="comma-separated visible fractions of a filtered run (default all)")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    ds = load(a.data)
    name = os.path.basename(os.path.normpath(a.data))
    if a.cmd == "build":
        build(ds, a.out, a.lib, a.threads)
    else:
        query(ds, a.out, a.lib, a.reps, a.filtered, a.nq, name, [int(e) for e in a.efs.split(",")] if a.efs else None,
              a.builds.split(",") if a.builds else None, [float(f) for f in a.fracs.split(",")] if a.fracs else None,
              [int(p) for p in a.nprobes.split(",")] if a.nprobes else None)


if __name__ == "__main__":
    main()
