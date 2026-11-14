# The largest exact quantile over variance shares, by local optimisation from several starts (not a certificate).
import numpy as np, math, time, sys
from scipy.optimize import minimize
from law import cw, c1

def blocks(n):
    fr = [0.125, 0.29167, 0.625]
    b = [0] + [int(round(f * n / 16)) * 16 for f in fr] + [n]
    for j in range(1, len(b)):
        b[j] = max(b[j], b[j - 1] + 16)
    return [b[j + 1] - b[j] for j in range(4)]

def sup_c(ds, lnd, starts=None, verbose=False):
    J = len(ds)
    if J == 1:
        return c1(ds[0], lnd), [1.0]
    def f(th):
        w = np.exp(th - th.max()); w = w / w.sum()
        return -cw(w, ds, lnd)
    best = (-1, None)
    D2 = np.array(ds, float) + 2
    starts = starts or [np.log(D2 / D2.sum()), np.zeros(J)]
    for s in starts:
        r = minimize(f, s, method="Nelder-Mead", options=dict(xatol=1e-4, fatol=1e-7, maxiter=600))
        w = np.exp(r.x - r.x.max()); w /= w.sum()
        if verbose: print("   start", np.round(np.exp(s) / np.exp(s).sum(), 3), "->", round(-r.fun, 5), np.round(w, 4))
        if -r.fun > best[0]: best = (-r.fun, w)
    return best

if __name__ == "__main__":
    t0 = time.time()
    L1 = math.log(2 * 1e9 * 4 / 1e-9)
    ds = [95, 127, 255, 287]
    c, w = sup_c(ds, -L1, verbose=True)
    D2 = np.array(ds) + 2.0
    print("sup c(w) =", round(c, 5), "at", np.round(w, 4), "; (d+2)/sum", np.round(D2 / D2.sum(), 4), f"[{time.time()-t0:.1f}s]")
    print("one sphere coordinate at D = sum d_j:", round(c1(sum(ds), -L1), 5), "; at D = sum(d_j + 2) - 2:", round(c1(sum(ds) + 6, -L1), 5))
