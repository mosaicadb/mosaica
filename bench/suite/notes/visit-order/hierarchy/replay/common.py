# Shared helpers for the hierarchy replays: data, partitions, per-leaf statistics, order keys and the counting.
# Everything is counted, nothing timed. A "leaf" is whatever is probed whole (a cluster or a sub-cluster); inside a
# probed leaf the search is taken as exact, so recall is decided by which leaves are probed.
#   python3 -I <script>.py DS      (DS: glove | yahoo | laion); caches under $HIER_CACHE (a scratch directory)
import os, json, time
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")  # many small per-leaf LAPACK calls; the machine is shared
import numpy as np
from statistics import NormalDist

PREP = "/home/user/suite-data/prep"
WORK = "/home/user/suite-data/work"
NAMES = dict(glove="glove-100-angular", yahoo="yahoo-minilm-384-normalized", laion="laion-clip-512-normalized")
CACHE = os.environ.get("HIER_CACHE", "/tmp/hier-cache")
KNN = 10
TARGETS = (0.8, 0.9, 0.95, 0.99)
SHARES = (0.002, 0.005, 0.016, 0.063)
_t0 = time.time()


def log(*a):
    print(f"[{time.time() - _t0:5.0f}s]", *a, flush=True)


def cpath(ds, name):
    d = os.path.join(CACHE, ds)
    os.makedirs(d, exist_ok=True)
    return os.path.join(d, name)


def cached(ds, name, fn):
    p = cpath(ds, name + ".npz")
    if os.path.exists(p):
        z = np.load(p)
        return {k: z[k] for k in z.files}
    r = fn()
    np.savez(p, **r)
    return r


class DS:
    """X: corpus (N x d f32, row = entity); Q: queries (f32); S: Q X' (nq x N f32); the real fit's clusters as
    a (entity -> cluster), Mq (stored int8 centres, first d dimensions), grp (cluster -> group), V (group bases)."""

    def __init__(self, ds):
        self.ds, self.name = ds, NAMES[ds]
        p, w = os.path.join(PREP, self.name), os.path.join(WORK, self.name, "vexact/index")
        self.prep, self.fit = p, w
        self.X = np.load(os.path.join(p, "corpus32.npy"))
        self.N, self.d = self.X.shape
        self.Q = np.load(os.path.join(p, "queries.npy")).astype(np.float32)
        self.nq = len(self.Q)
        self.order = np.load(os.path.join(p, "order.npy"), mmap_mode="r")
        self.thr = np.load(os.path.join(p, "thresholds.npy"))
        self.meta = json.load(open(os.path.join(w, "meta.json")))
        cstart, perm = np.load(os.path.join(w, "cstart.npy")), np.load(os.path.join(w, "perm.npy"))
        self.K = len(cstart) - 1
        a = np.empty(self.N, np.int64)
        a[perm] = np.repeat(np.arange(self.K), np.diff(cstart))
        self.a = a
        mq = np.load(os.path.join(w, "mq8.npy")).astype(np.float64) * np.load(os.path.join(w, "msc.npy"))[:, None]
        self.Mq = mq[:, :self.d].astype(np.float32)
        self.grp = np.load(os.path.join(w, "grp.npy"))
        sp = cpath(ds, "S.npy")
        if not os.path.exists(sp):
            S = np.concatenate([self.Q @ self.X[i:i + 100_000].T for i in range(0, self.N, 100_000)], 1)
            np.save(sp, S)
        self.S = np.load(sp)
        top = np.argsort(-self.S, 1, kind="stable")[:, :KNN]
        agree = np.mean([len(set(top[i]) & set(self.order[i, :KNN])) / KNN for i in range(self.nq)])
        assert agree > 0.995, agree  # ties and f32 rounding only

    def visible(self, name):
        """Visible sets: the suite's random-1 / random-25, and two correlated ones made here: clus-P (whole clusters
        of the real fit, drawn at random until P% of the corpus) and cap-P (the P% of items scoring highest against
        one corpus item: one region of the space). Returns a boolean mask over entities."""
        m = np.zeros(self.N, bool)
        kind, pc = name.split("-")
        rng = np.random.default_rng(20261118 + int(pc))
        if kind == "random":
            m[np.load(os.path.join(self.prep, f"vis_{name}.npy"))] = True
        elif kind == "clus":
            sz = np.bincount(self.a, minlength=self.K)
            o = rng.permutation(self.K)
            take = o[:max(1, int(np.searchsorted(np.cumsum(sz[o]), self.N * int(pc) / 100) + 1))]
            m[np.isin(self.a, take)] = True
        elif kind == "cap":
            s = self.X @ self.X[rng.integers(self.N)]
            m[np.argsort(-s)[:int(self.N * int(pc) / 100)]] = True
        return m

    def truth_knn(self, vis=None):
        """nq lists of the k best visible entities."""
        if vis is None:
            return [np.asarray(self.order[i, :KNN], np.int64) for i in range(self.nq)]
        ids = np.where(vis)[0]
        sv = self.S[:, ids]
        return [ids[np.argsort(-sv[i], kind="stable")[:KNN]] for i in range(self.nq)]

    def truth_thr(self, col, vis=None):
        return [np.where((self.S[i] >= self.thr[i, col]) & (True if vis is None else vis))[0] for i in range(self.nq)]


def counts(a, L, sets):
    """nq x L: how many of each query's true answers each leaf holds."""
    return np.stack([np.bincount(a[s], minlength=L) for s in sets]).astype(np.float64)


def z_of(k, n):
    """Phi^-1(1 - k/n), 0 for n <= k (as rust/src/v2/q.rs z_of)."""
    n = np.asarray(n, np.float64)
    out = np.zeros(n.shape)
    ok = n > k
    nd = NormalDist()
    tab = {v: nd.inv_cdf(1 - k / v) for v in np.unique(n[ok])}
    out[ok] = [tab[v] for v in n[ok]]
    return out


def leaf_stats(X, a, L, r=0, centres=None):
    """Per leaf: n, mean (f32), rad (RMS distance to `centres` when given, else to the mean), tr (trace of the
    covariance about the mean) and, for r > 0, the top r principal directions U (L x r x d f32) and values lam."""
    N, d = X.shape
    o = np.argsort(a, kind="stable")
    n = np.bincount(a, minlength=L)
    st = np.r_[0, np.cumsum(n)]
    mean = np.zeros((L, d), np.float32); rad = np.zeros(L); tr = np.zeros(L)
    lam = np.zeros((L, r)); U = np.zeros((L, r, d), np.float32)
    for c in range(L):
        if n[c] == 0:
            continue
        x = X[o[st[c]:st[c + 1]]].astype(np.float64)
        m = x.mean(0)
        mean[c] = m
        xc = x - m
        tr[c] = (xc * xc).sum() / n[c]
        ctr = m if centres is None else centres[c].astype(np.float64)
        rad[c] = np.sqrt(((x - ctr) ** 2).sum() / n[c])
        if r and n[c] > 1:
            if n[c] > d:
                w, v = np.linalg.eigh(xc.T @ xc / n[c])
                w, v = w[::-1][:r], v[:, ::-1][:, :r].T
            else:
                _, s, v = np.linalg.svd(xc, full_matrices=False)
                w, v = (s ** 2 / n[c])[:r], v[:r]
            lam[c, :len(w)] = w
            U[c, :len(w)] = v
    return dict(n=n, mean=mean, rad=rad, tr=tr, lam=lam, U=U)


def sigma(Q, st, r, which=None):
    """sqrt(q' Sigma q) per (query, leaf): top r directions plus an isotropic rest. `which` maps the rows asked for
    to the leaves of `st` (to use a parent's covariance for its children)."""
    lam, U, tr = st["lam"][:, :r], st["U"][:, :r], st["tr"]
    L, _, d = U.shape
    proj = (Q @ np.ascontiguousarray(U).reshape(L * r, d).T).reshape(len(Q), L, r).astype(np.float64)
    qq = (Q.astype(np.float64) ** 2).sum(1)[:, None]
    rest = np.maximum(tr - lam.sum(1), 0) / max(d - r, 1)
    var = (proj ** 2 * lam[None]).sum(2) + rest[None] * np.maximum(qq - (proj ** 2).sum(2), 0)
    s = np.sqrt(np.maximum(var, 1e-12))
    return s if which is None else s[:, which]


def sdn(x):
    s = float(np.std(x))
    return s if s > 0 else 1.0


def run(key, cnt, sizes, leaf_grp=None, live=None, leaf_par=None):
    """Probe leaves in descending key (ties by lower id). key: nq x L (-inf: not a candidate). Returns per query and
    per number of probed leaves p = 1..L: found (answers), items, groups and parents (distinct groups / parent
    clusters among the probed leaves)."""
    nq, L = key.shape
    if live is not None:
        key = np.where(live[None, :], key, -np.inf)
    o = np.argsort(-key, 1, kind="stable")
    valid = np.take_along_axis(key, o, 1) > -np.inf
    found = np.cumsum(np.take_along_axis(cnt, o, 1) * valid, 1)
    items = np.cumsum(sizes[o] * valid, 1).astype(np.float64)
    def distinct(lab):
        if lab is None:
            return None
        g = lab[o]
        first = np.zeros((nq, L))
        for i in range(nq):
            _, idx = np.unique(g[i][valid[i]], return_index=True)
            first[i, idx] = 1
        return np.cumsum(first, 1)
    return dict(found=found, items=items, groups=distinct(leaf_grp), parents=distinct(leaf_par), valid=valid)


def summarise(res, tot, N, extra=None, targets=TARGETS, shares=SHARES):
    """Fixed number of probes for every query (as nprobe works): mean recall and mean items along p; interpolated to
    items (and groups, probes) at each recall target and to recall at each share of the corpus N."""
    rec = (res["found"] / np.maximum(tot, 1)[:, None]).mean(0)
    it = res["items"].mean(0)
    out = dict(at={}, share={})
    for t in targets:
        hit = np.where(rec >= t - 1e-12)[0]
        if not len(hit):
            out["at"][t] = None
            continue
        p = int(hit[0])
        f = 1.0 if p == 0 or rec[p] == rec[p - 1] else (t - rec[p - 1]) / (rec[p] - rec[p - 1])
        f = min(max(f, 0.0), 1.0)
        lin = lambda v: float(v[p] if p == 0 else v[p - 1] + f * (v[p] - v[p - 1]))
        e = dict(items=lin(it), probes=p + 1)
        for k in ("groups", "parents"):
            if res.get(k) is not None:
                e[k] = lin(res[k].mean(0))
        if extra:
            for k, v in extra.items():
                e[k] = lin(v) if np.ndim(v) else float(v)
        out["at"][t] = e
    for s in shares:
        out["share"][s] = float(np.interp(s * N, np.r_[0, it], np.r_[0, rec]))
    out["max"] = float(rec[-1])
    return out


def fmt(s, keys=("items",), targets=TARGETS, shares=SHARES):
    a = " ".join(f"{s['share'][x]:.3f}" for x in shares)
    b = []
    for t in targets:
        e = s["at"][t]
        b.append("-" if e is None else "/".join(f"{e[k]:.0f}" if k in e else "-" for k in keys))
    return a + " | " + " | ".join(f"{x:>16s}" for x in b)


def kmeans(x, k, niter=10, seed=1):
    import faiss
    faiss.omp_set_num_threads(1 if len(x) < 50_000 else 4)
    x = np.ascontiguousarray(x, np.float32)
    k = int(min(k, len(x)))
    if k <= 1:
        return np.zeros(len(x), np.int64)
    km = faiss.Kmeans(x.shape[1], k, niter=niter, seed=seed, verbose=False, min_points_per_centroid=1,
                      max_points_per_centroid=10 ** 9)
    km.train(x)
    return km.index.search(x, 1)[1].ravel().astype(np.int64)


def compact(a):
    u, inv = np.unique(a, return_inverse=True)
    return inv.astype(np.int64), len(u)


def subsplit(D, s):
    """Each cluster of the real fit split into s sub-clusters by k-means on its items (rows, clusters and groups are
    untouched: a sub-cluster is a list of rows of its cluster). Returns sub (entity -> leaf) and par (leaf -> cluster)."""
    def fn():
        sub = np.zeros(D.N, np.int64)
        o = np.argsort(D.a, kind="stable")
        st = np.r_[0, np.cumsum(np.bincount(D.a, minlength=D.K))]
        for c in range(D.K):
            ids = o[st[c]:st[c + 1]]
            sub[ids] = c * s + kmeans(D.X[ids], s, 10, seed=c + 1)
        sub, L = compact(sub)
        par = np.zeros(L, np.int64)
        par[sub] = D.a
        return dict(sub=sub, par=par)
    return cached(D.ds, f"sub{s}", fn)


def refit(D, K, niter=10, sample=400_000):
    """faiss k-means with K clusters on the same corpus (a different partition from the real fit), and groups as
    fit.py makes them: k-means of the normalised centres into 64."""
    def fn():
        import faiss
        faiss.omp_set_num_threads(4)
        rng = np.random.default_rng(K)
        tr = D.X[np.sort(rng.choice(D.N, min(D.N, sample), replace=False))]
        km = faiss.Kmeans(D.d, K, niter=niter, seed=K, verbose=False, max_points_per_centroid=10 ** 9)
        km.train(tr)
        a = np.concatenate([km.index.search(D.X[i:i + 100_000], 1)[1].ravel() for i in range(0, D.N, 100_000)])
        a, Kk = compact(a)
        M = leaf_stats(D.X, a, Kk)["mean"]
        Mh = M / np.linalg.norm(M, axis=1, keepdims=True)
        return dict(a=a, grp=kmeans(Mh, 64, 20, seed=7))
    return cached(D.ds, f"refit{K}", fn)
