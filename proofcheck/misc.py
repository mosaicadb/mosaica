import numpy as np, math
from scipy import stats
rng = np.random.default_rng(1)
def Eg(k): return math.sqrt(2) * math.exp(math.lgamma((k + 1) / 2) - math.lgamma(k / 2))
def lvol(k): return k / 2 * math.log(math.pi) - math.lgamma(k / 2 + 1)
def ecsq(sig, w, off):
    j = np.arange(-3000, 3001); e = (j + off) * w; p = np.diff(stats.norm.cdf(np.append(e, e[-1] + w) / sig)); p = p[p > 0]
    return -(p * np.log(p)).sum()
# (a) non-commuting Q and Lambda: boxes in Lambda's eigenbasis, exact E h, against the floor with eigenvalues of Q^1/2 Lam Q^1/2
worst = 9
for t in range(3000):
    n = rng.integers(2, 6); A = rng.standard_normal((n, n)) * np.exp(rng.normal(0, 1, n)); Q = A @ A.T
    V = np.linalg.qr(rng.standard_normal((n, n)))[0]; sig = np.exp(rng.normal(0, 1.2, n)); Lam = (V * sig ** 2) @ V.T
    w = sig * np.exp(rng.uniform(-4, 2, n)); H = sum(ecsq(s, ww, o) for s, ww, o in zip(sig, w, rng.uniform(0, 1, n)))
    ach = sum(math.sqrt(2 / math.pi) * math.sqrt(V[:, i] @ Q @ V[:, i]) * w[i] / 2 for i in range(n))
    ev, U = np.linalg.eigh(Q); Qh = (U * np.sqrt(ev)) @ U.T; gam = np.sort(np.linalg.eigvalsh(Qh @ Lam @ Qh))[::-1]
    b = max(Eg(k) * math.exp(-lvol(k) / k + 0.5 * math.log(2 * math.pi * math.e) + 0.5 * np.log(gam[:k]).sum() / k - H / k) for k in range(1, n + 1))
    worst = min(worst, ach / b)
print("(a) non-commuting Q, Lambda; 3000 random instances: worst achieved/floor = %.5f (>=1 required)" % worst)
# (b) sub-bit sketch law: tan^2 phi vs pi n/(2k') - 1
print("(b) sub-bit law: k', n_b, median tan^2, mean tan^2, 99.9%% quantile, predicted pi n/(2k')-1")
for nb, kk in ((256, 1), (256, 2), (256, 4), (256, 16), (256, 64), (256, 256), (1024, 64)):
    z = rng.standard_normal((20000, nb)); c = np.abs(z[:, :kk]).sum(1) / (math.sqrt(kk) * np.linalg.norm(z, axis=1)); t2 = 1 / c ** 2 - 1
    print("    %4d %5d  %10.2f %12.2f %12.2f %10.2f" % (kk, nb, np.median(t2), t2.mean(), np.quantile(t2, .999), math.pi * nb / (2 * kk) - 1))
# (c) T4: ||P_W u|| against sqrt(k/m) + c/sqrt(m-2); empirical exceedance and exact Beta tail
print("(c) T4: P(||P_W u|| > sqrt(k/m)+c/sqrt(m-2)) exact (Beta) vs exp(-c^2/2)")
for m, k in ((30, 1), (30, 5), (100, 10), (767, 20), (767, 200), (10, 3), (4, 1), (3, 1)):
    for c in (1., 2., 3., 5.):
        thr = math.sqrt(k / m) + c / math.sqrt(m - 2); p = stats.beta.sf(min(thr * thr, 1), k / 2, (m - k) / 2) if thr < 1 else 0.0
        flag = "" if p <= math.exp(-c * c / 2) else "  VIOLATION"
        print("    m=%4d k=%3d c=%.0f: exact %.3e  bound %.3e%s" % (m, k, c, p, math.exp(-c * c / 2), flag))
