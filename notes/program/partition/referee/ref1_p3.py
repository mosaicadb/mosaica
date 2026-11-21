# Referee 1, attacks on P3.1 and P3.5. numpy only, synthetic data.
import numpy as np
from scipy.optimize import brentq

rng = np.random.default_rng(17)


def sqrtm_psd(A):
    l, v = np.linalg.eigh(A)
    return (v * np.sqrt(np.maximum(l, 0))) @ v.T


def F(S, SQh):
    return np.sqrt(np.maximum(np.linalg.eigvalsh(SQh @ S @ SQh), 0)).sum()


def metric(S, SQh, SQhi):
    """M = Sigma_Q^-1 # S and its inverse."""
    Bh = sqrtm_psd(SQh @ S @ SQh)
    return SQhi @ Bh @ SQhi, SQh @ np.linalg.inv(Bh) @ SQh


def setup(n, K, kinds, follow_position, rows=(40, 200)):
    SQ = np.cov(rng.normal(size=(4 * n, n)).T) + 0.1 * np.eye(n)
    SQh = sqrtm_psd(SQ); SQhi = np.linalg.inv(SQh)
    shapes = []
    for k in range(kinds):
        A = rng.normal(size=(n, n)) * np.exp(rng.normal(0, 1.2, n))
        shapes.append(A @ A.T / n)
    covs, ns, kind = [], [], []
    for c in range(K):
        k = c * kinds // K if follow_position else int(rng.integers(kinds))
        m = int(rng.integers(*rows))
        L = np.linalg.cholesky(shapes[k] + 1e-9 * np.eye(n))
        R = rng.normal(size=(m, n)) @ L.T * np.exp(rng.normal(0, 0.3))
        R -= R.mean(0)
        covs.append(R.T @ R / m); ns.append(m); kind.append(k)
    return SQ, SQh, SQhi, np.array(covs), np.array(ns), np.array(kind)


def pooled(covs, ns, a, G):
    out = []
    for g in range(G):
        m = a == g
        out.append((covs[m] * ns[m, None, None]).sum(0) / ns[m].sum() if m.any() else None)
    return out


def Phi(covs, ns, a, G, SQh):
    return sum(ns[a == g].sum() * F(P, SQh) ** 2 for g, P in enumerate(pooled(covs, ns, a, G)) if P is not None)


print("== P3.1 ==")
stats = dict(rises=0, steps=0, price_viol=0, id5=0.0, right4=0, left4=0, single_better=0, fixed_points=0, loo_rises=0, loo_steps=0)
for trial in range(40):
    n = int(rng.integers(3, 9)); K = int(rng.integers(8, 40)); G = int(rng.integers(2, 6))
    SQ, SQh, SQhi, covs, ns, kind = setup(n, K, kinds=int(rng.integers(2, 6)), follow_position=bool(trial % 2))
    a = np.arange(K) % G; rng.shuffle(a)
    trQ = lambda M: np.trace(M @ SQ)
    hist = [Phi(covs, ns, a, G, SQh)]
    for it in range(200):
        P = pooled(covs, ns, a, G)
        cost = np.full((K, G), np.inf)
        for g in range(G):
            if P[g] is None:
                continue
            M, Mi = metric(P[g], SQh, SQhi)
            cost[:, g] = trQ(M) * np.einsum("ij,cji->c", Mi, covs)
        # item 1: value at the geometric mean
        v1 = (ns * cost[np.arange(K), a]).sum()
        assert abs(v1 - hist[-1]) < 1e-8 * v1
        new = np.where(cost.min(1) < cost[np.arange(K), a] - 1e-12 * cost[np.arange(K), a], cost.argmin(1), a)
        if (new == a).all():
            break
        v2 = (ns * cost[np.arange(K), new]).sum()
        a = new
        hist.append(Phi(covs, ns, a, G, SQh))
        stats["steps"] += 1
        stats["rises"] += (v2 > v1 * (1 + 1e-10)) + (hist[-1] > v2 * (1 + 1e-10))
    # item 4 and 5 at the fixed point
    left = (ns * np.array([F(C, SQh) ** 2 for C in covs])).sum()
    Wp = (covs * ns[:, None, None]).sum(0) / ns.sum()
    right = ns.sum() * F(Wp, SQh) ** 2
    stats["left4"] += left > hist[-1] * (1 + 1e-10); stats["right4"] += hist[-1] > right * (1 + 1e-10)
    kmax = np.bincount(a, minlength=G).max()
    stats["price_viol"] += hist[-1] / left > kmax + 1e-9
    d = cost[np.arange(K), a] - np.array([F(C, SQh) ** 2 for C in covs])
    assert d.min() > -1e-9
    stats["id5"] = max(stats["id5"], abs((ns * d).sum() - (hist[-1] - left)) / hist[-1])
    # is the fixed point a minimum over moves of one cell, with both metrics refitted?
    stats["fixed_points"] += 1
    base = hist[-1]; better = 0
    for c in range(K):
        for g in range(G):
            if g != a[c] and (a == a[c]).sum() > 1:
                b = a.copy(); b[c] = g
                if Phi(covs, ns, b, G, SQh) < base * (1 - 1e-9):
                    better += 1
    stats["single_better"] += better > 0
    # P3.3's variant: compare a cell with each group's pooled covariance with the cell's own rows left out
    b = np.arange(K) % G; rng.shuffle(b); prev = Phi(covs, ns, b, G, SQh)
    for it in range(60):
        new = b.copy()
        for c in range(K):
            best, bg = np.inf, b[c]
            for g in range(G):
                m = (b == g); m[c] = False
                if not m.any():
                    continue
                Pg = (covs[m] * ns[m, None, None]).sum(0) / ns[m].sum()
                if np.linalg.eigvalsh(Pg).min() < 1e-10:
                    continue
                M, Mi = metric(Pg, SQh, SQhi)
                v = trQ(M) * np.trace(Mi @ covs[c])
                if v < best:
                    best, bg = v, g
            new[c] = bg
        if (new == b).all():
            break
        b = new; cur = Phi(covs, ns, b, G, SQh)
        stats["loo_steps"] += 1; stats["loo_rises"] += cur > prev * (1 + 1e-10); prev = cur
print(stats)
print("  rises: steps of the alternation that raised Psi (should be 0); left4/right4/price_viol: violations of item 4; id5: worst relative error of item 5's identity;")
print("  single_better: fixed points (of", stats["fixed_points"], ") from which moving ONE cell and refitting both metrics lowers the functional;")
print("  loo_rises: passes of the leave-own-rows-out variant of P3.3 that raised the functional, of loo_steps.")

# the generator: -F^2 is convex but linear along rays, d(tY, Y) = 0
n = 5; SQ, SQh, SQhi, covs, ns, kind = setup(n, 4, 2, False)
Y = covs[0] + 0.01 * np.eye(n); M, Mi = metric(Y, SQh, SQhi)
dB = lambda X: np.trace(M @ SQ) * np.trace(Mi @ X) - F(X, SQh) ** 2
print("d(X, Y) for X = tY, t = 0.1, 1, 7:", [float(dB(t * Y)) for t in (0.1, 1, 7)], "; for an unrelated X:", float(dB(covs[1])), "; F(Y)^2 =", F(Y, SQh) ** 2)
# a singular first argument is allowed, a singular second argument is not
u = rng.normal(size=(n, 1)); print("d(rank-one X, Y) =", float(dB(u @ u.T)), "(finite)")

print("== P3.5 ==")


def waterfill(sig, H):
    """s_i = min(1, lam/sig_i) with sum ln(1/s_i) = H; returns s and W = sum s_i sig_i."""
    if H <= 0:
        return np.ones_like(sig), sig.sum()
    f = lambda ll: np.maximum(np.log(sig) - ll, 0).sum() - H
    ll = brentq(f, np.log(sig.max()) - H - 60, np.log(sig.max()))
    s = np.minimum(1, np.exp(ll) / sig)
    return s, (s * sig).sum()


CODED = []


def row_scheme(X, cell, grp, SQ, H, iters=60, do_c=True):
    """Steps (a), (b), (c) of P3.5 with the idealised level cost W_g sum_i s_i z_i^2 (H = 0: nothing read)."""
    N, n = X.shape; K = cell.max() + 1; G = grp.max() + 1
    SQh = sqrtm_psd(SQ); SQhi = np.linalg.inv(SQh)
    obj = []; A = [None] * G

    def fit_group(g, mu):
        rows = np.isin(cell, np.where(grp == g)[0])
        R = X[rows] - mu[cell[rows]]
        P = R.T @ R / max(rows.sum(), 1) + 1e-10 * np.eye(n)
        gam, U = np.linalg.eigh(SQh @ P @ SQh); gam = np.maximum(gam, 1e-14)
        s, Wg = waterfill(np.sqrt(gam), H)
        CODED.append(int((s < 1).sum()))
        Vinv = (U * gam ** -0.25).T @ SQh                      # z = V^-1 r
        return Wg * Vinv.T @ (Vinv * s[:, None])               # quadratic form of the row cost

    mu = np.stack([X[cell == c].mean(0) if (cell == c).any() else np.zeros(n) for c in range(K)])
    for g in range(G):
        A[g] = fit_group(g, mu)
    rises = 0
    val = lambda: sum(np.einsum("ij,jk,ik->i", X[cell == c] - mu[c], A[grp[c]], X[cell == c] - mu[c]).sum() for c in range(K))
    obj.append(val())
    for it in range(iters):
        d = np.stack([np.einsum("ij,jk,ik->i", X - mu[c], A[grp[c]], X - mu[c]) for c in range(K)], 1)
        cur = d[np.arange(N), cell]
        new = np.where(d.min(1) < cur * (1 - 1e-12), d.argmin(1), cell)
        if (new == cell).all():
            break
        cell = new; va = val(); rises += va > obj[-1] * (1 + 1e-10)
        mu = np.stack([X[cell == c].mean(0) if (cell == c).any() else mu[c] for c in range(K)])
        vb = val(); rises += vb > va * (1 + 1e-10)
        if do_c:
            for g in range(G):
                A[g] = fit_group(g, mu)
        vc = val(); rises += vc > vb * (1 + 1e-10)
        obj.append(vc)
    return cell, obj, rises


tot_r = 0; runs = 0
for trial in range(12):
    n = int(rng.integers(3, 8)); K = 12; G = 3; N = 4000
    cen = rng.normal(size=(K, n)) * 2
    shp = [rng.normal(size=(n, n)) * np.exp(rng.normal(0, 1, n)) for _ in range(2)]
    c0 = rng.integers(0, K, N); kind = rng.integers(0, 2, N)
    X = cen[c0] + np.einsum("ij,ikj->ik", rng.normal(size=(N, n)), np.stack(shp)[kind]) * 0.6
    SQ = np.cov(rng.normal(size=(5 * n, n)).T) + 0.1 * np.eye(n)
    cell = ((X[:, None, :] - cen[None]) ** 2).sum(2).argmin(1)
    grp = np.arange(K) % G
    for H in [0.0, 2.0, 8.0]:
        _, obj, r = row_scheme(X, cell.copy(), grp, SQ, H)
        tot_r += r; runs += 1
        if trial < 3:
            print(f"trial {trial} n {n} H {H}: objective {obj[0]:.1f} -> {obj[-1]:.1f} in {len(obj) - 1} passes, rises {r}")
print("row scheme: steps that raised the objective, over", runs, "runs at H = 0, 2, 8 nats:", tot_r)

# the level form's step (c): no basis beats the dual pair with water-filling, in the idealised cost?
worst = np.inf
for trial in range(300):
    n = int(rng.integers(2, 7)); gam = np.exp(rng.normal(0, 1.5, n)); H = rng.uniform(0, 6)
    s, Wst = waterfill(np.sqrt(gam), H)
    V = rng.normal(size=(n, n)) if trial % 2 else np.linalg.qr(rng.normal(size=(n, n)))[0]
    Vi = np.linalg.inv(V)
    q = (V ** 2).sum(0); a = np.einsum("ij,j,ij->i", Vi, gam, Vi)   # Sigma_Q = I, B = diag(gam)
    sig = np.sqrt(q * a)
    worst = min(worst, waterfill(sig, H)[1] / Wst)
print("least (best allocation in a random basis pair) / W(H) over 300 trials:", worst, "(>= 1 means the dual pair is not beaten)")

# the level form at two levels: do the two levels pick the same cell for a row?
n, K, G, N = 6, 12, 3, 4000
cen = rng.normal(size=(K, n)) * 1.2
shp = [rng.normal(size=(n, n)) * np.exp(rng.normal(0, 1, n)) for _ in range(2)]
X = cen[rng.integers(0, K, N)] + np.einsum("ij,ikj->ik", rng.normal(size=(N, n)), np.stack(shp)[rng.integers(0, 2, N)]) * 0.6
SQ = np.cov(rng.normal(size=(5 * n, n)).T) + 0.1 * np.eye(n)
cell = ((X[:, None, :] - cen[None]) ** 2).sum(2).argmin(1); grp = np.arange(K) % G
picks = {}
for H in [0.0, 3.0, 10.0]:
    CODED.clear()
    c1, obj, _ = row_scheme(X, cell.copy(), grp, SQ, H, iters=1, do_c=False)
    picks[H] = c1
    print('H', H, 'coordinates above the water level per group, of', n, ':', CODED[:G])
print("one assignment step from the same start; rows whose best cell differs between levels: H 0 vs 3: %.3f, H 0 vs 10: %.3f, H 3 vs 10: %.3f; rows moved at H = 0: %.3f" %
      ((picks[0.0] != picks[3.0]).mean(), (picks[0.0] != picks[10.0]).mean(), (picks[3.0] != picks[10.0]).mean(), (picks[0.0] != cell).mean()))
