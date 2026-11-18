# The exceedance table of policy.md (section 4) and its offline checks (4.4), on the real fit of one dataset.
#   ln P_u(tau) = g_L(zeta_u) + beta_L ln(rad_u / median rad),  zeta_u = (tau - mu_u) / s_u(q)
# Levels L: cluster, sub (16 sub-clusters per cluster, labels over the unchanged fit). Spread tiers:
#   S0  s_u = sd(q), the s.d. of q.m_c over the clusters
#   S1  s_u^2 = (rad_u^2 / tr Sigma_g) q' Sigma_g q, Sigma_g the group's pooled residual covariance (top 16
#       directions and an isotropic rest)
#   S2  the unit's own covariance: top 16 directions and an isotropic rest for a cluster (H r16's spread), top 4
#       for a sub-cluster; evaluated for every unit here (no laziness)
# Calibration (4.3), corpus only: 300 corpus rows as plain pseudo-queries (the row itself removed from the counts)
# and 300 shifted ones as scripts/orders5.py builds them (a x + r, a in [0.3, 0.4], r a random unit vector
# orthogonal to the top 64 corpus directions, normalised); thresholds at answer fractions 1e-6 .. 1e-1 of N; g a
# non-increasing function on 65 values of zeta (weighted isotonic fit of the Poisson rates), beta by Newton steps on
# the Poisson likelihood, alternated. The table is FITTED ON HALF of the pseudo-queries (even ones) and checked on
# the other half and on the suite's real queries, which are never used in the fit.
# usage: python3 -I calib.py DS      (prints the checks; caches the tables)
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *
import numpy as np

FRACS = (1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1)
NG = 65
LEVELS = ("cluster", "sub")
TIERS = ("S0", "S1", "S2")


def pava_dec(y, w):
    """Weighted isotonic regression, non-increasing."""
    y, w = list(-np.asarray(y, float)), list(np.asarray(w, float))
    vals, wts, cnt = [], [], []
    for yi, wi in zip(y, w):
        vals.append(yi); wts.append(wi); cnt.append(1)
        while len(vals) > 1 and vals[-2] > vals[-1]:
            ww = wts[-2] + wts[-1]
            v = (vals[-2] * wts[-2] + vals[-1] * wts[-1]) / ww if ww > 0 else max(vals[-2], vals[-1])
            c = cnt[-2] + cnt[-1]
            vals[-2:], wts[-2:], cnt[-2:] = [v], [ww], [c]
    return -np.repeat(vals, cnt)


class Table:
    def __init__(self, grid, g, beta, slope):
        self.grid, self.g, self.beta, self.slope = grid, g, beta, slope

    def lnP(self, zeta, lr):
        # beyond the grid the tail continues at its last fitted slope, so that far units are still ordered
        tail = self.slope * np.maximum(zeta - self.grid[-1], 0.0)
        return np.minimum(np.interp(zeta, self.grid, self.g) + tail + self.beta * lr, 0.0)

    def P(self, zeta, lr):
        return np.exp(self.lnP(zeta, lr))


def fit_table(zeta, lr, n, N, iters=8):
    lo, hi = np.quantile(zeta, 0.001), np.quantile(zeta[N > 0], 0.999) if (N > 0).any() else zeta.max()
    hi = max(hi, lo + 1e-6) + 0.05 * (max(hi, lo + 1e-6) - lo)
    edges = np.linspace(lo, hi, NG + 1)
    grid = 0.5 * (edges[1:] + edges[:-1])
    b = np.clip(np.searchsorted(edges, zeta) - 1, 0, NG - 1)
    beta = 0.0
    O = np.bincount(b, weights=N, minlength=NG)
    for _ in range(iters):
        e = n * np.exp(beta * lr)
        E = np.bincount(b, weights=e, minlength=NG)
        rate = np.where(E > 0, O / np.maximum(E, 1e-300), 0.0)
        g = np.log(np.maximum(pava_dec(rate, E), 1e-12))
        for _ in range(3):
            m = e * np.exp(g[b])
            grad = (N * lr).sum() - (m * lr).sum()
            hess = (m * lr * lr).sum()
            beta += float(np.clip(grad / max(hess, 1e-30), -1, 1))
            e = n * np.exp(beta * lr)
    # the tail: bins past the last one with answers (rate 0) and anything beyond the grid follow the slope of the
    # last 8 bins that hold answers
    pos = np.where(O > 0)[0]
    jl = int(pos[-1])
    j0 = max(jl - 8, 0)
    slope = min((g[jl] - g[j0]) / max(grid[jl] - grid[j0], 1e-12), 0.0)
    g[jl + 1:] = g[jl] + slope * (grid[jl + 1:] - grid[jl])
    return Table(grid, g, beta, slope)


class Fit:
    """The real fit's units and their features for a set of queries."""

    def __init__(self, D):
        self.D = D
        K = D.K
        self.cs = cached(D.ds, "st_real", lambda: leaf_stats(D.X, D.a, K, 16, D.Mq))
        z = subsplit(D, 16)
        self.sub, self.par = z["sub"], z["par"]
        self.L = len(self.par)
        self.ss = cached(D.ds, "st_sub16", lambda: leaf_stats(D.X, self.sub, self.L, 4))
        self.G = int(D.grp.max()) + 1
        self.gc = cached(D.ds, "grpcov16", self.group_cov)
        self.cent = dict(cluster=D.Mq.astype(np.float64), sub=self.ss["mean"].astype(np.float64))
        self.rad = dict(cluster=self.cs["rad"], sub=np.maximum(self.ss["rad"], 1e-6))
        self.lr = {k: np.log(v / np.median(v)) for k, v in self.rad.items()}
        self.n = dict(cluster=self.cs["n"].astype(np.float64), sub=self.ss["n"].astype(np.float64))
        self.grp = dict(cluster=D.grp, sub=D.grp[self.par])
        self.lab = dict(cluster=D.a, sub=self.sub)

    def group_cov(self):
        D, G = self.D, self.G
        d = D.d
        C = np.zeros((G, d, d)); cnt = np.zeros(G)
        o = np.argsort(D.a, kind="stable")
        st = np.r_[0, np.cumsum(np.bincount(D.a, minlength=D.K))]
        for c in range(D.K):
            x = D.X[o[st[c]:st[c + 1]]].astype(np.float64) - D.Mq[c].astype(np.float64)
            C[D.grp[c]] += x.T @ x
            cnt[D.grp[c]] += len(x)
        lam = np.zeros((G, 16)); U = np.zeros((G, 16, d), np.float32); tr = np.zeros(G)
        for g in range(G):
            Cg = C[g] / max(cnt[g], 1)
            w, v = np.linalg.eigh(Cg)
            lam[g], U[g], tr[g] = w[::-1][:16], v[:, ::-1][:, :16].T, np.trace(Cg)
        return dict(lam=lam, U=U, tr=tr)

    def feats(self, Q):
        """mu (per level), sd(q), and the S1 spread per level, for queries Q (nq x d f32)."""
        Q64 = Q.astype(np.float64)
        mu = {k: Q64 @ self.cent[k].T for k in LEVELS}
        sd = mu["cluster"].std(1)[:, None]
        sg = sigma(Q, self.gc, 16)  # nq x G
        s = {"S0": {k: np.broadcast_to(sd, mu[k].shape) for k in LEVELS},
             "S1": {k: sg[:, self.grp[k]] * (self.rad[k] / np.sqrt(self.gc["tr"][self.grp[k]]))[None] for k in LEVELS},
             "S2": dict(cluster=sigma(Q, self.cs, 16), sub=sigma(Q, self.ss, 4))}
        return mu, sd, s


def pseudo_queries(D, nq=300, seed=17):
    rng = np.random.default_rng(seed)
    rows = np.sort(rng.choice(D.N, 2 * nq, replace=False))
    X = D.X[rows].astype(np.float64)
    plain, prow, base = X[:nq], rows[:nq], X[nq:]
    Gs = D.X[np.sort(rng.choice(D.N, 50_000, replace=False))].astype(np.float64)
    w, V = np.linalg.eigh(Gs.T @ Gs / len(Gs))
    top = V[:, ::-1][:, :min(64, D.d // 2)]
    R = rng.standard_normal((nq, D.d))
    R -= (R @ top) @ top.T
    R /= np.linalg.norm(R, axis=1, keepdims=True)
    sh = rng.uniform(0.3, 0.4, nq)[:, None] * base + R
    sh /= np.linalg.norm(sh, axis=1, keepdims=True)
    return np.concatenate([plain, sh]).astype(np.float32), np.r_[prow, -np.ones(nq, np.int64)]


def counts_at(S, F, self_rows=None, vis=None, fracs=FRACS):
    """For each query (row of S, scores against the corpus): tau at each answer fraction of the visible rows, and
    the count of visible rows at or above it per unit of each level."""
    nq, N = S.shape
    ids = np.arange(N) if vis is None else np.where(vis)[0]
    taus = np.zeros((nq, len(fracs)))
    cn = {k: np.zeros((nq, len(fracs), len(F.n[k])), np.float32) for k in LEVELS}
    ranks = [max(1, int(round(f * len(ids)))) for f in fracs]
    for i in range(nq):
        s = S[i, ids].astype(np.float64)
        if self_rows is not None and self_rows[i] >= 0:
            s[self_rows[i]] = -np.inf  # only used without a visible set
        top = np.argpartition(-s, ranks[-1])[:ranks[-1] + 1]
        st = np.sort(s[top])[::-1]
        for j, r in enumerate(ranks):
            taus[i, j] = st[r - 1]
            sel = ids[top[s[top] >= taus[i, j]]]
            for k in LEVELS:
                cn[k][i, j] = np.bincount(F.lab[k][sel], minlength=len(F.n[k]))
    return taus, cn


def calib_data(D, F):
    def fn():
        PQ, prow = pseudo_queries(D)
        taus, cn = [], {k: [] for k in LEVELS}
        for i in range(0, len(PQ), 100):
            S = np.concatenate([PQ[i:i + 100] @ D.X[j:j + 100_000].T for j in range(0, D.N, 100_000)], 1)
            t, c = counts_at(S, F, prow[i:i + 100])
            taus.append(t)
            for k in LEVELS:
                cn[k].append(c[k])
        return dict(PQ=PQ, taus=np.concatenate(taus), **{"cn_" + k: np.concatenate(cn[k]) for k in LEVELS})
    return cached(D.ds, "calib16", fn)


def pairs(F, Q, taus, cn, level, tier, nvis=None, sel=None):
    """Flattened (query, tau, unit) pairs: zeta, lr, exposure n, count N."""
    mu, sd, s = F.feats(Q)
    m, ss = mu[level], s[tier][level]
    n = F.n[level] if nvis is None else nvis
    if sel is not None:
        m, ss, taus, cn = m[sel], ss[sel], taus[sel], cn[sel]
    zeta = (taus[:, :, None] - m[:, None, :]) / ss[:, None, :]
    lr = np.broadcast_to(F.lr[level][None, None, :], zeta.shape)
    nn = np.broadcast_to(n[None, None, :], zeta.shape)
    return zeta.ravel(), lr.ravel(), nn.ravel(), cn.ravel().astype(np.float64)


def tables(D, F):
    cd = calib_data(D, F)
    half = np.arange(0, len(cd["PQ"]), 2)
    out = {}
    for level in LEVELS:
        for tier in TIERS:
            z, lr, n, N = pairs(F, cd["PQ"], cd["taus"], cd["cn_" + level], level, tier, sel=half)
            ok = n > 0
            out[(level, tier)] = fit_table(z[ok], lr[ok], n[ok], N[ok])
    return out, cd


def reliability(T, z, lr, n, N):
    """delta: the largest |ln(observed / predicted)| over bins of predicted P (third-decades) holding 95% of the
    answers; and the overall ln(observed / predicted)."""
    ok = n > 0
    z, lr, n, N = z[ok], lr[ok], n[ok], N[ok]
    lp = T.lnP(z, lr)
    b = np.clip(np.floor(lp / np.log(10) * 3).astype(int) + 30, 0, 30)
    O = np.bincount(b, weights=N, minlength=31)
    E = np.bincount(b, weights=n * np.exp(lp), minlength=31)
    o = np.argsort(-O)
    keep = o[:int(np.searchsorted(np.cumsum(O[o]), 0.95 * O.sum()) + 1)]
    d = np.abs(np.log(np.maximum(O[keep], 1e-9) / np.maximum(E[keep], 1e-9)))
    return float(d.max()), float(np.log(O.sum() / E.sum()))


def kappa(T, F, Q, taus_j, level, tier, target, nvis=None):
    """Per query: ln( sum_u n_u P_u(tau_true) / target )."""
    mu, sd, s = F.feats(Q)
    n = F.n[level] if nvis is None else nvis
    P = T.P((taus_j[:, None] - mu[level]) / s[tier][level], F.lr[level][None])
    return np.log(np.maximum((n[None] * P).sum(1), 1e-12) / target)


def q3(x):
    return f"{np.median(x):+.2f} [{np.quantile(x, 0.1):+.2f}, {np.quantile(x, 0.9):+.2f}]"


if __name__ == "__main__":
    D = DS(sys.argv[1])
    F = Fit(D)
    T, cd = tables(D, F)
    nq = len(cd["PQ"]) // 2
    held = dict(plain=np.arange(1, nq, 2), shifted=np.arange(nq + 1, 2 * nq, 2))
    rt, rc = counts_at(D.S, F)
    print(f"== {D.name}: exceedance tables fitted on {nq} pseudo-queries (half plain, half shifted); checks on the held-out half and on the {D.nq} real queries")
    for (level, tier), t in T.items():
        print(f"\n-- level {level}, tier {tier}: beta {t.beta:+.3f}; tail slope {t.slope:.2f} per unit of zeta; zeta grid {t.grid[0]:.2f}..{t.grid[-1]:.2f}; ln P at the ends {t.g[0]:.2f} .. {t.g[-1]:.2f}")
        sets = {k: (cd["PQ"], cd["taus"], cd["cn_" + level], v) for k, v in held.items()}
        sets["real"] = (D.Q, rt, rc[level], None)
        for name, (Q, taus, cn, sel) in sets.items():
            z, lr, n, N = pairs(F, Q, taus, cn, level, tier, sel=sel)
            dl, tot = reliability(t, z, lr, n, N)
            Qs, ts = (Q, taus) if sel is None else (Q[sel], taus[sel])
            ks = []
            for j, fr in ((1, 1e-5), (3, 1e-3), (4, 1e-2)):
                ks.append(f"{fr:g}: " + q3(kappa(t, F, Qs, ts[:, j], level, tier, max(1, round(fr * D.N)))))
            # residual covariates: ln(obs/pred) by tercile of rad and of n
            res = []
            for nm, cov in (("rad", F.rad[level]), ("n", F.n[level])):
                ter = np.searchsorted(np.quantile(cov, [1 / 3, 2 / 3]), cov)
                tt = np.broadcast_to(ter[None, None, :], (len(Qs), len(FRACS), len(cov))).ravel()
                lp = t.lnP(z, lr)
                ok = n > 0
                r = [np.log(max(N[ok & (tt == k)].sum(), 1e-9) / max((n * np.exp(lp))[ok & (tt == k)].sum(), 1e-9)) for k in range(3)]
                res.append(f"{nm} terciles " + " ".join(f"{x:+.2f}" for x in r))
            print(f"{name:8s} delta {dl:.2f}; ln(obs/pred) overall {tot:+.2f}; ln kappa median [10%, 90%] at answer fraction " + "; ".join(ks) + "; residual ln(obs/pred) by " + ", ".join(res))
        # thinning: the real queries under visible sets, kappa at k = 10 and reliability
        for f in ("random-1", "random-25", "clus-1", "clus-25", "cap-1", "cap-25"):
            vis = D.visible(f)
            nvis = np.bincount(F.lab[level], weights=vis.astype(np.float64), minlength=len(F.n[level]))
            fr = (10 / vis.sum(), 1e-2)
            vt, vc = counts_at(D.S, F, vis=vis, fracs=fr)
            z, lr, n, N = pairs(F, D.Q, vt, vc[level], level, tier, nvis=nvis)
            dl, tot = reliability(t, z, lr, n, N)
            print(f"real, {f:9s} delta {dl:.2f}; ln(obs/pred) {tot:+.2f}; ln kappa at k = 10: {q3(kappa(t, F, D.Q, vt[:, 0], level, tier, 10, nvis))}; at 1% of the visible: {q3(kappa(t, F, D.Q, vt[:, 1], level, tier, max(1, round(0.01 * vis.sum())), nvis))}")
