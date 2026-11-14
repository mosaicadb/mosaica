# Cluster a file of vectors for the fit (`vexact fit`): spherical k-means with faiss to about PER_CLUSTER items per
# cluster, then every row assigned to its nearest centre over all centres.
# usage: python3 -I cluster.py VECTORS.npy OUTDIR [--cells C] [--per-cluster 1000] [--sample-per-centre 200] [--seed S]
#   VECTORS.npy: N x n, float16/32/64; row i is entity i.
#   --cells C: train in two levels, C coarse cells with fine centres trained per cell in proportion to the cell's share
#     of the sample. The default, 1, trains the centres in one k-means. Assignment is over all centres either way.
# Writes to OUTDIR: centres.npy (K x n float32, unit length, in the vectors' own dimension) and assign.npy (N, uint32:
# the nearest centre of each row by inner product of the unit vectors).
import argparse, os, time
import numpy as np
import faiss

T0 = time.time()
CHUNK = 100_000


def log(*a):
    print(f"[{time.time() - T0:6.0f}s]", *a, flush=True)


def unit(x):
    x = np.asarray(x, np.float32)
    nr = np.linalg.norm(x, axis=1, keepdims=True)
    return np.where(nr > 0, x / np.where(nr > 0, nr, 1), 0).astype(np.float32)


def kmeans(S, k, seed, niter):
    km = faiss.Kmeans(S.shape[1], k, niter=niter, spherical=True, seed=seed, min_points_per_centroid=1,
                      max_points_per_centroid=1 << 30, verbose=False)
    km.train(S)
    return km.centroids


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("vectors")
    ap.add_argument("out")
    ap.add_argument("--cells", type=int, default=1)
    ap.add_argument("--per-cluster", type=int, default=1000)
    ap.add_argument("--sample-per-centre", type=int, default=200)
    ap.add_argument("--niter", type=int, default=20)
    ap.add_argument("--seed", type=int, default=20261112)
    ap.add_argument("--refine", type=int, default=0,
                    help="after two-level training, this many Lloyd steps on the sample over all centres")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    X = np.load(a.vectors, mmap_mode="r")
    N, n = X.shape
    K = max(1, round(N / a.per_cluster))
    rng = np.random.default_rng(a.seed)
    idx = np.sort(rng.choice(N, min(N, a.sample_per_centre * K), replace=False))
    S = unit(X[idx])
    log(f"N {N} n {n}: K {K} centres from a sample of {len(S)}, {a.cells} cell(s)")
    if a.cells <= 1:
        C = kmeans(S, K, a.seed, a.niter)
    else:
        coarse = kmeans(S, a.cells, a.seed, a.niter)
        cell = faiss.IndexFlatIP(n)
        cell.add(coarse)
        cs = cell.search(S, 1)[1][:, 0]
        cnt = np.bincount(cs, minlength=a.cells)
        # fine centres per cell in proportion to its share of the sample, at least one for a non-empty cell
        kc = np.where(cnt > 0, np.maximum(1, np.round(K * cnt / len(S))), 0).astype(int)
        parts = []
        for c in range(a.cells):
            if kc[c] == 0:
                continue
            Sc = S[cs == c]
            parts.append(kmeans(Sc, min(kc[c], len(Sc)), a.seed + c, a.niter))
        C = np.concatenate(parts)
        log(f"cells: sizes {cnt.min()}..{cnt.max()} of the sample; {len(C)} fine centres")
        for _ in range(a.refine):
            C = unit(C)
            ix = faiss.IndexFlatIP(n)
            ix.add(C)
            sa = ix.search(S, 1)[1][:, 0]
            M = np.zeros_like(C)
            np.add.at(M, sa, S)
            M[np.bincount(sa, minlength=len(C)) == 0] = C[np.bincount(sa, minlength=len(C)) == 0]
            C = M
        if a.refine:
            log(f"refined {a.refine} steps over all {len(C)} centres")
    C = unit(C)
    index = faiss.IndexFlatIP(n)
    index.add(C)
    assign = np.empty(N, np.uint32)
    for i in range(0, N, CHUNK):
        assign[i:i + CHUNK] = index.search(unit(X[i:i + CHUNK]), 1)[1][:, 0]
    used = np.bincount(assign, minlength=len(C))
    log(f"assigned {N} rows to {int((used > 0).sum())} of {len(C)} centres (sizes {used[used > 0].min()}..{used.max()})")
    np.save(os.path.join(a.out, "centres.npy"), C)
    np.save(os.path.join(a.out, "assign.npy"), assign)


if __name__ == "__main__":
    main()
