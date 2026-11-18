# Prepare the replay state for hier.py: for one dataset of the suite, the exact scores of every query against every
# row (in the fit's row order), and nested sub-clusterings of every cluster of the fit (4, 16, 64 leaves per cluster by
# recursive 4-way k-means, plus a direct 16-way k-means), with the leaf centres.
#   python3 prep.py DATASET OUT_DIR
# Reads /home/user/suite-data/prep/DATASET (queries.npy) and /home/user/suite-data/work/DATASET/vexact/index (Xs.npy,
# cstart.npy). Nothing is timed. Seeds are fixed.
import sys, os, numpy as np
ds, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
P = f"/home/user/suite-data/prep/{ds}/"
I = f"/home/user/suite-data/work/{ds}/vexact/index/"
Xs = np.load(I + "Xs.npy", mmap_mode="r")
N, n = Xs.shape
X = np.empty((N, n), np.float32)
for i in range(0, N, 100_000):
    X[i:i + 100_000] = Xs[i:i + 100_000]
cstart = np.load(I + "cstart.npy")
K = len(cstart) - 1
Q = np.load(P + "queries.npy").astype(np.float32)
Q = np.pad(Q, ((0, 0), (0, n - Q.shape[1])))
S = np.empty((len(Q), N), np.float32)
for i in range(0, N, 100_000):
    S[:, i:i + 100_000] = Q @ X[i:i + 100_000].T
np.save(out + "/S.npy", S)
np.save(out + "/Q.npy", Q)
rng = np.random.default_rng(1)


def km(R, k, it=8):
    """Lloyd's k-means (Euclidean) on the rows of R; returns the assignment. With k rows or fewer: one leaf each."""
    m = len(R)
    if m <= k:
        return np.arange(m)
    C = R[rng.choice(m, k, replace=False)].copy()
    r2 = (R * R).sum(1)
    for _ in range(it):
        a = (r2[:, None] - 2 * R @ C.T + (C * C).sum(1)[None]).argmin(1)
        cnt = np.bincount(a, minlength=k)
        Sm = np.zeros_like(C)
        np.add.at(Sm, a, R)
        ok = cnt > 0
        C[ok] = Sm[ok] / cnt[ok, None]
        if (~ok).any():
            C[~ok] = R[rng.choice(m, int((~ok).sum()))]
    return (r2[:, None] - 2 * R @ C.T + (C * C).sum(1)[None]).argmin(1)


a4 = np.zeros(N, np.int64); a16 = np.zeros(N, np.int64); a64 = np.zeros(N, np.int64); d16 = np.zeros(N, np.int64)
for c in range(K):
    i, j = cstart[c], cstart[c + 1]
    R = X[i:j]
    s4 = km(R, 4)
    s16 = np.zeros(j - i, np.int64); s64 = np.zeros(j - i, np.int64)
    for u in range(4):
        w = np.where(s4 == u)[0]
        t = km(R[w], 4)
        s16[w] = u * 4 + t
        for v in range(4):
            w2 = w[t == v]
            s64[w2] = (u * 4 + v) * 4 + km(R[w2], 4)
    a4[i:j] = c * 64 + s4 * 16; a16[i:j] = c * 64 + s16 * 4; a64[i:j] = c * 64 + s64
    d16[i:j] = c * 64 + km(R, 16)
rowc = np.repeat(np.arange(K), np.diff(cstart))
for name, a in (("a4", a4), ("a16", a16), ("a64", a64), ("d16", d16)):
    lab = np.unique(a, return_inverse=True)[1].astype(np.int32)   # leaf ids, increasing with (cluster, sub-cluster)
    np.save(f"{out}/{name}.npy", lab)
    L = lab.max() + 1
    cnt = np.bincount(lab, minlength=L)
    o = np.argsort(lab, kind="stable")
    st = np.r_[0, np.cumsum(cnt)[:-1]]
    C = np.add.reduceat(X[o], st, axis=0) / cnt[:, None]
    np.save(f"{out}/{name}_cent.npy", C.astype(np.float32))
    par = np.zeros(L, np.int32); par[lab] = rowc
    np.save(f"{out}/{name}_parent.npy", par)
    print(ds, name, "leaves", L, "sizes", cnt.min(), int(np.median(cnt)), cnt.max(), flush=True)
