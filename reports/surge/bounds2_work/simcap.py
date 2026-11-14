# Toy check of the data-dependent constants for verified-inclusion threshold queries (one level, L = 1).
# Corpus fixed, Haar rotation redrawn per trial, 1-bit code, exact sphere law. Each item has a scalar
# z_x = (t - est_x) / (tan(phi_x) sqrt(1 - t^2)); it is excluded at sphere quantile s iff z_x > s.
# A match is wrongly excluded iff z_x > s. Schemes choose s from counts; we count Pr[some match excluded].
import sys, math, numpy as np
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from consts2 import tail
n = 16; m = n - 1; N = 300; t = 0.5; EPS = float(sys.argv[1]); TR = int(sys.argv[2]); B = 2000
rng = np.random.default_rng(int(sys.argv[3]) if len(sys.argv) > 3 else 1)
_c = {}
def squant(d):                      # s with P[u1 >= s] = d on the unit sphere of R^m
    d = min(d, 0.5)
    if d not in _c:
        lo, hi = 0.0, 1.0
        for _ in range(60):
            mid = (lo + hi) / 2
            if tail(m, mid) > d: lo = mid
            else: hi = mid
        _c[d] = hi
    return _c[d]
def corpus(nS, mode):
    q = np.zeros(n); q[0] = 1
    X = np.zeros((N, n))
    sc = np.full(N, t); sc[nS:] = rng.uniform(t - 0.7, t - 0.01, N - nS)     # matches sit exactly at t (gap 0)
    V = rng.standard_normal((N, n)); V[:, 0] = 0
    if mode == 'dup': V[:nS] = V[0] + 1e-3 * rng.standard_normal((nS, n)); V[:, 0] = 0
    V /= np.linalg.norm(V, axis=1, keepdims=True)
    X = sc[:, None] * q[None, :] + np.sqrt(1 - sc * sc)[:, None] * V
    return q, X
def zstats(q, X, nb):
    G = rng.standard_normal((nb, n, n)); Qm, Rm = np.linalg.qr(G)
    P = Qm * np.sign(np.einsum('bii->bi', Rm))[:, None, :]
    Z = np.einsum('bij,xj->bxi', P, X); Pq = np.einsum('bij,j->bi', P, q)
    code = np.sign(Z) / math.sqrt(n)
    cos = (code * Z).sum(2); est = np.einsum('bxi,bi->bx', code, Pq) / cos
    tan = np.sqrt(np.maximum(1 - cos * cos, 0)) / cos
    return (t - est) / (tan * math.sqrt(1 - t * t))
M0 = 16; FAC = 2 + 1 / M0 + 4 / M0
def run(nS, mode):
    q, X = corpus(nS, mode)
    names = ['allN', 'oracle', 'selfbound', 'capgrid', 'capgrid_raw']
    err = dict.fromkeys(names, 0); smean = dict.fromkeys(names, 0.0); zm_hits = 0
    s_all = squant(EPS / N); s_or = squant(EPS / nS)
    caps = [M0 * 2 ** j for j in range(12)]
    for b0 in range(0, TR, B):
        z = zstats(q, X, min(B, TR - b0))
        zmax = z[:, :nS].max(1)                        # the worst match
        for i in range(z.shape[0]):
            zi = np.sort(z[i]); zs = np.sort(z[i, :nS])
            def surv(s): return int(np.searchsorted(zi, s, side='right'))
            def kept(s): return int(np.searchsorted(zs, s, side='right'))
            # self-bounding: U <- #survivors at s(EPS/U), from U = N, to its fixed point
            U = N
            while True:
                s = squant(EPS / max(U, 1)); U2 = surv(s)
                if U2 >= U: break
                U = U2
            s_sb = s
            def grid(e0):
                for Mj in caps:
                    s = squant(e0 / Mj)
                    if kept(s) <= Mj: return s
            for nm, s in (('allN', s_all), ('oracle', s_or), ('selfbound', s_sb), ('capgrid', grid(EPS / FAC)), ('capgrid_raw', grid(EPS))):
                err[nm] += zmax[i] > s; smean[nm] += s
        zm_hits += int((zmax > s_or).sum())
    print(f'|S|={nS:4d} {mode:5s} ' + '  '.join(f'{nm} {err[nm] / TR:.4f} (c={smean[nm] / TR * math.sqrt(m):.2f})' for nm in names), flush=True)
print(f'n={n} N={N} eps={EPS} trials={TR} M0={M0} cap-grid factor {FAC:.3f}; each entry: Pr[some match wrongly excluded] (mean constant in sd units)')
for nS in (1, 5, 16, 17, 33, 40, 100):
    for mode in ('indep', 'dup'):
        run(nS, mode)
