"""The B3 construction at n = 128 and n = 64 (not in the claims' table), at the block's own constant."""
import numpy as np
from scipy import special, optimize

rng = np.random.default_rng(12)


def hadamard(n):
    k = np.arange(n); x = k[:, None] & k[None, :]
    pc = np.zeros((n, n), dtype=np.int64)
    while x.any():
        pc += x & 1; x >>= 1
    return (1.0 - 2.0 * (pc % 2)) / np.sqrt(n)


def sphere_c(d, delta):
    return optimize.brentq(lambda c: 0.5 * special.betainc((d - 1) / 2, 0.5, 1 - c * c / d) - delta, 1.0, np.sqrt(d) * 0.999, xtol=1e-12)


def xt(t, total=6_000_000):
    Ht = hadamard(t); chi = 1.0 - 2.0 * (np.arange(t) & 1)
    U = []; N = []
    if t <= 16:
        idx = np.arange(2 ** t, dtype=np.int64)
        batches = [1.0 - 2.0 * ((idx[:, None] >> np.arange(t)[None, :]) & 1)]
    else:
        batches = (rng.choice((-1.0, 1.0), (100000, t)) for _ in range(total // 100000))
    for eps in batches:
        zeta = np.round((eps @ Ht) * np.sqrt(t)) / t
        xi = ((eps * chi) @ Ht) / np.sqrt(t)
        o = np.where(zeta >= 0, 0.5, -0.5)
        kappa = (zeta * zeta).sum(1) / (o * zeta).sum(1)
        e = kappa[:, None] * o - zeta
        A = np.maximum(np.linalg.norm(e, axis=1), 1e-300)
        U.append((e * xi).sum(1) / (A / np.sqrt(t - 1))); N.append((np.where(zeta != 0, e, 0.0) * xi).sum(1) / (A / np.sqrt(t - 1)))
    return np.concatenate(U), np.concatenate(N)


cache = {t: xt(t) for t in (8, 16, 32, 64)}
for n in (64, 128):
    c = sphere_c(n - 1, 2.5e-19)
    print(f"n = {n}: one-block constant {c:.3f}, sqrt(n-1) = {np.sqrt(n-1):.2f}")
    for k in (1, 2, 3):
        for t in (8, 16, 32, 64):
            if t >= n:
                continue
            V = n // t
            lp = ((1 + np.log2(V) - V) + (1 - V)) * (k // 2) + (1 + np.log2(t) - t) * ((k - 1) // 2)
            up, no = cache[t]
            thr = c * np.sqrt((t - 1) / (n - 1))
            pu = np.mean(np.abs(up) > thr); pn = np.mean(np.abs(no) > thr)
            mix = pu if k == 1 else (t / n) * pu + (1 - t / n) * pn
            print(f"   k={k} t={t:2d}: log2 P(structure) {lp:6.1f}; P_up {pu:.2e} P_none {pn:.2e}; max |X| sqrt((n-1)/(t-1)) = {max(np.abs(up).max(), np.abs(no).max()) * np.sqrt((n-1)/(t-1)):.2f}; bound 2^{lp + (np.log2(mix) if mix > 0 else -np.inf):.1f}")
