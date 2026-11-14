# Top-k latency of the approximate libraries at the cheapest setting reaching each mean recall target, unfiltered,
# 1 thread, on the threshold-db timing queries (every --step-th of the 150). Recall@k is the share of the first k of
# order.npy (ids by f64 similarity) returned. Ladders as in ../threshold-db/ann.py: HNSW ef = ceil(k x mult) over
# mult in {1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24, 32, 64, ..., 1024}, then doubling, with ef at most N; IVF and RaBitQ nprobe doubling
# 1..4096. A ladder stops at the first rung whose median query exceeds --cap-ms; a target not reached by then is
# reported at the last rung within the cap, with reached = 0.
# usage: python3 -I ann_topk.py METHOD PREP_DIR ANN_DIR OUT_CSV [--ks 1,10,100,1000] [--targets 0.8,0.99]
import argparse, time
import numpy as np

ap = argparse.ArgumentParser()
ap.add_argument("method", choices=["hnswlib", "faiss_hnsw", "hnsw_sq16", "ivf", "ivf_sq16", "rabitq"])
ap.add_argument("prep")
ap.add_argument("ann")
ap.add_argument("out")
ap.add_argument("--ks", default="1,10,100,1000")
ap.add_argument("--targets", default="0.8,0.99")
ap.add_argument("--step", type=int, default=10)
ap.add_argument("--reps", type=int, default=5)
ap.add_argument("--cap-ms", type=float, default=5000)
a = ap.parse_args()
import faiss, hnswlib, rabitqlib

faiss.omp_set_num_threads(1)
Q = np.load(f"{a.prep}/queries.npy").astype(np.float32)
order = np.load(f"{a.prep}/order.npy", mmap_mode="r")
N = np.load(f"{a.prep}/corpus32.npy", mmap_mode="r").shape[0]
qis = list(range(0, len(Q), a.step))
path = f"{a.ann}/{a.method}.bin"
t = time.time()
if a.method == "rabitq":
    ix = rabitqlib.IvfIndex.load(path)
elif a.method == "hnswlib":
    ix = hnswlib.Index(space="ip", dim=Q.shape[1])
    ix.load_index(path, max_elements=N)
    ix.set_num_threads(1)
else:
    ix = faiss.read_index(path)
print(f"{a.method}: loaded in {time.time() - t:.0f} s", flush=True)
HNSW = a.method in ("faiss_hnsw", "hnsw_sq16", "hnswlib")


def one(q, k, p):
    if a.method == "rabitq":
        return ix.search(q, k, p, True, 1)[0][0]
    if a.method == "hnswlib":
        ix.set_ef(max(p, k))
        return ix.knn_query(q, k=k)[0][0]
    ps = faiss.SearchParametersHNSW(efSearch=p) if HNSW else faiss.SearchParametersIVF(nprobe=p)
    _, I = ix.search(q, k, params=ps)
    return I[0][I[0] >= 0]


def evaluate(k, p, reps):
    res = []
    for qi in qis:
        q = Q[qi:qi + 1]
        ts = []
        for _ in range(reps):
            t0 = time.perf_counter()
            got = one(q, k, p)
            ts.append((time.perf_counter() - t0) * 1e3)
        res.append((qi, float(np.median(ts)), len(np.intersect1d(got, order[qi, :k])) / k))
    return res


targets = [float(x) for x in a.targets.split(",")]
rows = []
for k in [int(x) for x in a.ks.split(",")]:
    if HNSW:
        ladder = [x for x in [1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24, 32, 64, 128, 256, 512, 1024] if k * x <= N]
        params = sorted({int(np.ceil(k * x)) for x in ladder})
        while params[-1] * 2 <= N:  # past mult 1024, keep doubling ef up to N (it matters for small k)
            params.append(params[-1] * 2)
    else:
        params = [1 << i for i in range(13)]
    pending = list(targets)
    last = None
    for p in params:
        r = evaluate(k, p, 1)
        rec, ms = np.mean([x[2] for x in r]), np.median([x[1] for x in r])
        print(f"{a.method} k {k} ladder: param {p}, mean recall {rec:.4f}, median {ms:.2f} ms", flush=True)
        if ms > a.cap_ms:
            break
        last = p
        hit = [tg for tg in pending if rec >= tg]
        if hit:
            r = evaluate(k, p, a.reps)
            for tg in hit:
                pending.remove(tg)
                rows += [(a.method, tg, k, qi, p, ms_, rc, 1) for qi, ms_, rc in r]
                print(f"{a.method} k {k} target {tg}: param {p}, median {np.median([x[1] for x in r]):.3f} ms, "
                      f"mean recall {np.mean([x[2] for x in r]):.4f}", flush=True)
        if not pending:
            break
    if pending and last is not None:
        r = evaluate(k, last, a.reps)
        for tg in pending:
            rows += [(a.method, tg, k, qi, last, ms_, rc, 0) for qi, ms_, rc in r]
            print(f"{a.method} k {k} target {tg}: NOT reached; last rung {last}, median "
                  f"{np.median([x[1] for x in r]):.3f} ms, mean recall {np.mean([x[2] for x in r]):.4f}", flush=True)
import os
new = not os.path.exists(a.out)
with open(a.out, "a") as f:
    if new:
        f.write("method,target,k,query,param,median_ms,recall,reached\n")
    f.write("".join(f"{m},{tg},{k},{qi},{p},{ms:.4f},{rc:.5f},{ok}\n" for m, tg, k, qi, p, ms, rc, ok in rows))
