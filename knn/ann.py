# Standard ANN baselines on the same rows, queries, filters and ground truth as the k-NN study.
# usage: python3 ann.py NAME WHAT   (WHAT in flat, fhnsw, ivfpq, hnswlib)
import sys, os, time, json
KNN = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/knn/"
sys.path.insert(0, KNN + "pylib")
import numpy as np, faiss
NAME, WHAT = sys.argv[1], sys.argv[2]; W_ = KNN + f"work_{NAME}/"
idx = (KNN + "../combined/idx_m768/") if NAME == "m768" else KNN + f"idx_{NAME}/"
I = np.load(idx + "index.npz"); qsel = np.load(W_ + "qsel.npy"); Q = np.ascontiguousarray(I["Q"][qsel].astype(np.float32)); qcls = I["qcls"][qsel]
NQ = len(Q); qrow = np.full(NQ, -1, np.int64)
if len(I["qi"]): qrow[qcls == "item"] = I["qi"][qsel[qcls == "item"]]
N, n = int(I["N"]), int(I["n"])
Sd = np.load(W_ + "seed.npz"); GTS = Sd["GTS"]; M = np.load(W_ + "masks.npy"); S = np.load(W_ + "S.npy", mmap_mode="r")
FN = ["all", "rand50", "rand10", "rand1", "rand0.1", "clusters10", "idrange10"]
CFG = [(1, 0), (10, 0), (100, 0)] + [(10, f) for f in range(1, 7)]
def loadX():
    Xs = np.load(idx + "Xs.npy", mmap_mode="r"); X = np.empty((N, n), np.float32)
    for i in range(0, N, 50_000):
        x = Xs[i:i+50_000].astype(np.float32); X[i:i+50_000] = x / np.linalg.norm(x, axis=1, keepdims=True)
    return X
OUT = open(KNN + f"ann_{NAME}.jsonl", "a")
def emit(**kw):
    OUT.write(json.dumps(kw) + "\n"); OUT.flush(); print(kw, flush=True)
def score(ids, k, f):
    """recall@k with ties, self removed; ids: NQ x (k+1) rows (-1 = missing)"""
    rec = np.zeros(NQ); got = np.zeros(NQ)
    for q in range(NQ):
        r = [i for i in ids[q] if i >= 0 and i != qrow[q]][:k]; got[q] = len(r)
        if r: rec[q] = (S[np.sort(r), q] >= GTS[f, q, k - 1] - 1e-6).sum() / k
    return {str(c): round(float(rec[qcls == c].mean()), 4) for c in np.unique(qcls)}, round(float(got.mean()), 2)
def sel_of(f):
    if f == 0: return None
    bm = np.packbits(M[f], bitorder="little"); s = faiss.IDSelectorBitmap(N, faiss.swig_ptr(bm)); s._keep = bm; return s

if WHAT == "flat":
    X = loadX(); ix = faiss.IndexFlatIP(n); ix.add(X); faiss.omp_set_num_threads(1)
    for k, f in CFG:
        p = faiss.SearchParameters(sel=sel_of(f)) if f else None
        t = time.time(); D, L = ix.search(Q, k + 1, params=p); t = (time.time() - t) / NQ
        rec, got = score(L, k, f); emit(index="flat", k=k, filter=FN[f], recall=rec, got=got, ms=round(1e3 * t, 3), ndis=int(M[f].sum()))
if WHAT == "fhnsw":
    Mh = int(os.environ.get("HM", 16)); fn = W_ + f"fhnsw{Mh}.index"
    if os.path.exists(fn): ix = faiss.read_index(fn); bt = -1
    else:
        X = loadX(); faiss.omp_set_num_threads(2); ix = faiss.IndexHNSWFlat(n, Mh, faiss.METRIC_INNER_PRODUCT); ix.hnsw.efConstruction = 200
        t = time.time(); ix.add(X); bt = time.time() - t; del X; faiss.write_index(ix, fn)
    emit(index="fhnsw", M=Mh, build_s=round(bt, 1), file_bytes=os.path.getsize(fn), bytes_per_item=round(os.path.getsize(fn) / N, 1))
    faiss.omp_set_num_threads(1); st = faiss.cvar.hnsw_stats
    for k, f in CFG:
        sel = sel_of(f)
        for ef in ([16, 32, 64, 128, 256, 512] if f == 0 else [64, 256, 1024, 4096]):
            if ef < k: continue
            p = faiss.SearchParametersHNSW(efSearch=ef) if f == 0 else faiss.SearchParametersHNSW(efSearch=ef, sel=sel)
            st.reset(); t = time.time(); D, L = ix.search(Q, k + 1, params=p); t = (time.time() - t) / NQ
            rec, got = score(L, k, f)
            emit(index="fhnsw", M=Mh, k=k, filter=FN[f], ef=ef, recall=rec, got=got, ms=round(1e3 * t, 3), ndis=round(st.ndis / NQ, 1), hops=round(st.nhops / NQ, 1))
if WHAT == "ivfpq":
    pm = int(os.environ.get("PM", 96)); nlist = int(os.environ.get("NLIST", 1024)); X = loadX(); faiss.omp_set_num_threads(2)
    qz = faiss.IndexFlatL2(n); ix = faiss.IndexIVFPQ(qz, n, nlist, pm, 8)
    t = time.time(); ix.train(X[np.random.default_rng(3).choice(N, min(N, 200_000), replace=False)]); ix.add(X); bt = time.time() - t
    emit(index="ivfpq", pm=pm, nlist=nlist, build_s=round(bt, 1), code_bytes_per_item=pm + 8)
    faiss.omp_set_num_threads(1); st = faiss.cvar.indexIVF_stats
    for k, f in CFG:
        sel = sel_of(f)
        for nprobe in [1, 4, 16, 64, 256]:
            for R in ([0, 10 * k + 100] if f == 0 else [10 * k + 100]):          # R = candidates re-ranked with full vectors
                p = faiss.SearchParametersIVF(nprobe=nprobe) if f == 0 else faiss.SearchParametersIVF(nprobe=nprobe, sel=sel)
                st.reset(); t = time.time(); D, L = ix.search(Q, max(R, k + 1), params=p)
                if R:
                    L2 = np.full((NQ, k + 1), -1, np.int64)
                    for q in range(NQ):
                        c = L[q][L[q] >= 0]
                        if len(c): L2[q, :min(k + 1, len(c))] = c[np.argsort(-(X[c] @ Q[q]))[:k + 1]]
                    L = L2
                t = (time.time() - t) / NQ; rec, got = score(L, k, f)
                emit(index="ivfpq", pm=pm, k=k, filter=FN[f], nprobe=nprobe, rerank=R, recall=rec, got=got, ms=round(1e3 * t, 3),
                     ndis=round(st.ndis / NQ, 1), nlist=nlist)
if WHAT == "hnswlib":
    import hnswlib
    Mh = int(os.environ.get("HM", 16)); fn = W_ + f"hnswlib{Mh}.bin"; ix = hnswlib.Index("ip", n)
    if os.path.exists(fn): ix.load_index(fn); bt = -1
    else:
        X = loadX(); ix.init_index(N, M=Mh, ef_construction=200); ix.set_num_threads(2)
        t = time.time(); ix.add_items(X, np.arange(N)); bt = time.time() - t; del X; ix.save_index(fn)
    emit(index="hnswlib", M=Mh, build_s=round(bt, 1), file_bytes=os.path.getsize(fn), bytes_per_item=round(os.path.getsize(fn) / N, 1))
    ix.set_num_threads(1)
    for k, f in CFG:
        for ef in ([16, 32, 64, 128, 256, 512] if f == 0 else [64, 256]):
            if ef < k + 1 or (f in (4, 5, 6) and ef > 64): continue   # selective filters: the search visits most of the graph
            ix.set_ef(ef); m = M[f]; L = np.full((NQ, k + 1), -1, np.int64); t = time.time()
            if f == 0: L, D = ix.knn_query(Q, k + 1, num_threads=1)
            else:
                fl = lambda i: bool(m[i])
                for q in range(NQ):
                    try: l, d = ix.knn_query(Q[q], k + 1, num_threads=1, filter=fl); L[q, :l.shape[1]] = l[0]
                    except RuntimeError:
                        for kk in range(k, 0, -1):
                            try: l, d = ix.knn_query(Q[q], kk, num_threads=1, filter=fl); L[q, :kk] = l[0]; break
                            except RuntimeError: pass
            t = (time.time() - t) / NQ; rec, got = score(L, k, f)
            emit(index="hnswlib", M=Mh, k=k, filter=FN[f], ef=ef, recall=rec, got=got, ms=round(1e3 * t, 3))
