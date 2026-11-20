"""Replay A for layered.md: how the work of the cluster order scales with N and with the cluster size S.

Counts only (nothing is timed). For one dataset: nested random subsamples N, N/4, N/16; on each, k-means partitions at
several cluster sizes S; for every query the exact per-cluster statistics of its scores (mean mu_c, standard deviation
sigma_c, the true top-k rows per cluster), the clusters read in the order of mu_c + 3 sigma_c, the same number for
every query. Reported: rows and clusters read to recall 0.9 / 0.95 / 0.99 at k = 1, 10, 100.

Also, per partition:
  - a random visible set as a subsample: the partition of the full set read through a random 1/4, 1/16 and 1/100;
  - a sketch of the centres: the centre scores from the first r principal coordinates of the centres, and how many
    centres must then be scored in full to keep 99% of the clusters the exact order reads to recall 0.95;
  - two levels of centres: parents by k-means over the centres (sqrt(K) of them), keyed as one cluster each (their
    rows' mean and standard deviation), the best P expanded; centres scored and rows read to each recall.

usage: python3 -I scaling.py DATASET OUT.json   (DATASET: glove | yahoo | laion | sift)
"""
import json
import sys
import time

import numpy as np

PREP = "/home/user/suite-data/prep"
DL = "/home/user/scratch-layered/dl"
PSI = 3.0
TARGETS = (0.9, 0.95, 0.99)
KS = (1, 10, 100)
NQ = 1000
CHUNK = 100


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def unit(a):
    a = np.asarray(a, dtype=np.float32)
    return a / np.linalg.norm(a, axis=1, keepdims=True)


def load(ds):
    """X (N x d f32; inner-product scores), the vectors to cluster, queries, metric."""
    import h5py
    if ds == "glove":
        X = np.load(f"{PREP}/glove-100-angular/corpus32.npy")
        with h5py.File(f"{DL}/glove-100-angular.hdf5", "r") as h:
            Q = unit(h["test"][:NQ])
        return X, X, Q, "angular"
    if ds == "yahoo":
        X = np.load(f"{PREP}/yahoo-minilm-384-normalized/corpus32.npy")
        with h5py.File("/home/user/mosaica/yahoo-minilm-384-normalized.hdf5", "r") as h:
            Q = unit(h["test"][:NQ])
        return X, X, Q, "angular"
    if ds == "laion":
        X = np.load(f"{PREP}/laion-clip-512-normalized/corpus32.npy")
        with h5py.File("/home/user/mosaica/laion-clip-512-normalized.hdf5", "r") as h:
            Q = unit(h["test"][:NQ])
        return X, X, Q, "angular"
    if ds == "sift":
        # L2: the score q.x - |x|^2 / 2 ranks as the distance does; as an inner product with one more coordinate
        with h5py.File(f"{DL}/sift-128-euclidean.hdf5", "r") as h:
            T = h["train"][:].astype(np.float32)
            Q = h["test"][:NQ].astype(np.float32)
        s = np.sqrt((T * T).sum(1).mean())
        T /= s
        Q /= s
        X = np.concatenate([T, -0.5 * (T * T).sum(1, keepdims=True)], axis=1)
        Qa = np.concatenate([Q, np.ones((len(Q), 1), np.float32)], axis=1)
        return np.ascontiguousarray(X), T, Qa, "l2"
    raise SystemExit(f"unknown dataset {ds}")


def kmeans(V, K, metric, seed):
    import faiss
    faiss.omp_set_num_threads(3)
    km = faiss.Kmeans(V.shape[1], K, niter=10, seed=seed, spherical=(metric == "angular"), max_points_per_centroid=128, min_points_per_centroid=1)
    km.train(V)
    if metric == "angular":
        ix = faiss.IndexFlatIP(V.shape[1])
    else:
        ix = faiss.IndexFlatL2(V.shape[1])
    ix.add(km.centroids)
    a = np.empty(len(V), np.int64)
    for i in range(0, len(V), 100000):
        a[i:i + 100000] = ix.search(V[i:i + 100000], 1)[1][:, 0]
    return a


class Part:
    """A partition of the rows `rows` (indices into X) into K clusters."""

    def __init__(self, name, rows, assign, K, X):
        import scipy.sparse as sp
        self.name, self.rows, self.K = name, rows, K
        self.order = np.argsort(assign, kind="stable")
        self.cnt = np.bincount(assign, minlength=K)
        self.ne = np.flatnonzero(self.cnt)
        self.starts = (np.cumsum(self.cnt) - self.cnt)[self.ne]
        self.assign = assign
        M = sp.csr_matrix((np.ones(len(rows), np.float32), (assign, np.arange(len(rows)))), shape=(K, len(rows)))
        Xr = X if len(rows) == len(X) else X[rows]
        self.cent = np.asarray(M @ Xr) / np.maximum(self.cnt, 1)[:, None]
        self.acc = []

    def stats(self, Ssub, top):
        """Per query and cluster: mean and sd of the scores, and the hits of the top k for each k of KS."""
        Q = len(Ssub)
        Ss = Ssub[:, self.order]
        s1 = np.zeros((Q, self.K))
        s2 = np.zeros((Q, self.K))
        s1[:, self.ne] = np.add.reduceat(Ss, self.starts, axis=1, dtype=np.float64)
        np.multiply(Ss, Ss, out=Ss)
        s2[:, self.ne] = np.add.reduceat(Ss, self.starts, axis=1, dtype=np.float64)
        n = np.maximum(self.cnt, 1)
        mu = s1 / n
        sd = np.sqrt(np.maximum(s2 / n - mu * mu, 0))
        hits = {}
        for k in KS:
            h = np.zeros((Q, self.K), np.int32)
            np.add.at(h, (np.repeat(np.arange(Q), k), self.assign[top[:, :k]].ravel()), 1)
            hits[k] = h
        return mu, sd, hits, s1, s2


def curve(key, cnt, hits, live=None):
    """Clusters in decreasing key; per query the cumulative rows and hits (for each k) after m clusters."""
    if live is not None:
        key = np.where(live, key, -np.inf)
    o = np.argsort(-key, axis=1, kind="stable")
    c = np.broadcast_to(cnt, key.shape) if cnt.ndim == 1 else cnt
    rows = np.take_along_axis(c, o, 1).astype(np.float64)
    if live is not None:
        rows = rows * np.take_along_axis(live, o, 1)
    cr = np.cumsum(rows, axis=1)
    lo = None if live is None else np.take_along_axis(live, o, 1)
    ch = {k: np.cumsum(np.take_along_axis(h, o, 1) * (1 if lo is None else lo), axis=1) for k, h in hits.items()}
    return o, cr, ch


def at_targets(rows, rec):
    """Mean rows and clusters at which the mean recall reaches each target (the same m for every query; linear in m);
    `rows` and `rec` the means over the queries after m = 1, 2, ... clusters."""
    out = []
    for t in TARGETS:
        i = np.searchsorted(rec, t - 1e-12)
        if i >= len(rec):
            out.append((None, None))
            continue
        if i == 0:
            out.append((float(rows[0] * t / max(rec[0], 1e-12)), float(t / max(rec[0], 1e-12))))
            continue
        f = (t - rec[i - 1]) / max(rec[i] - rec[i - 1], 1e-12)
        out.append((float(rows[i - 1] + f * (rows[i] - rows[i - 1])), float(i + f)))
    return out


def main():
    ds, outp = sys.argv[1], sys.argv[2]
    X, V, Q, metric = load(ds)
    N, d = X.shape
    log(ds, "N", N, "d", d, "queries", len(Q), metric)
    rng = np.random.default_rng(5)
    perm = rng.permutation(N)
    subs = {1: np.arange(N), 4: np.sort(perm[:N // 4]), 16: np.sort(perm[:N // 16])}
    sizes = {1: (250, 1000, 4000, 16000), 4: (250, 1000, 4000), 16: (62, 250, 1000)}
    parts = []
    for f, rows in subs.items():
        for S in sizes[f]:
            K = max(2, round(len(rows) / S))
            t = time.time()
            a = kmeans(V if f == 1 else V[rows], K, metric, 11 + f)
            parts.append((f, S, Part(f"N/{f} S={S}", rows, a, K, X)))
            log("partition", parts[-1][2].name, "K", K, f"{time.time() - t:.0f}s")
    # the full set's partitions read through a random visible set (a subsample of every cluster)
    vis = {v: np.sort(perm[:N // v]) for v in (4, 16, 100)}
    full = [(S, p) for f, S, p in parts if f == 1 and S in (1000, 4000)]
    visparts = []
    for S, p in full:
        for v, rows in vis.items():
            vp = Part(f"N S={S} visible 1/{v}", rows, p.assign[rows], p.K, X)
            visparts.append((S, v, vp))
    # sketches and parents of the full set's partitions
    import faiss
    sk = {}
    for f, S, p in parts:
        if f != 1:
            continue
        w = p.cnt / p.cnt.sum()
        mbar = (p.cent * w[:, None]).sum(0)
        U, sv, Vt = np.linalg.svd((p.cent - mbar) * np.sqrt(w)[:, None], full_matrices=False)
        kp = max(2, int(round(np.sqrt(p.K))))
        km = faiss.Kmeans(V.shape[1], kp, niter=20, seed=3, spherical=(metric == "angular"), min_points_per_centroid=1, max_points_per_centroid=10 ** 6)
        km.train(np.ascontiguousarray(p.cent[:, :V.shape[1]] if metric == "l2" else p.cent, dtype=np.float32))
        par = km.index.search(np.ascontiguousarray(p.cent[:, :V.shape[1]] if metric == "l2" else p.cent, dtype=np.float32), 1)[1][:, 0]
        sk[p.name] = dict(mbar=mbar, Vt=Vt, evr=(sv ** 2 / (sv ** 2).sum()).cumsum(), par=par, kp=kp)
    acc = {}

    def push(name, **kw):
        # curves (cr, h*) and centre counts (cs) as running sums over the queries; ranks kept per query (int16)
        a = acc.setdefault(name, {})
        for k, v in kw.items():
            if k == "cr" or k.startswith("h") or k == "cs":
                a[k] = a.get(k, 0) + v.sum(0)
                a["nq_" + k] = a.get("nq_" + k, 0) + len(v)
            elif k.endswith("rank") or k == "rank_exact":
                a.setdefault(k, []).append(v.astype(np.int16))
            else:
                a.setdefault(k, []).append(v)

    for q0 in range(0, len(Q), CHUNK):
        Sc = Q[q0:q0 + CHUNK] @ X.T
        for f, rows in subs.items():
            Ssub = Sc if f == 1 else Sc[:, rows]
            top = np.argpartition(-Ssub, 100, axis=1)[:, :100]
            top = np.take_along_axis(top, np.argsort(-np.take_along_axis(Ssub, top, 1), axis=1), 1)
            for ff, S, p in parts:
                if ff != f:
                    continue
                mu, sd, hits, s1, s2 = p.stats(Ssub, top)
                o, cr, ch = curve(mu + PSI * sd, p.cnt, hits, live=np.broadcast_to(p.cnt > 0, mu.shape))
                push(p.name, cr=cr, **{f"h{k}": ch[k] for k in KS})
                # the order by the centre score alone, for reference
                o0, cr0, ch0 = curve(mu, p.cnt, hits, live=np.broadcast_to(p.cnt > 0, mu.shape))
                push(p.name + " | centre only", cr=cr0, **{f"h{k}": ch0[k] for k in KS})
                if f == 1:
                    s = sk[p.name]
                    qc = Q[q0:q0 + CHUNK]
                    base = qc @ s["mbar"]
                    proj = qc @ s["Vt"].T  # Q x rank
                    cproj = (p.cent - s["mbar"]) @ s["Vt"].T  # K x rank
                    rank_exact = np.empty_like(o)
                    np.put_along_axis(rank_exact, o, np.arange(p.K)[None, :].repeat(len(o), 0), 1)
                    for r in (8, 16, 32, 64):
                        if r >= cproj.shape[1]:
                            continue
                        muh = base[:, None] + proj[:, :r] @ cproj[:, :r].T
                        # the residual's spread over the clusters, per query (what the sketch leaves out)
                        res = mu - muh
                        so = np.argsort(-(muh + PSI * sd), axis=1, kind="stable")
                        rank_sk = np.empty_like(so)
                        np.put_along_axis(rank_sk, so, np.arange(p.K)[None, :].repeat(len(so), 0), 1)
                        push(p.name, **{f"sk{r}_rank": rank_sk, f"sk{r}_res": res.std(1), f"sk{r}_tot": mu.std(1)})
                    push(p.name, rank_exact=rank_exact)
                    # two levels: parents keyed as one cluster each
                    par, kp = s["par"], s["kp"]
                    pn = np.bincount(par, weights=p.cnt, minlength=kp)
                    P1 = np.zeros((p.K, kp))
                    P1[np.arange(p.K), par] = 1
                    ps1, ps2 = s1 @ P1, s2 @ P1
                    pmu = ps1 / np.maximum(pn, 1)
                    psd = np.sqrt(np.maximum(ps2 / np.maximum(pn, 1) - pmu * pmu, 0))
                    po = np.argsort(-(pmu + PSI * psd), axis=1, kind="stable")
                    prank = np.empty_like(po)
                    np.put_along_axis(prank, po, np.arange(kp)[None, :].repeat(len(po), 0), 1)
                    leaf_prank = prank[:, par]  # Q x K: the rank of each leaf's parent
                    nch = np.bincount(par, minlength=kp)
                    for P in (2, 4, 8, 16, 32):
                        if P >= kp:
                            continue
                        live = (leaf_prank < P) & (p.cnt > 0)
                        _, cr2, ch2 = curve(mu + PSI * sd, p.cnt, hits, live=live)
                        cs = kp + np.take_along_axis(np.broadcast_to(nch, po.shape), po[:, :P], 1).sum(1)
                        push(p.name + f" | two-level P={P}", cr=cr2, cs=cs, **{f"h{k}": ch2[k] for k in KS})
        for S, v, vp in visparts:
            Ssub = Sc[:, vp.rows]
            top = np.argpartition(-Ssub, 100, axis=1)[:, :100]
            top = np.take_along_axis(top, np.argsort(-np.take_along_axis(Ssub, top, 1), axis=1), 1)
            mu, sd, hits, _, _ = vp.stats(Ssub, top)
            _, cr, ch = curve(mu + PSI * sd, vp.cnt, hits, live=np.broadcast_to(vp.cnt > 0, mu.shape))
            push(vp.name, cr=cr, **{f"h{k}": ch[k] for k in KS})
        log("queries", q0 + CHUNK)
    res = {"dataset": ds, "N": N, "d": d, "nq": len(Q), "psi": PSI, "targets": TARGETS, "parts": {}}
    info = {p.name: (len(p.rows), p.K, S) for _, S, p in parts}
    info.update({vp.name: (len(vp.rows), vp.K, S / v) for S, v, vp in visparts})
    for name, a in acc.items():
        e = {"rows_clusters": {k: at_targets(a["cr"] / a["nq_cr"], a[f"h{k}"] / a[f"nq_h{k}"] / k) for k in KS}}
        base = name.split(" | ")[0]
        e["n"], e["K"], e["S"] = info[base]
        if "cs" in a:
            e["centres_scored"] = float(a["cs"] / a["nq_cs"])
        if "rank_exact" in a:
            rex = np.concatenate(a["rank_exact"])
            e["sketch"] = {}
            for tgt_i, tgt in enumerate(TARGETS):
                m = e["rows_clusters"][10][tgt_i][1]
                if m is None:
                    continue
                need = rex < int(np.ceil(m))
                for r in (8, 16, 32, 64):
                    if f"sk{r}_rank" not in a:
                        continue
                    rs = np.concatenate(a[f"sk{r}_rank"])[need]
                    e["sketch"].setdefault(str(r), {})[str(tgt)] = {"m": m, "depth99": int(np.quantile(rs, 0.99)) + 1, "depth999": int(np.quantile(rs, 0.999)) + 1}
            for r in (8, 16, 32, 64):
                if f"sk{r}_res" in a:
                    rr, tt = np.concatenate(a[f"sk{r}_res"]), np.concatenate(a[f"sk{r}_tot"])
                    e["sketch"].setdefault(str(r), {})["unexplained_var"] = float((rr ** 2).mean() / (tt ** 2).mean())
            e["evr"] = {str(r): float(sk[base]["evr"][r - 1]) for r in (8, 16, 32, 64) if r <= len(sk[base]["evr"])}
            e["parents"] = sk[base]["kp"]
        res["parts"][name] = e
    with open(outp, "w") as f:
        json.dump(res, f, indent=1)
    log("wrote", outp)


if __name__ == "__main__":
    main()
