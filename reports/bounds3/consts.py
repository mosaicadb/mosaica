# Constants per configuration: sub-Gaussian, single block exact, Chernoff min-max as tail.rs computes it, the exact
# worst case over shares (local optimisation; the certificates are in certs.py), and the convex-order bound (C1).
import numpy as np, math, sys, time, json
import mpmath as mp
from scipy.optimize import minimize
from law import c1, lnmgf
from ctable import quantiles

def blocks(n):
    fr = [0.125, 0.29167, 0.625]
    b = [0] + [int(round(f * n / 16)) * 16 for f in fr] + [n]
    for j in range(1, len(b)):
        b[j] = max(b[j], b[j - 1] + 16)
    return [b[j + 1] - b[j] - 1 for j in range(4)]

def chernoff_minmax(ds, l1, K=1000):
    """tail.rs level_constant for several blocks: min over lambda of max over rounded shares (dynamic programme)."""
    J = len(ds); csub = math.sqrt(2 * l1); top = K + J - 1; best = np.inf
    i = np.arange(K + 1)
    for q in range(64):
        lam = csub * 0.7 * (3 / 0.7) ** (q / 63)
        acc = np.full(top + 1, -np.inf); acc[0] = 0.0
        for d in ds:
            g = np.zeros(K + 1); g[1:] = lnmgf(d, lam * np.sqrt(i[1:] / K * d))
            nx = np.full(top + 1, -np.inf)
            for ii in range(K + 1):
                np.maximum(nx[ii:], acc[:top + 1 - ii] + g[ii], out=nx[ii:])
            acc = nx
        best = min(best, (l1 + acc[K:].max()) / lam)
    return min(best, csub)

def chernoff_maxmin(ds, l1):
    """sup over shares of the per-share Chernoff constant (what an exact max-min would give)."""
    def c_ch(w):
        lam = np.linspace(0.75, 1.6, 400) * math.sqrt(2 * l1)
        v = l1 + sum(lnmgf(d, lam * math.sqrt(x * d)) for x, d in zip(w, ds) if x > 0)
        return (v / lam).min()
    D2 = np.array(ds, float) + 2
    f = lambda th: -c_ch(np.exp(th - th.max()) / np.exp(th - th.max()).sum())
    r = minimize(f, np.log(D2 / D2.sum()), method="Nelder-Mead", options=dict(xatol=1e-5, fatol=1e-9))
    return -r.fun

def exact_sup(ds, lnd):
    """sup over shares of the exact quantile (Nelder-Mead from the fourth-cumulant point and the centre)."""
    J = len(ds)
    if J == 1: return c1(ds[0], lnd), np.array([1.0])
    D2 = np.array(ds, float) + 2
    def f(th):
        w = np.exp(th - th.max()); w /= w.sum()
        return -quantiles(w[None], ds, [lnd])[0, 0]
    best = (-1, None)
    for s in (np.log(D2 / D2.sum()), np.zeros(J)):
        r = minimize(f, s, method="Nelder-Mead", options=dict(xatol=1e-5, fatol=1e-9, maxiter=800))
        w = np.exp(r.x - r.x.max()); w /= w.sum()
        if -r.fun > best[0]: best = (-r.fun, w)
    return best

def convex_order(ds, lnd):
    """C1: sum_j a_j U_j <=_cx s U_D with D = sum d_j, s^2 = sum a_j^2 / m_j^2, m_j = E sqrt(Beta(d_j/2, (D-d_j)/2));
    so P[X >= x] <= inf_{t < x} E (s U_D - t)_+ / (x - t). Returns the constant in units of the sd of X."""
    mp.mp.dps = 40
    D = sum(ds)
    if len(ds) == 1: return None
    m = [mp.exp(mp.loggamma(mp.mpf(d) / 2 + mp.mpf(1) / 2) + mp.loggamma(mp.mpf(D) / 2) - mp.loggamma(mp.mpf(d) / 2) - mp.loggamma(mp.mpf(D) / 2 + mp.mpf(1) / 2)) for d in ds]
    theta = max(mp.sqrt(mp.mpf(d) / (D * mj * mj)) for d, mj in zip(ds, m))     # s <= theta sqrt(D) sigma
    lC = mp.loggamma(mp.mpf(D) / 2) - mp.log(mp.pi) / 2 - mp.loggamma(mp.mpf(D - 1) / 2)
    def stoploss(t):       # E (U_D - t)_+
        return mp.exp(lC + (mp.mpf(D - 1) / 2) * mp.log(1 - t * t)) / (D - 1) - t * mp.betainc(mp.mpf(D - 1) / 2, mp.mpf(1) / 2, 0, 1 - t * t, regularized=True) / 2
    def bound(tau):        # ln inf_t E(U - t)_+ / (tau - t), golden section on t in (0, tau)
        f = lambda t: mp.log(stoploss(t)) - mp.log(tau - t)
        a_, b_ = mp.mpf(0), tau * (1 - mp.mpf('1e-9')); gr = (mp.sqrt(5) - 1) / 2
        c_, d_ = b_ - gr * (b_ - a_), a_ + gr * (b_ - a_)
        for _ in range(120):
            if f(c_) < f(d_): b_ = d_
            else: a_ = c_
            c_, d_ = b_ - gr * (b_ - a_), a_ + gr * (b_ - a_)
        return f((a_ + b_) / 2)
    lo, hi = mp.mpf(5) / mp.sqrt(D), mp.mpf(12) / mp.sqrt(D)
    for _ in range(60):
        mid = (lo + hi) / 2
        if bound(mid) > lnd: lo = mid
        else: hi = mid
    return float(hi * mp.sqrt(D) * theta), float(theta)

if __name__ == "__main__":
    eps = 1e-9; L = 4
    out = {}
    for n in (512, 768, 1536):
        ds = blocks(n)
        for N in (1e6, 1e9):
            for side, name in ((2, "two"), (1, "one")):
                l1 = math.log(side * N * L / eps); lnd = -l1
                t0 = time.time()
                row = dict(n=n, ds=ds, N=N, side=name, l1=l1, subg=math.sqrt(2 * l1), single=[c1(d, lnd) for d in ds])
                row["chern_minmax"] = chernoff_minmax(ds, l1)
                row["chern_maxmin"] = chernoff_maxmin(ds, l1)
                c, w = exact_sup(ds, lnd); row["exact_sup"] = c; row["argmax"] = [float(x) for x in w]
                row["sphere_Dp"] = c1(sum(ds) + 2 * len(ds) - 2, lnd)
                cx, th = convex_order(ds, lnd); row["convex_order"] = cx; row["theta"] = th
                for sub in ([0, 1], [0, 1, 2]):
                    dsub = [ds[j] for j in sub]
                    row[f"exact_sup_{len(sub)}"] = exact_sup(dsub, lnd)[0]
                out[f"{n}_{int(N)}_{name}"] = row
                print(f"n {n} ds {ds} N {N:.0e} {name}-sided: subG {row['subg']:.3f}; single {np.round(row['single'], 3)}; Chernoff minmax {row['chern_minmax']:.3f} (maxmin {row['chern_maxmin']:.3f}); "
                      f"exact sup {c:.4f} at {np.round(w, 4)}; sphere(D') {row['sphere_Dp']:.4f}; convex-order {cx:.3f} (theta {th:.5f}); two blocks {row['exact_sup_2']:.3f}, three {row['exact_sup_3']:.3f}  [{time.time()-t0:.0f}s]", flush=True)
    json.dump(out, open("consts.json", "w"), indent=1)
