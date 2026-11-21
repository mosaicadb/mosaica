"""Referee 1, checks on A2.1 and A2.2. Small numpy; one core.

A. A2.1 in its own model (M3 with a known shape): the identity E[X(t_hat) at the stop] = E[M at the stop], the
   recall at the stop, and for item 4 the two readings of kappa (the largest ratio G/G_hat over the unread
   clusters, and M/M_hat) when the stop uses a normal tail on Student-t(5) scores.
B. A2.2's constant: E min(k, Binomial(W, m/W)) against (1/2) min(k, m).
"""
import math
import numpy as np
from scipy.stats import norm, t as student, binom

rng = np.random.default_rng(3)
K, k, r = 300, 10, 0.9
T5 = math.sqrt(5 / 3)                      # sd of t(5)


def sf_true(z, shape):
    return norm.sf(z) if shape == "normal" else student.sf(z * T5, 5)


def run(shape, tail_used, order, queries):
    rec = []; Xs = []; Ms = []; Mh = []; miss = []; kmax = []; reads = []
    for _ in range(queries):
        v = rng.integers(100, 401, K)
        mu = rng.normal(0, 1, K); sg = 0.3 * np.exp(0.25 * rng.normal(0, 1, K))
        z = rng.normal(0, 1, (K, 400)) if shape == "normal" else rng.standard_t(5, (K, 400)) / T5
        s = mu[:, None] + sg[:, None] * z
        s[np.arange(400)[None, :] >= v[:, None]] = -np.inf
        tau = np.partition(s.ravel(), -k)[-k]
        ncl = (s >= tau).sum(1)                               # true neighbours per cluster
        unread = np.ones(K, bool); held = np.full(k, -np.inf)
        key = mu + 3 * sg
        while True:
            if order == "static" or not np.isfinite(held.min()):
                c = int(np.argmax(np.where(unread, key, -np.inf)))
            else:                                             # gap order, re-keyed at the running k-th best
                c = int(np.argmin(np.where(unread, (held.min() - mu) / sg, np.inf)))
            unread[c] = False
            held = np.partition(np.concatenate([held, s[c, :v[c]]]), -k)[-k:]
            th = held.min()
            zc = (th - mu[unread]) / sg[unread]
            m_true = float((v[unread] * sf_true(zc, shape)).sum())
            m_used = float((v[unread] * sf_true(zc, tail_used)).sum())
            if m_used < (1 - r) * k or not unread.any():
                break
        X = int((s[unread] >= th).sum())
        ms = int(ncl[unread].sum())
        rec.append(1 - ms / k); Xs.append(X); Ms.append(m_true); Mh.append(m_used); miss.append(ms)
        reads.append(K - unread.sum())
        if unread.any() and tail_used != shape:
            with np.errstate(divide="ignore", invalid="ignore"):
                ratio = sf_true(zc, shape) / sf_true(zc, tail_used)
            kmax.append(float(np.nanmax(ratio)))
    return map(np.array, (rec, Xs, Ms, Mh, miss, kmax, reads))


print("A. A2.1 in model M3, K 300 clusters of 100 to 400 rows, k 10, stop at the first read with M_used < 1")
for shape, used, order, q in (("normal", "normal", "static", 1500), ("normal", "normal", "gap", 1500),
                              ("t5", "t5", "static", 1500), ("t5", "t5", "gap", 1500),
                              ("t5", "normal", "static", 1500)):
    rec, X, M, Mh, miss, kmax, reads = run(shape, used, order, q)
    se = lambda a: a.std() / math.sqrt(len(a))
    print(f"   scores {shape:6s} stop's tail {used:6s} order {order:6s}: clusters read {reads.mean():5.1f};  E X {X.mean():.3f} +- {se(X):.3f}  E M(true tail) {M.mean():.3f}"
          f"  E M_used {Mh.mean():.3f};  E Miss {miss.mean():.3f};  recall {rec.mean():.4f};  share of queries with recall < 0.9: {(rec < 0.9 - 1e-9).mean():.3f}")
    lo = M < np.median(M)
    print(f"        lower half of M at the stop: E X {X[lo].mean():.3f}, E M {M[lo].mean():.3f};  upper half: E X {X[~lo].mean():.3f}, E M {M[~lo].mean():.3f}")
    if len(kmax):
        kw = M / Mh
        print(f"        kappa as the largest G/G_hat over unread clusters: median {np.median(kmax):.3g}, 10th pct {np.quantile(kmax, .1):.3g};"
              f"  kappa as M/M_hat: mean {kw.mean():.2f}, median {np.median(kw):.2f}, 90th pct {np.quantile(kw, .9):.2f};"
              f"  bound 1 - E[M/M_hat * M_hat]/k = {1 - M.mean() / k:.3f}")

print("\nB. A2.2: least value of E min(k, Bin(W, m/W)) / ((1/2) min(k, m)) over a grid")
worst = (9, None)
for kk in (1, 2, 3, 5, 10, 30, 100):
    for W in (kk, 2 * kk + 1, 50, 1000, 100000):
        if W < kk:
            continue
        for m in np.concatenate([np.geomspace(1e-3, 1, 25), np.linspace(1, 3 * kk, 60)]):
            if m > W:
                continue
            e = binom.sf(np.arange(kk), W, m / W).sum()
            ratio = e / (0.5 * min(kk, m))
            if ratio < worst[0]:
                worst = (ratio, (kk, W, round(float(m), 4)))
print(f"   least ratio {worst[0]:.4f} at (k, W, m) = {worst[1]}  (the claim needs it to be at least 1)")
