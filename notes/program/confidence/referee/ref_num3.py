"""Check of the refined step (c): Pr[G_1(c) and angle(c,z_2)<=theta] by simulation against the two bounds, small d."""
import numpy as np
from scipy.special import betainc, betaincinv, gammaln
from scipy.integrate import quad
def T(d, x):
    x = np.asarray(x, float); ax = np.clip(np.abs(x), 0, 1)
    t = 0.5 * betainc((d - 1) / 2, 0.5, 1 - ax * ax); return np.where(x >= 0, t, 1 - t)
def qd(d, delta): return float(np.sqrt(1 - betaincinv((d - 1) / 2, 0.5, 2 * delta)))
rng = np.random.default_rng(3)
def unit(a): return a / np.linalg.norm(a, axis=-1, keepdims=True)
for d, delta, th in ((12, 0.01, 0.9), (12, 0.001, 0.9), (12, 0.001, 1.2), (24, 0.001, 1.0), (6, 0.01, 0.8)):
    n = d + 1; q = qd(d, delta); cap = lambda dd, t: 0.5 * betainc(dd / 2, 0.5, np.sin(t) ** 2)
    tot = 0.0; tot2 = 0.0; cnt = 0; g1 = 0
    for _ in range(40):
        b = 100000
        Dp = unit(rng.standard_normal((b, n)))
        w = rng.standard_normal((b, n)); w -= (w * Dp).sum(1, keepdims=True) * Dp; z1 = unit(w)
        c = unit(rng.standard_normal((b, n)))            # uniform codeword; keep those within theta of z1
        cosf = (c * z1).sum(1); tau = -(c * Dp).sum(1)
        G1 = (cosf >= np.cos(th)) & (tau > q * np.sqrt(1 - cosf ** 2))
        val = np.where(G1, T(d, np.cos(th) / np.sqrt(np.clip(1 - tau ** 2, 1e-300, 1))), 0.0)   # exact Pr[z2 within theta of c | c, Delta']
        tot += val.sum(); tot2 += (val ** 2).sum(); cnt += b; g1 += G1.sum()
    est = tot / cnt; se = np.sqrt(tot2 / cnt - est ** 2) / np.sqrt(cnt)
    lk = gammaln((d + 1) / 2) - gammaln(d / 2) - 0.5 * np.log(np.pi)
    integ, _ = quad(lambda ph: np.exp(lk) * np.sin(ph) ** (d - 1) * float(T(d, np.cos(th) / np.sqrt(1 - q * q * np.sin(ph) ** 2))), 0, th)
    print(f"d={d} delta={delta} theta={th}: Pr[G1]={g1/cnt:.3e} (Omega*delta={cap(d,th)*delta:.3e});  Pr[G1 and H2]={est:.3e} +- {se:.1e};  refined bound={delta*integ:.3e};  author's bound={cap(d,th)*delta*cap(d-1,th):.3e}")
