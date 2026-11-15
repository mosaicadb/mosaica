# Partition a file of vectors into cells for the fit and the build (`vexact fit OUT --partition DIR`), with faiss:
# coarse cells, fine clusters of about PER_CLUSTER items trained per cell, every row assigned to a fine centre.
# Nothing holds more than a chunk of the vectors and one cell's training sample at a time.
# usage: python3 -I partition.py VECTORS.npy OUTDIR [--cells C] [--cross T] [--per-cluster 1000]
#        [--sample-per-centre 200] [--cell-sample 1000] [--niter 20] [--seed S]
#   VECTORS.npy: N x n0, float16/32/64; row i is entity i.
#   --cells C: coarse cells (default 1: one cell, one k-means over a sample of the whole set).
#   --cross T: each row is assigned to its nearest fine centre among those of its cell's T nearest cells (default 1;
#     T >= C is exact assignment over all centres). A row whose centre lies in another cell moves to that cell.
# Writes to OUTDIR, per cell c: cell{c}.f16 (unit rows, float16, zero-padded to n, a multiple of 32), cell{c}.ids
# (uint64 source rows) and cell{c}.cl (uint32 fine cluster of each row); the rows that moved into cell c from other
# cells in cell{c}.in.f16, cell{c}.in.ids and cell{c}.in.cl. Also centres.npy (K x n float32, unit fine centres,
# numbered cell by cell), cluster_cell.npy (K, uint32) and manifest.json.
import argparse, json, os, time
import numpy as np
import faiss

T0 = time.time()
CHUNK = 100_000


def log(*a):
    print(f"[{time.time() - T0:6.0f}s]", *a, flush=True)


def unit(x, n):
    x = np.asarray(x, np.float32)
    nr = np.linalg.norm(x, axis=1, keepdims=True)
    u = np.zeros((len(x), n), np.float32)
    u[:, :x.shape[1]] = np.where(nr > 0, x / np.where(nr > 0, nr, 1), 0)
    return u


def kmeans(S, k, seed, niter):
    km = faiss.Kmeans(S.shape[1], k, niter=niter, spherical=True, seed=seed, min_points_per_centroid=1,
                      max_points_per_centroid=1 << 30, verbose=False)
    km.train(np.ascontiguousarray(S, np.float32))
    return km.centroids


def nearest(C, X):
    index = faiss.IndexFlatIP(C.shape[1])
    index.add(np.ascontiguousarray(C, np.float32))
    return index.search(np.ascontiguousarray(X, np.float32), 1)[1][:, 0]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("vectors")
    ap.add_argument("out")
    ap.add_argument("--cells", type=int, default=1)
    ap.add_argument("--cross", type=int, default=1)
    ap.add_argument("--per-cluster", type=int, default=1000)
    ap.add_argument("--sample-per-centre", type=int, default=200)
    ap.add_argument("--cell-sample", type=int, default=1000)
    ap.add_argument("--niter", type=int, default=20)
    ap.add_argument("--seed", type=int, default=20261112)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    X = np.load(a.vectors, mmap_mode="r")
    N, n0 = X.shape
    n = (n0 + 31) // 32 * 32
    K = max(1, round(N / a.per_cluster))
    C = max(1, min(a.cells, K))
    rng = np.random.default_rng(a.seed)
    f = lambda name: os.path.join(a.out, name)
    log(f"N {N} n {n0} (padded to {n}): K {K} clusters in {C} cell(s), cross {a.cross}")

    # coarse cells from a sample, then every row to its cell's bucket
    if C > 1:
        idx = np.sort(rng.choice(N, min(N, a.cell_sample * C), replace=False))
        coarse = kmeans(unit(X[idx], n), C, a.seed, a.niter)
    files = [(open(f(f"cell{c}.f16"), "wb"), open(f(f"cell{c}.ids"), "wb")) for c in range(C)]
    rows = np.zeros(C, np.int64)
    for i in range(0, N, CHUNK):
        U = unit(X[i:i + CHUNK], n)
        cell = nearest(coarse, U) if C > 1 else np.zeros(len(U), np.int64)
        order = np.argsort(cell, kind="stable")
        bounds = np.searchsorted(cell[order], np.arange(C + 1))
        U16, ids = U[order].astype(np.float16), (i + order).astype(np.uint64)
        for c in np.nonzero(np.diff(bounds))[0]:
            s, e = bounds[c], bounds[c + 1]
            files[c][0].write(U16[s:e].tobytes())
            files[c][1].write(ids[s:e].tobytes())
        rows += np.diff(bounds)
    for fv, fi in files:
        fv.close(), fi.close()
    log(f"pass 1: cells of {rows.min()}..{rows.max()} rows")

    cellrows = lambda c: np.memmap(f(f"cell{c}.f16"), np.float16, "r", shape=(int(rows[c]), n)) if rows[c] else np.zeros((0, n), np.float16)

    # fine centres per cell, in proportion to its rows, from a sample of the cell
    kc = np.where(rows > 0, np.maximum(1, np.round(K * rows / N)), 0).astype(np.int64)
    parts = []
    for c in range(C):
        if kc[c] == 0:
            continue
        m = int(min(rows[c], a.sample_per_centre * kc[c]))
        sidx = np.sort(rng.choice(int(rows[c]), m, replace=False))
        parts.append(kmeans(cellrows(c)[sidx].astype(np.float32), int(min(kc[c], m)), a.seed + 1 + c, a.niter))
    cent = np.concatenate(parts).astype(np.float32)
    cent /= np.maximum(np.linalg.norm(cent, axis=1, keepdims=True), 1e-30)
    ccell = np.repeat(np.nonzero(kc)[0], [len(p) for p in parts]).astype(np.uint32)
    first = np.searchsorted(ccell, np.arange(C + 1))
    log(f"{len(cent)} fine centres, {kc.min()}..{kc.max()} per cell")

    # every row to its nearest fine centre among its cell's T nearest cells; rows of another cell's centre move there
    near = (np.argsort(-(coarse @ coarse.T), axis=1)[:, :a.cross] if C > 1 else np.zeros((1, 1), np.int64))
    moved = 0
    outs = {}
    for c in range(C):
        R = cellrows(c)
        cand = np.concatenate([np.arange(first[d], first[d + 1]) for d in sorted(near[c])])
        cl = np.empty(int(rows[c]), np.uint32)
        for i in range(0, int(rows[c]), CHUNK):
            cl[i:i + CHUNK] = cand[nearest(cent[cand], R[i:i + CHUNK].astype(np.float32))]
        cl.tofile(f(f"cell{c}.cl"))
        away = np.nonzero(ccell[cl] != c)[0]
        if len(away):
            ids = np.fromfile(f(f"cell{c}.ids"), np.uint64)
            dest = ccell[cl[away]]
            for d in np.unique(dest):
                sel = away[dest == d]
                if d not in outs:
                    outs[d] = tuple(open(f(f"cell{d}.in.{x}"), "wb") for x in ("f16", "ids", "cl"))
                outs[d][0].write(np.ascontiguousarray(R[sel]).tobytes())
                outs[d][1].write(ids[sel].tobytes())
                outs[d][2].write(cl[sel].tobytes())
            moved += len(away)
    for fs in outs.values():
        for x in fs:
            x.close()
    log(f"pass 2: {moved} rows ({moved / N:.2%}) moved to a centre in another cell")

    np.save(f("centres.npy"), cent)
    np.save(f("cluster_cell.npy"), ccell)
    src = os.path.abspath(a.vectors)
    st = os.stat(src)
    json.dump(dict(N=int(N), n=int(n), n_orig=int(n0), K=int(len(cent)), cells=int(C), cross=int(a.cross),
                   seed=a.seed, source=src, source_bytes=st.st_size, source_mtime=st.st_mtime,
                   rows=[int(r) for r in rows], moved=int(moved)), open(f("manifest.json"), "w"), indent=1)
    log(f"wrote {a.out}")


if __name__ == "__main__":
    main()
