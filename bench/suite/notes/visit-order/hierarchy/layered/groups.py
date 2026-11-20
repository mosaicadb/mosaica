"""Replay C for layered.md: what the number of groups does to the code, on the suite's real fits. Counts only.

A proxy, not the encoder: for each grouping of the fit's clusters (k-means on the centre directions, as fit.py groups
them; G from 1 to K), each group's basis is the eigenbasis of its pooled residual covariance, bits are given to its
coordinates by reverse water-filling on the pooled variances (the same bits for every cluster of the group, as the
build allocates per group), and a cluster's distortion is sum_i v_ci 2^(-2 b_i), v_ci its own residual variance along
the group's coordinate i. Reported per G: the share f of the clusters' residual variance a 16 B and a 32 B code
explains (mean over clusters, weighted by rows), and the bits per row a code would need for the distortion the
G = 64 grouping reaches at 16 B (how many bytes a finer or coarser grouping is worth).

usage: python3 groups.py DATASET(glove|yahoo|laion)
"""
import sys

import numpy as np

FIT = "/home/user/suite-data/work"
NAMES = {"glove": "glove-100-angular", "yahoo": "yahoo-minilm-384-normalized", "laion": "laion-clip-512-normalized"}


def alloc(lam, bits):
    """Reverse water-filling: b_i = max(0, log2(lam_i / theta) / 2) with sum b_i = bits."""
    lam = np.maximum(lam, 1e-30)
    lo, hi = 1e-30, lam.max()
    for _ in range(200):
        th = np.sqrt(lo * hi)
        b = np.maximum(0, 0.5 * np.log2(lam / th)).sum()
        lo, hi = (lo, th) if b < bits else (th, hi)
    return np.maximum(0, 0.5 * np.log2(lam / np.sqrt(lo * hi)))


def main():
    import faiss
    ds = sys.argv[1]
    fit = f"{FIT}/{NAMES[ds]}/vexact/index"
    Xs = np.load(f"{fit}/Xs.npy", mmap_mode="r")
    cs = np.load(f"{fit}/cstart.npy")
    cen = np.load(f"{fit}/mq8.npy").astype(np.float64) * np.load(f"{fit}/msc.npy")[:, None]
    grp = np.load(f"{fit}/grp.npy")
    K, n = cen.shape
    cnt = np.diff(cs).astype(np.float64)
    cov = np.empty((K, n, n), np.float32)
    for c in range(K):
        R = np.asarray(Xs[cs[c]:cs[c + 1]], dtype=np.float32) - cen[c].astype(np.float32)
        cov[c] = R.T @ R / max(len(R), 1)
    tr = np.einsum("cii->c", cov).astype(np.float64)
    dirs = np.ascontiguousarray(cen / np.linalg.norm(cen, axis=1, keepdims=True), dtype=np.float32)
    print(f"### {ds}: K {K}, n {n}; residual variance explained by a code in the group's basis, by the number of groups\n")
    print("| groups | clusters per group | f at 16 B | f at 32 B | bits a row for G = 64's distortion at 16 B (128 bits there) | int8 bases, MB |")
    print("|---|---|---|---|---|---|")
    out = {}
    gs = [1, 2, 4, 8, 16, 32, 64, 128, 250, K]
    for G in gs:
        if G > K:
            continue
        if G == K:
            a = np.arange(K)
        elif G == 64 and len(np.unique(grp)) == 64:
            a = grp
        elif G == 1:
            a = np.zeros(K, np.int64)
        else:
            km = faiss.Kmeans(n, G, niter=20, seed=7, spherical=True, min_points_per_centroid=1, max_points_per_centroid=10 ** 6)
            km.train(dirs)
            a = km.index.search(dirs, 1)[1][:, 0]
        v = np.empty((K, n))
        lam = np.empty((K, n))
        for g in np.unique(a):
            idx = np.flatnonzero(a == g)
            pooled = np.tensordot(cnt[idx] / cnt[idx].sum(), cov[idx].astype(np.float64), axes=1)
            w, U = np.linalg.eigh(pooled)
            w, U = w[::-1], U[:, ::-1]
            for c in idx:
                v[c] = np.einsum("ij,ij->j", cov[c].astype(np.float64) @ U, U)
                lam[c] = w
        out[G] = (a, v, lam)
    wts = cnt / cnt.sum()

    def dist(G, bits):
        a, v, lam = out[G]
        d = np.empty(K)
        for g in np.unique(a):
            idx = np.flatnonzero(a == g)
            b = alloc(lam[idx[0]], bits)
            d[idx] = (v[idx] * 2.0 ** (-2 * b)).sum(1)
        return d

    ref = float((wts * dist(64 if 64 in out else gs[0], 128) / tr).sum())
    for G in out:
        f16 = 1 - (wts * dist(G, 128) / tr).sum()
        f32 = 1 - (wts * dist(G, 256) / tr).sum()
        lo, hi = 8.0, 2048.0
        for _ in range(40):
            mid = 0.5 * (lo + hi)
            lo, hi = (lo, mid) if (wts * dist(G, mid) / tr).sum() < ref else (mid, hi)
        print(f"| {G} | {K / G:.0f} | {f16:.3f} | {f32:.3f} | {0.5 * (lo + hi):.0f} | {G * n * n / 1e6:.1f} |")


if __name__ == "__main__":
    main()
