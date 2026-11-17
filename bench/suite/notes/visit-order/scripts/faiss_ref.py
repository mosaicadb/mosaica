# faiss IVF as the suite's faiss-ivf-disk trains it (bench/suite/systems/faiss_sys.py: IndexFlatIP quantizer, inner
# product, trained on 200,000 rows drawn with default_rng(1)), at several nlist; the lists are the corpus rows'
# argmax centroid (what add() does), the probe order q.c over the centroids. Recall@10 against rows scanned.
# usage: python -I faiss_ref.py PREPDIR OUT.json NLIST [NLIST ...]
import sys, os, json, time
import numpy as np
import faiss

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common_r import curve, summarise  # noqa: E402

prep, outp = sys.argv[1], sys.argv[2]
nlists = [int(x) for x in sys.argv[3:]]
t0 = time.time()
log = lambda *a: print(f"[{time.time() - t0:5.0f}s]", *a, flush=True)
X = np.load(os.path.join(prep, "corpus16.npy"), mmap_mode="r")
N, d = X.shape
rng = np.random.default_rng(1)
tr = np.ascontiguousarray(X[np.sort(rng.choice(N, min(N, 200_000), replace=False))], dtype=np.float32)
Q = np.load(os.path.join(prep, "queries.npy")).astype(np.float32)
gt = np.load(os.path.join(prep, "order.npy"))[:, :10]
res = {}
for nlist in nlists:
    q = faiss.IndexFlatIP(d)
    ix = faiss.IndexIVFFlat(q, d, nlist, faiss.METRIC_INNER_PRODUCT)
    ix.train(tr)
    C = q.reconstruct_n(0, nlist)
    a = np.concatenate([q.search(np.ascontiguousarray(X[i:i + 100_000], dtype=np.float32), 1)[1][:, 0]
                        for i in range(0, N, 100_000)])
    sizes = np.bincount(a, minlength=nlist)
    # faiss's own search on the test queries at a few nprobe, as a check of the counting (IVF-Flat, f32 lists)
    rec, items = curve(Q.astype(np.float64) @ C.T.astype(np.float64), a[gt], sizes)
    s = summarise(rec, items, N)
    s.update(nlist=nlist, sizes=dict(min=int(sizes.min()), max=int(sizes.max()), median=float(np.median(sizes)),
                                     empty=int((sizes == 0).sum())), centroid_norm=float(np.linalg.norm(C, axis=1).mean()))
    if nlist == 4096:
        for i in range(0, N, 100_000):
            ix.add(np.ascontiguousarray(X[i:i + 100_000], dtype=np.float32))
        chk = {}
        for p in (16, 64, 256):
            _, I = ix.search(Q, 10, params=faiss.SearchParametersIVF(nprobe=p))
            chk[str(p)] = float(np.mean([len(set(I[j]) & set(gt[j])) / 10 for j in range(len(Q))]))
        s["faiss_search_check"] = chk
        log(f"nlist {nlist}: faiss's own search recall at nprobe 16/64/256: {chk}")
        del ix
    res[str(nlist)] = s
    log(f"nlist {nlist}: sizes {sizes.min()}..{sizes.max()} (median {np.median(sizes):.0f}, empty {(sizes == 0).sum()}), "
        f"|c| {np.linalg.norm(C, axis=1).mean():.3f}; " + " ".join(f"{k}:{v:.3f}" for k, v in s["fixed"].items()) +
        f" | nprobe {', '.join(f'{p}: {v['recall']:.3f} ({v['share'] * 100:.2f}%)' for p, v in s['nprobe'].items())}"
        f" | zero@{s['zero16']['share'] * 100:.2f}%: {s['zero16']['n_zero']}")
json.dump(res, open(outp, "w"), indent=1)
log("saved", outp)
