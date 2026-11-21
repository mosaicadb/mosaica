# Fit the build parameters of the fast-scan index (v2) to a file of vectors: clusters with 8-bit centres, groups of
# clusters with a local basis each, the query-offset profile, the level schedule and the randomised blocks. The Rust
# build (`vexact --v2 --index OUTDIR --rebuild`, v2::rebuild) encodes every item from these and saves the index in
# OUTDIR/encoded; `vexact --v2 --index OUTDIR MODE` opens it.
# usage: python3 fit.py VECTORS.npy OUTDIR [--basis dual|pca] [--query-sample QUERIES.npy [--query-weight W]] [--metric angular] [--seed S]
#   [--per-cluster by-n|M] [--groups by-n|by-k8|G] [--rows ROWS.npy]
#   VECTORS.npy: N x n, float16/32/64; row i is entity i. The vectors are read from it as needed (no float32 copy).
#   --per-cluster: by-n (default): 2,000 items per cluster up to 2M items, doubling for each 4x N (4,000 up to 8M, 8,000
#     up to 32M, ...; research/levels-clusters' rule, `per_cluster`); or M items per cluster on average (1,000 before).
#   --groups: by-n (default): round(N / (0.5 n^2)), from 1 to K; by-k8: min(64, K / 8, N / (20 n)); or G.
#   --rows: a .npy of row numbers of VECTORS: fit those rows only, as entities 0, 1, ...
#   --basis pca: each group's PCA basis of the residuals and the data-only profile H25.
#   --basis dual (default): the dual pair of the geometric-mean metric M = Sigma_Q^-1 # Sigma_R per group. Sigma_Q is one
#     covariance of query offsets q - (q.m^) m^ from clusters drawn by size, 40 per pseudo-query, with a ridge of 0.1
#     of its mean eigenvalue; the pseudo-queries are 3,000 corpus items, or with --query-sample the mix
#     (1 - W) S / tr S + W St / tr St of that and the same covariance of the given queries (W from --query-weight,
#     0 to 0.5, 0.5 by default; a dial on the trace-normalised covariances, not a share of traffic). V is the query-side transform V_Q (q.r = (V_Q^T q).(V_Q^-1 r));
#     lam is the mean square of the item-side coordinates V_Q8^-1 r under the int8 V_Q8 the Rust build uses; the
#     profile is the mean square of V_Q8^T (q - m) for corpus items against their 4 nearest centres.
# Writes to OUTDIR: Xs.npy (unit vectors in cluster order, f16, zero-padded to a multiple of 32 dimensions), perm.npy
# (row -> entity), cstart, grp, mq8, msc, lam, V, r2 (mean |r|^2 per group), profile.npy, tree tables, qi (rows for
# verification), meta.json (with the fit used, under "fit", and the randomised levels' certified constant under
# "CERTS": the shipped table's entry for the layout (src/certs_table.json), else from certify.py; --cert-time caps that
# step, after which the build falls back to closed-form bounds).
import argparse, json, math, os, sys, time
import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "combined"))
from core import kmeans, TREE, GTREE  # noqa: E402
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import certify  # noqa: E402
from rules import per_cluster, groups_for  # noqa: E402,F401 (tools/rules.py, shared with partition.py)

# Design constants (not fitted): items per cluster before the rule by N (`per_cluster`, the default), the guarantee's N and epsilon, the deterministic level ends, the
# randomised level ends up to 256 B, randomised block boundaries as fractions of n, the H25 hedge weight.
PER_CLUSTER = 1000
NTARGET, EPS = 1e9, 1e-9
DLV = [4, 8, 12, 16, 24, 32]
RLV_BASE = [56, 96, 160, 256]
BLOCK_FRACS = [0.125, 0.29167, 0.625]
HEDGE = 0.25
FORMAT = 2
RIDGE = 0.1

T0 = time.time()
def log(*a):
    print(f"[{time.time() - T0:6.0f}s]", *a, flush=True)


def schedule(n0, n):
    """Randomised level ends: the base list, then steps of 1.6x, while within 40% of the f16 vector, within 256 B
    per 768 dimensions and within 8 bits per dimension; a list cut short by that cap ends at the cap."""
    cap = min(0.4 * 2 * n0, 256 * max(1.0, n0 / 768), DLV[-1] + n)
    rlv = [b for b in RLV_BASE if b <= cap]
    if len(rlv) < len(RLV_BASE):
        # the base list cut short (low dimension): end at the cap, on whole 8-byte steps, at least 8 B past the
        # deterministic levels (within 8 bits per dimension), with levels between no more than 1.8x apart
        top = max(int(cap) // 8 * 8, DLV[-1] + min(8, n))
        prev = rlv[-1] if rlv else DLV[-1]
        if top > prev:
            mids = []
            while top / (mids[-1] if mids else prev) > 1.8:
                m = int(round(((mids[-1] if mids else prev) * top) ** 0.5 / 8)) * 8
                if m <= (mids[-1] if mids else prev) or m >= top:
                    break
                mids.append(m)
            rlv += mids + [top]
    else:
        b = RLV_BASE[-1]
        while True:
            b = int(round(b * 1.6 / 32)) * 32
            if b > cap:
                break
            rlv.append(b)
    assert rlv, "empty randomised schedule"
    bounds = [0] + [int(round(f * n / 16)) * 16 for f in BLOCK_FRACS] + [n]
    for j in range(1, len(bounds)):
        bounds[j] = max(bounds[j], bounds[j - 1] + 16)
    assert bounds[-2] <= n - 16, f"n = {n} is too small for four randomised blocks"
    return rlv, bounds


class Subset:
    """Rows `rows` of a (memory-mapped) array, indexed like an array of those rows."""

    def __init__(self, X, rows):
        self.X, self.rows = X, np.asarray(rows, np.int64)
        self.shape = (len(self.rows), X.shape[1])

    def __len__(self):
        return len(self.rows)

    def __getitem__(self, k):
        if isinstance(k, slice):
            k = np.arange(*k.indices(len(self)))
        r = self.rows[np.asarray(k)]
        o = np.argsort(r, kind="stable")
        out = np.empty((len(r), self.shape[1]), self.X.dtype)
        out[o] = self.X[r[o]]
        return out


class Rows:
    """The unit vectors, zero-padded to n, read from the source file on demand (row order `order` when given), so no
    full float32 copy is held: R[i:j] and R[index array] give float32 rows, the same values as normalising the whole
    file at once."""

    def __init__(self, X, n, order=None):
        self.X, self.n, self.order = X, n, order

    def __len__(self):
        return len(self.X)

    def permuted(self, order):
        return Rows(self.X, self.n, order)

    def __getitem__(self, k):
        if isinstance(k, slice):
            k = np.arange(*k.indices(len(self)))
        k = np.asarray(k)
        if self.order is not None:
            k = self.order[k]
        x = np.asarray(self.X[k], np.float32)
        nr = np.linalg.norm(x, axis=1, keepdims=True)
        out = np.zeros((len(x), self.n), np.float32)
        out[:, :x.shape[1]] = np.where(nr > 0, x / np.where(nr > 0, nr, 1), 0)
        return out


def int8_basis(v):
    """The int8 basis the Rust build makes from V (f32, columns): one scale per column, max |v| / 127."""
    v = v.astype(np.float64)
    sc = np.abs(v).max(0) / 127.0
    sc[sc == 0] = 1.0
    return np.clip(np.round(v / sc), -127, 127) * sc


def offset_cov(Q, Mh, p, rng):
    """Covariance of the offsets q - (q.m^) m^ of unit queries Q from 40 centres each, drawn with probabilities p."""
    n = Mh.shape[1]
    S = np.zeros((n, n))
    c = 0
    for i in range(0, len(Q), 200):
        q = Q[i:i + 200]
        C = Mh[rng.choice(len(Mh), (len(q), 40), p=p)]
        D = (q[:, None, :] - np.einsum("bkn,bn->bk", C, q)[:, :, None] * C).reshape(-1, n).astype(np.float64)
        S += D.T @ D
        c += len(D)
    return S / c


def sigma_q(Xu, Mh, cnt, n, sample, weight):
    """Sigma_Q of the dual basis: offsets of 3,000 corpus items (and, with a query sample, the trace-normalised mix of
    that and the queries' offsets, the queries weighted `weight`), with a ridge of RIDGE times the mean eigenvalue."""
    rng = np.random.default_rng(3)
    p = cnt / cnt.sum()
    S = offset_cov(Xu[np.sort(rng.choice(len(Xu), min(len(Xu), 3000), replace=False))], Mh, p, rng)
    if sample:
        T = np.load(sample).astype(np.float32)
        T = np.pad(T, ((0, 0), (0, n - T.shape[1])))
        T /= np.linalg.norm(T, axis=1, keepdims=True)
        St = offset_cov(T, Mh, p, rng)
        S = (1.0 - weight) * (S / np.trace(S)) + weight * (St / np.trace(St))
        log(f"Sigma_Q: {min(len(Xu), 3000)} corpus items and {len(T)} queries from {sample}, query weight {weight:g}")
    return S + RIDGE * np.trace(S) / n * np.eye(n)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("vectors")
    ap.add_argument("out")
    ap.add_argument("--metric", default="angular", choices=["angular"])
    ap.add_argument("--seed", type=int, default=20261112)
    ap.add_argument("--basis", default="dual", choices=["dual", "pca"])
    ap.add_argument("--query-sample", default="", help="queries mixed into Sigma_Q (dual basis only)")
    ap.add_argument("--query-weight", type=float, default=0.5,
                    help="with --query-sample: the queries' weight W in Sigma_Q = (1 - W) S / tr S + W St / tr St, 0 to 0.5 (default 0.5)")
    ap.add_argument("--cert-time", type=float, default=600, help="seconds allowed for the constant's certificate")
    ap.add_argument("--per-cluster", default="by-n",
                    help="by-n (default): 2,000 items per cluster up to 2M items, doubling for each 4x; or items per cluster on average (1000 before)")
    ap.add_argument("--groups", default="by-n", help="by-n (default): round(N / (0.5 n^2)), from 1 to K; by-k8: min(64, K / 8, N / (20 n)); or G")
    ap.add_argument("--rows", default="", help="a .npy of row numbers of VECTORS: fit those rows only, as entities 0, 1, ...")
    a = ap.parse_args()
    if a.query_sample and a.basis != "dual":
        sys.exit("--query-sample shapes Sigma_Q, which only the dual basis uses; add --basis dual")
    if not 0.0 <= a.query_weight <= 0.5:
        sys.exit(f"--query-weight is {a.query_weight:g}; give a weight from 0 to 0.5")
    if not (a.per_cluster == "by-n" or (a.per_cluster.isdigit() and int(a.per_cluster) > 0)):
        sys.exit(f"--per-cluster is a positive number or by-n, not {a.per_cluster}")
    if not (a.groups in ("by-k8", "by-n") or (a.groups.isdigit() and int(a.groups) > 0)):
        sys.exit(f"--groups is by-k8, by-n or a positive number, not {a.groups}")
    os.makedirs(a.out, exist_ok=True)
    rng = np.random.default_rng(a.seed)
    X = np.load(a.vectors, mmap_mode="r")
    if a.rows:
        X = Subset(X, np.load(a.rows))
    N, n0 = X.shape
    if n0 < 50:
        sys.exit(f"{a.vectors} has {n0} dimensions; the randomised levels need at least 50 (the first ends at 40 bytes, "
                 f"which must be within 40% of the f16 vector). Pad the vectors with zero columns to 50 or more.")
    n = (n0 + 31) // 32 * 32
    Xu = Rows(X, n)
    nzero = sum(int((np.abs(Xu[i:i + 100_000]).sum(1) == 0).sum()) for i in range(0, N, 100_000))
    log(f"N {N} n {n0} (padded to {n}); zero vectors {nzero}")

    pc = per_cluster(N) if a.per_cluster == "by-n" else int(a.per_cluster)
    K = max(1, round(N / pc))
    log(f"{pc} items per cluster ({a.per_cluster}): {K} clusters")
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
    for i in range(0, N, 100_000):
        np.add.at(Msum, assign[i:i + 100_000], Xu[i:i + 100_000])
    Mean = Msum / cnt[:, None]
    msc = np.abs(Mean).max(1) / 127
    msc[msc == 0] = 1.0
    mq8 = np.round(Mean / msc[:, None]).astype(np.int8)
    Mq = (mq8 * msc[:, None]).astype(np.float32)
    mn = np.linalg.norm(Mq, axis=1)
    Mh = Mq / np.where(mn > 0, mn, 1)[:, None]

    G = groups_for(a.groups, N, K, n0)
    grp = (Mh @ kmeans(Mh.astype(np.float32).copy(), G, rng, 20).T).argmax(1)
    gk = np.unique(grp)
    gmap = -np.ones(G, np.int64)
    gmap[gk] = np.arange(len(gk))
    grp = gmap[grp]
    G = len(gk)
    gsz = np.bincount(grp[assign], minlength=G)
    cpg = np.bincount(grp, minlength=G)
    log(f"K {K} clusters (sizes {cnt.min()}..{cnt.max()}), G {G} groups ({a.groups}; sizes {gsz.min()}..{gsz.max()}, "
        f"clusters per group {cpg.min()}..{cpg.max()})")

    # rows in (group, cluster) order; clusters renumbered so they are contiguous
    order = np.lexsort((assign, grp[assign]))
    assign = assign[order]
    first = assign[np.r_[0, np.where(np.diff(assign))[0] + 1]]
    newid = -np.ones(K, np.int64)
    newid[first] = np.arange(K)
    assign = newid[assign]
    mq8, msc, Mq, grp, cnt = mq8[first], msc[first], Mq[first], grp[first], cnt[first]
    cstart = np.r_[0, np.cumsum(cnt)]
    Xu = Xu.permuted(order)
    xs = np.lib.format.open_memmap(os.path.join(a.out, "Xs.npy"), mode="w+", dtype=np.float16, shape=(N, n))
    for i in range(0, N, 100_000):
        xs[i:i + 100_000] = Xu[i:i + 100_000].astype(np.float16)
    xs.flush()
    del xs
    np.save(os.path.join(a.out, "perm.npy"), order.astype(np.int64))

    lam = np.zeros((G, n))
    V = np.zeros((G, n, n), np.float32)
    r2 = np.zeros(G)
    gstart = np.searchsorted(grp[assign], np.arange(G + 1))
    resid = lambda i, j: (Xu[i:j] - Mq[assign[i:j]]).astype(np.float64)
    if a.basis == "dual":
        SQ = sigma_q(Xu, Mh, cnt, n, a.query_sample, a.query_weight)
        lq, uq = np.linalg.eigh(SQ)
        A, Ai = (uq * np.sqrt(lq)) @ uq.T, (uq / np.sqrt(lq)) @ uq.T
        cond = np.zeros(G)
    for g in range(G):
        Cv = np.zeros((n, n))
        for i in range(gstart[g], gstart[g + 1], 50_000):
            R = resid(i, min(i + 50_000, gstart[g + 1]))
            Cv += R.T @ R
            r2[g] += (R * R).sum()
        ng = max(gstart[g + 1] - gstart[g], 1)
        r2[g] /= ng
        if a.basis == "pca":
            l, v = np.linalg.eigh(Cv / ng)
            lam[g] = np.maximum(l[::-1], 1e-12)
            V[g] = v[:, ::-1]
            continue
        gam, W = np.linalg.eigh(A @ (Cv / ng) @ A)
        gam, W = gam[::-1], W[:, ::-1]
        gam = np.maximum(gam, 1e-12 * gam[0])
        V[g] = ((Ai @ W) * gam ** 0.25).astype(np.float32)
        # the coded coordinates z = V_Q8^-1 r under the int8 basis of the Rust build (one scale per column)
        vq = int8_basis(V[g])
        inv = np.linalg.inv(vq)
        sv = np.linalg.svd(vq, compute_uv=False)
        cond[g] = sv[0] / sv[-1]
        zz = np.zeros(n)
        for i in range(gstart[g], gstart[g + 1], 50_000):
            Z = resid(i, min(i + 50_000, gstart[g + 1])) @ inv.T
            zz += (Z * Z).sum(0)
        lam[g] = np.maximum(zz / ng, 1e-30)
    log("bases done (" + a.basis + "); group sizes", np.diff(gstart).min(), "..", np.diff(gstart).max())

    # data-only profile: corpus items as pseudo-queries against their 4 nearest centres, hedged towards isotropic
    # (spread over the n0 data dimensions; the padding has none)
    pq = Xu[rng.choice(N, min(N, 5000), replace=False)]
    pk = np.argsort(-(pq @ Mq.T), axis=1)[:, :4]
    Qi = np.zeros((G, n))
    for g in range(G):
        rows, cols = np.where(grp[pk] == g)
        Qi[g] = (((pq[rows] - Mq[pk[rows, cols]]) @ V[g]) ** 2).mean(0) if len(rows) > 50 else lam[g]
    if a.basis == "pca":
        iso = np.zeros((G, n))
        iso[:, :n0] = Qi.sum(1, keepdims=True) / n0
        prof = ((1 - HEDGE) * np.sqrt(Qi) + HEDGE * np.sqrt(iso)) ** 2
        fit = dict(basis="pca", profile="h25")
    else:
        # in the int8 query-side coordinates the build uses, no hedge
        rq = np.random.default_rng(a.seed + 2)
        pq2 = Xu[rq.choice(N, min(N, 5000), replace=False)]
        pk2 = np.argsort(-(pq2 @ Mq.T), axis=1)[:, :4]
        prof = np.zeros((G, n))
        for g in range(G):
            rows, cols = np.where(grp[pk2] == g)
            prof[g] = (((pq2[rows] - Mq[pk2[rows, cols]]) @ int8_basis(V[g])) ** 2).mean(0) if len(rows) > 50 else lam[g]
        fit = dict(basis="dual", profile="qi", ridge=RIDGE, sigma_q="corpus items" + (" + queries " + os.path.abspath(a.query_sample) if a.query_sample else ""),
                   **(dict(query_weight=a.query_weight) if a.query_sample else {}),
                   cond_max=float(cond.max()))

    rlv, bounds = schedule(n0, n)
    # the rate design's constant (two-sided, N = 1e9); the query-time constants follow the index size (src/v2/tail.rs)
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
    sv("r2", r2)
    sv("profile", prof)
    sv("tree_edges", tree_edges)
    sv("tree_cent", tree_cent)
    sv("gtree", GTREE.astype(np.float64))
    sv("qi", np.sort(rng.choice(N, min(N, 200), replace=False)).astype(np.int64))
    meta = dict(N=int(N), n=int(n), n_orig=int(n0), K=int(K), G=int(G), metric=a.metric, DLV=DLV, RLV=rlv, BOUNDS=bounds,
                CSG=math.sqrt(2 * L1), L1=L1, EPS=EPS, xs="Xs.npy", source=os.path.abspath(a.vectors),
                format=FORMAT, fit=fit)
    # recorded only when not the earlier default (1,000 per cluster, by-k8), so such a fit's meta.json is as before
    if a.per_cluster != str(PER_CLUSTER) or a.groups != "by-k8":
        meta["clusters"] = dict(per_cluster=a.per_cluster, groups=a.groups)
    if a.rows:
        meta["rows"] = os.path.abspath(a.rows)
    with open(os.path.join(a.out, "meta.json"), "w") as f:
        json.dump(meta, f, indent=1)
    log("saved", meta)
    certify.add_certificate(a.out, certify.ln_delta(meta), a.cert_time, log, table=True)


if __name__ == "__main__":
    main()
