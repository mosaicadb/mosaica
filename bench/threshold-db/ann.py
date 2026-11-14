# faiss, hnswlib and RaBitQ baselines for the threshold filter, unfiltered and within the 1% visible set.
# Approximate, at answer shares 0.01%, 0.1% and 1%: hnswlib and faiss HNSW as top-m with m the exact answer size, faiss
# IVF4096,Flat as a range search at the threshold, RaBitQ (rabitqlib IVF, 7 bits, every cluster probed) as top-m.
# Exact, at every share (0.01% to 75%): faiss IndexFlatIP and IndexFlatIPPanorama (8 levels, after an orthogonal PCA
# rotation of data and queries, as Panorama needs) as range searches at the threshold. Filtering: faiss through an IDSelectorBitmap, the flat index also over a
# compacted copy of the visible rows; hnswlib's Python binding filters only through a Python callback and rabitqlib has
# no filter, so neither runs filtered.
# usage: python3 -I ann.py build METHOD PREP_DIR ANN_DIR [--threads 4]
#        python3 -I ann.py query METHOD PREP_DIR ANN_DIR OUT_CSV [--step 10] [--reps 5] [--threads 1]
# Each setting runs at its default (top-m: ef = m; IVF: nprobe = 1) and at the tight one: the smallest ef (or nprobe) in
# a doubling ladder whose mean recall over the queries reaches 0.99 at that share. Rungs stop at ef = N and at the first
# rung whose median query exceeds --cap-ms; a ladder that ends short of 0.99 gives the last rung within the cap.
# Latency is one query at a time, 1 thread, timed from the call to the numpy ids.
import argparse, os, resource, sys, time
import numpy as np

ap = argparse.ArgumentParser()
ap.add_argument("cmd", choices=["build", "query"])
ap.add_argument("method", choices=["hnswlib", "faiss_hnsw", "ivf", "flat", "panorama", "rabitq"])
ap.add_argument("prep")
ap.add_argument("ann")
ap.add_argument("out", nargs="?")
ap.add_argument("--step", type=int, default=10)
ap.add_argument("--reps", type=int, default=5)
ap.add_argument("--vis", default="all,vis_1")
ap.add_argument("--threads", type=int, default=1)
ap.add_argument("--shares", default="")
ap.add_argument("--settings", default="default,tight", help="default, tight, or ladder (walk the ladder only, no timing)")
ap.add_argument("--tight", type=float, default=0, help="the tight rung, when known: skips the ladder")
ap.add_argument("--cap-ms", type=float, default=5000, help="the ladder stops at the first rung slower than this per query")
a = ap.parse_args()
os.makedirs(a.ann, exist_ok=True)
import faiss, hnswlib, rabitqlib
EXACT = a.method in ("flat", "panorama")

X = np.load(f"{a.prep}/corpus32.npy", mmap_mode="r")
N, n = X.shape
path = f"{a.ann}/{a.method}.bin"


def rss_gb():
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 2 ** 20


if a.cmd == "build":
    X = np.ascontiguousarray(X)
    faiss.omp_set_num_threads(a.threads)
    t = time.time()
    if a.method == "hnswlib":
        ix = hnswlib.Index(space="ip", dim=n)
        ix.init_index(max_elements=N, M=16, ef_construction=200, random_seed=1)
        ix.set_num_threads(a.threads)
        ix.add_items(X, np.arange(N))
        bt = time.time() - t
        ix.save_index(path)
    elif a.method == "faiss_hnsw":
        ix = faiss.IndexHNSWFlat(n, 16, faiss.METRIC_INNER_PRODUCT)
        ix.hnsw.efConstruction = 200
        ix.add(X)
        bt = time.time() - t
        faiss.write_index(ix, path)
    elif a.method == "panorama":
        # an orthogonal PCA rotation, applied to the data and to each query, keeps inner products and puts the
        # energy in the leading dimensions, as Panorama's pruning needs (faiss's PCAMatrix centres, which does not)
        rng = np.random.default_rng(1)
        S = X[np.sort(rng.choice(N, 200_000, replace=False))].astype(np.float64)
        w, V = np.linalg.eigh(np.cov(S, rowvar=False))
        V = np.ascontiguousarray(V[:, ::-1]).astype(np.float32)
        np.save(f"{a.ann}/panorama_rot.npy", V)
        ix = faiss.IndexFlatIPPanorama(n, 8)
        for i in range(0, N, 100_000):
            ix.add(np.ascontiguousarray(X[i:i + 100_000] @ V))
        bt = time.time() - t
        faiss.write_index(ix, path)
    elif a.method == "rabitq":
        ivf = faiss.read_index(f"{a.ann}/ivf.bin")  # its 4096 centres
        cent = ivf.quantizer.reconstruct_n(0, ivf.nlist)
        _, cid = ivf.quantizer.search(X, 1)
        t = time.time()
        ix = rabitqlib.IvfIndex(n, N, ivf.nlist, 7, "ip")
        ix.build(X, cent, cid[:, 0].astype(np.uint32), a.threads)
        bt = time.time() - t
        ix.save(path)
    else:
        q = faiss.IndexFlatIP(n)
        ix = faiss.IndexIVFFlat(q, n, 4096, faiss.METRIC_INNER_PRODUCT)
        rng = np.random.default_rng(1)
        ix.train(X[np.sort(rng.choice(N, 200_000, replace=False))])
        ix.add(X)
        bt = time.time() - t
        faiss.write_index(ix, path)
    print(f"{a.method}: built in {bt:.0f} s on {a.threads} threads; file {os.path.getsize(path) / 2 ** 30:.2f} GiB; "
          f"peak RSS {rss_gb():.2f} GiB", flush=True)
    sys.exit()

Q = np.load(f"{a.prep}/queries.npy").astype(np.float32)
T = np.load(f"{a.prep}/thresholds.npy")
order = np.load(f"{a.prep}/order.npy", mmap_mode="r")
ALL = (1e-4, 1e-3, 1e-2, 1e-1, 0.3, 0.5, 0.75)
ks = [round(s * N) for s in ALL]
shares = [int(x) for x in a.shares.split(",")] if a.shares else list(range(len(ks) if EXACT else 3))
qis = list(range(0, len(Q), a.step))
faiss.omp_set_num_threads(a.threads)
t = time.time()
if a.method == "flat":
    ix = faiss.IndexFlatIP(n)
    ix.add(np.ascontiguousarray(X))
elif a.method == "rabitq":
    ix = rabitqlib.IvfIndex.load(path)
elif a.method == "hnswlib":
    ix = hnswlib.Index(space="ip", dim=n)
    ix.load_index(path, max_elements=N)
    ix.set_num_threads(1)
else:
    ix = faiss.read_index(path)
ROT = np.load(f"{a.ann}/panorama_rot.npy") if a.method == "panorama" else None
print(f"{a.method}: loaded in {time.time() - t:.0f} s; RSS {rss_gb():.2f} GiB", flush=True)

rows = []
for vis in a.vis.split(","):
    sel, mask = None, None
    if vis != "all":
        ids = np.load(f"{a.prep}/{vis}.npy")
        mask = np.zeros(N, bool)
        mask[ids] = True
        bits = np.packbits(mask, bitorder="little")
        sel = faiss.IDSelectorBitmap(N, faiss.swig_ptr(bits))
        if a.method in ("hnswlib", "rabitq"):
            continue
        if a.method == "flat":
            t = time.time()
            compact = faiss.IndexFlatIP(n)
            compact.add(np.ascontiguousarray(X[ids]))
            print(f"flat: compacted copy of {vis} in {(time.time() - t) * 1e3:.0f} ms", flush=True)
    for si in shares:
        k = ks[si]
        exact = {}
        for qi in qis:
            e = np.asarray(order[qi, :k])
            exact[qi] = e[mask[e]] if mask is not None else e

        def one(qi, p):
            """ids for query qi at parameter p (ef, nprobe, or for the flat index under a visible set 0 = IDSelector and
            1 = the compacted copy)"""
            q = Q[qi:qi + 1]
            m = len(exact[qi])
            if a.method in ("flat", "panorama"):
                if p == 1:
                    _, D, I = compact.range_search(q, float(T[qi, si]))
                    return ids[I]
                ps = faiss.SearchParameters(sel=sel) if sel is not None else None
                if ROT is not None:
                    q = q @ ROT
                _, D, I = ix.range_search(q, float(T[qi, si]), params=ps)
                return I
            if a.method == "rabitq":
                return ix.search(q, m, p, True, 1)[0][0]
            if a.method == "hnswlib":
                ix.set_ef(p)
                return ix.knn_query(q, k=m)[0][0]
            if a.method == "faiss_hnsw":
                ps = faiss.SearchParametersHNSW(efSearch=p, sel=sel) if sel is not None else faiss.SearchParametersHNSW(efSearch=p)
                _, I = ix.search(q, m, params=ps)
                return I[0][I[0] >= 0]
            ps = faiss.SearchParametersIVF(nprobe=p, sel=sel) if sel is not None else faiss.SearchParametersIVF(nprobe=p)
            lims, D, I = ix.range_search(q, float(T[qi, si]), params=ps)
            return I

        def evaluate(mult, timed):
            res = []
            for qi in qis:
                m = len(exact[qi])
                if m == 0:
                    continue
                p = mult if a.method in ("ivf", "flat", "panorama", "rabitq") else int(np.ceil(m * mult))
                ts = []
                for _ in range(a.reps if timed else 1):
                    t0 = time.perf_counter()
                    got = one(qi, p)
                    ts.append((time.perf_counter() - t0) * 1e3)
                hit = len(np.intersect1d(got, exact[qi]))
                res.append((qi, p, float(np.median(ts)), hit / m, len(got), len(got) - hit, m))
            return res

        if EXACT:
            for setting, p in (("idselector", 0), ("compacted", 1)) if sel is not None and a.method == "flat" else (("exact", 0),):
                r = evaluate(p, True)
                for qi, _, ms, rec, ret, bad, m in r:
                    rows.append(f"{a.method},{setting},{vis},{si},{qi},{a.threads},{ms:.4f},{rec:.5f},{ret},{bad},{m}")
                print(f"{a.method} {setting} {vis} share {si} threads {a.threads}: median {np.median([x[2] for x in r]):.2f} ms, "
                      f"mean recall {np.mean([x[3] for x in r]):.5f}, non-matches {sum(x[5] for x in r)}", flush=True)
            continue
        if a.method == "rabitq":
            r = evaluate(4096, True)
            for qi, p, ms, rec, ret, bad, m in r:
                rows.append(f"{a.method},all_clusters,{vis},{si},{qi},{p},{ms:.4f},{rec:.5f},{ret},{bad},{m}")
            print(f"rabitq nprobe 4096 {vis} share {si}: median {np.median([x[2] for x in r]):.2f} ms, mean recall "
                  f"{np.mean([x[3] for x in r]):.5f}, min {np.min([x[3] for x in r]):.5f}", flush=True)
            continue
        ladder = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096] if a.method == "ivf" else \
            [1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24, 32, 64, 128, 256, 512, 1024]
        if a.method != "ivf":  # ef at most N
            mmax = max(len(e) for e in exact.values())
            ladder = [x for x in ladder if mmax * x <= N] or [1]
        tight = a.tight
        if not tight:
            tight = ladder[0]
            for mult in ladder:
                r = evaluate(mult, False)
                rec, ms = np.mean([x[3] for x in r]), np.median([x[2] for x in r])
                print(f"{a.method} ladder {vis} share {si}: rung {mult}, mean recall {rec:.4f}, median {ms:.1f} ms", flush=True)
                if ms > a.cap_ms:
                    break
                tight = mult
                if rec >= 0.99:
                    break
        for setting, mult in (("default", 1), ("tight", tight)):
            if setting not in a.settings.split(","):
                continue
            r = evaluate(mult, True)
            for qi, p, ms, rec, ret, bad, m in r:
                rows.append(f"{a.method},{setting},{vis},{si},{qi},{p},{ms:.4f},{rec:.5f},{ret},{bad},{m}")
            if r:
                print(f"{a.method} {setting} {vis} share {si}: param {'x' if a.method != 'ivf' else ''}{mult}, "
                      f"median {np.median([x[2] for x in r]):.2f} ms, mean recall {np.mean([x[3] for x in r]):.4f}, "
                      f"non-matches {sum(x[5] for x in r)} over {len(r)} queries", flush=True)
new = rows and not os.path.exists(a.out)
with open(a.out, "a") if rows else open(os.devnull, "w") as f:
    f.write(("method,setting,vis,share_index,query,param,median_ms,recall,returned,nonmatch,exact\n" if new else "")
            + "\n".join(rows) + "\n")
print(f"peak RSS {rss_gb():.2f} GiB", flush=True)
