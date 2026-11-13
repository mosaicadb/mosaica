# Fit the build parameters of the fast-scan index (v2) to a file of vectors, from the data alone: clusters with 8-bit
# centres, groups of clusters with local PCA bases, the data-only query-offset profile H25, the level schedule and the
# randomised blocks. The Rust build (`vexact --v2 --index OUTDIR`) encodes every item from these.
# usage: python3 -I fit.py VECTORS.npy OUTDIR [--metric angular] [--seed S]
#   VECTORS.npy: N x n, float16/32/64; row i is entity i.
# Writes to OUTDIR: Xs.npy (unit vectors in cluster order, f16, zero-padded to a multiple of 32 dimensions), perm.npy
# (row -> entity), cstart, grp, mq8, msc, lam, V, profile.npy, tree tables, qi (rows for verification), meta.json.
import argparse, json, math, os, sys, time
import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "combined"))
from core import kmeans, TREE, GTREE  # noqa: E402

# Design constants (not fitted): items per cluster, the guarantee's N and epsilon, the deterministic level ends, the
# randomised level ends up to 256 B, randomised block boundaries as fractions of n, the H25 hedge weight.
PER_CLUSTER = 1000
NTARGET, EPS = 1e9, 1e-9
DLV = [4, 8, 12, 16, 24, 32]
RLV_BASE = [40, 48, 56, 64, 72, 80, 96, 112, 128, 144, 160, 192, 224, 256]
BLOCK_FRACS = [0.125, 0.29167, 0.625]
HEDGE = 0.25

T0 = time.time()
def log(*a):
    print(f"[{time.time() - T0:6.0f}s]", *a, flush=True)


def schedule(n0, n):
    """Randomised level ends: the base list, then steps of 1.25x, while within 40% of the f16 vector, within 256 B
    per 768 dimensions and within 8 bits per dimension."""
    cap = min(0.4 * 2 * n0, 256 * max(1.0, n0 / 768), DLV[-1] + n)
    rlv = [b for b in RLV_BASE if b <= cap]
    b = RLV_BASE[-1]
    while True:
        b = int(round(b * 1.25 / 32)) * 32
        if b > cap:
            break
        rlv.append(b)
    bounds = [0] + [int(round(f * n / 16)) * 16 for f in BLOCK_FRACS] + [n]
    for j in range(1, len(bounds)):
        bounds[j] = max(bounds[j], bounds[j - 1] + 16)
    assert bounds[-2] <= n - 16, f"n = {n} is too small for four randomised blocks"
    return rlv, bounds


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("vectors")
    ap.add_argument("out")
    ap.add_argument("--metric", default="angular", choices=["angular"])
    ap.add_argument("--seed", type=int, default=20261112)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    rng = np.random.default_rng(a.seed)
    X = np.load(a.vectors, mmap_mode="r")
    N, n0 = X.shape
    n = (n0 + 31) // 32 * 32
    Xu = np.zeros((N, n), np.float32)
    for i in range(0, N, 100_000):
        x = np.asarray(X[i:i + 100_000], np.float32)
        nr = np.linalg.norm(x, axis=1, keepdims=True)
        Xu[i:i + len(x), :n0] = np.where(nr > 0, x / np.where(nr > 0, nr, 1), 0)
    nzero = int((np.abs(Xu).sum(1) == 0).sum())
    log(f"N {N} n {n0} (padded to {n}); zero vectors {nzero}")

    K = max(1, round(N / PER_CLUSTER))
    sub = Xu[rng.choice(N, min(N, 200_000), replace=False)]
    C = kmeans(sub, K, rng)
    del sub
    assign = np.concatenate([(Xu[i:i + 50_000] @ C.T).argmax(1) for i in range(0, N, 50_000)])
    keep = np.unique(assign)
    remap = -np.ones(K, np.int64)
    remap[keep] = np.arange(len(keep))
    assign = remap[assign]
    K = len(keep)
    cnt = np.bincount(assign, minlength=K)
    Msum = np.zeros((K, n))
    np.add.at(Msum, assign, Xu)
    Mean = Msum / cnt[:, None]
    msc = np.abs(Mean).max(1) / 127
    msc[msc == 0] = 1.0
    mq8 = np.round(Mean / msc[:, None]).astype(np.int8)
    Mq = (mq8 * msc[:, None]).astype(np.float32)
    mn = np.linalg.norm(Mq, axis=1)
    Mh = Mq / np.where(mn > 0, mn, 1)[:, None]

    # groups: enough items per group for a basis (20 n), at most 64 groups and 8 clusters per group on average
    G = int(max(1, min(64, K // 8, N // (20 * n0))))
    grp = (Mh @ kmeans(Mh.astype(np.float32).copy(), G, rng, 20).T).argmax(1)
    gk = np.unique(grp)
    gmap = -np.ones(G, np.int64)
    gmap[gk] = np.arange(len(gk))
    grp = gmap[grp]
    G = len(gk)
    log(f"K {K} clusters (sizes {cnt.min()}..{cnt.max()}), G {G} groups")

    # rows in (group, cluster) order; clusters renumbered so they are contiguous
    order = np.lexsort((assign, grp[assign]))
    assign = assign[order]
    first = assign[np.r_[0, np.where(np.diff(assign))[0] + 1]]
    newid = -np.ones(K, np.int64)
    newid[first] = np.arange(K)
    assign = newid[assign]
    mq8, msc, Mq, grp, cnt = mq8[first], msc[first], Mq[first], grp[first], cnt[first]
    cstart = np.r_[0, np.cumsum(cnt)]
    Xu = Xu[order]
    np.save(os.path.join(a.out, "Xs.npy"), Xu.astype(np.float16))
    np.save(os.path.join(a.out, "perm.npy"), order.astype(np.int64))

    lam = np.zeros((G, n))
    V = np.zeros((G, n, n), np.float32)
    gstart = np.searchsorted(grp[assign], np.arange(G + 1))
    for g in range(G):
        Cv = np.zeros((n, n))
        for i in range(gstart[g], gstart[g + 1], 50_000):
            j = min(i + 50_000, gstart[g + 1])
            R = (Xu[i:j] - Mq[assign[i:j]]).astype(np.float64)
            Cv += R.T @ R
        l, v = np.linalg.eigh(Cv / max(gstart[g + 1] - gstart[g], 1))
        lam[g] = np.maximum(l[::-1], 1e-12)
        V[g] = v[:, ::-1]
    log("bases done; group sizes", np.diff(gstart).min(), "..", np.diff(gstart).max())

    # data-only profile: corpus items as pseudo-queries against their 4 nearest centres, hedged towards isotropic
    # (spread over the n0 data dimensions; the padding has none)
    pq = Xu[rng.choice(N, min(N, 5000), replace=False)]
    pk = np.argsort(-(pq @ Mq.T), axis=1)[:, :4]
    Qi = np.zeros((G, n))
    for g in range(G):
        rows, cols = np.where(grp[pk] == g)
        Qi[g] = (((pq[rows] - Mq[pk[rows, cols]]) @ V[g]) ** 2).mean(0) if len(rows) > 50 else lam[g]
    iso = np.zeros((G, n))
    iso[:, :n0] = Qi.sum(1, keepdims=True) / n0
    prof = ((1 - HEDGE) * np.sqrt(Qi) + HEDGE * np.sqrt(iso)) ** 2

    rlv, bounds = schedule(n0, n)
    L1 = math.log(2 * NTARGET * len(rlv) / EPS)
    tree_edges = np.zeros((9, 256))
    tree_cent = np.zeros((9, 256))
    for b in range(1, 9):
        e, c = TREE[b]
        tree_edges[b, :len(e)] = e
        tree_cent[b, :len(c)] = c
    sv = lambda k, v: np.save(os.path.join(a.out, k + ".npy"), np.ascontiguousarray(v))
    sv("cstart", cstart.astype(np.int64))
    sv("grp", grp.astype(np.int64))
    sv("mq8", mq8)
    sv("msc", msc.astype(np.float64))
    sv("lam", lam)
    sv("V", V)
    sv("profile", prof)
    sv("tree_edges", tree_edges)
    sv("tree_cent", tree_cent)
    sv("gtree", GTREE.astype(np.float64))
    sv("qi", np.sort(rng.choice(N, min(N, 200), replace=False)).astype(np.int64))
    meta = dict(N=int(N), n=int(n), n_orig=int(n0), K=int(K), G=int(G), metric=a.metric, DLV=DLV, RLV=rlv, BOUNDS=bounds,
                CSG=math.sqrt(2 * L1), L1=L1, xs="Xs.npy", source=os.path.abspath(a.vectors))
    with open(os.path.join(a.out, "meta.json"), "w") as f:
        json.dump(meta, f, indent=1)
    log("saved", meta)


if __name__ == "__main__":
    main()
