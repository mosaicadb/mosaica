"""A4 (second round), synthetic only: why a level with the right spread still loses neighbours, and what psi_eff does.

Within a cluster s = a + e (a the level's estimate, e its error), f = Var(a)/Var(s), tau = sd(e), zeta = (t - mu)/sigma.
A. Normal scores: the keep probability of a row at s = t under a + psi tau >= t against Phi((psi - zeta sqrt(1-f))/sqrt f),
   and of all rows with s >= t; psi_eff = psi0 sqrt f + zeta sqrt(1-f) against Phi(psi0).
B. Scores with an exponential upper tail (a Laplace, e normal and independent): E[e | s = t] against the normal model's
   zeta sqrt(1-f) tau and against tau^2 times the local decay rate of the scores' density; keep probabilities.
C. The quantities estimated from m sampled rows: the keep probability at s = t under the sampled psi_eff, against
   Phi(psi0 / sqrt(1 + (zeta^2 + psi0^2/2 + (1-f)/f)/m)).
D. Which rule for which objective: rows passed at equal recall of the rows above t, one psi for all clusters against
   psi_eff, over 30 clusters at zeta from 3 to 7.

usage: run-limited.sh 6 1 2 python3 a4_select.py
"""
import math
import numpy as np
from scipy.stats import norm
from scipy import integrate, optimize

rng = np.random.default_rng(12)

print("A. normal scores (2e6 rows a cell): share kept among rows with s within 0.05 sigma above t | formula at s = t ; among all rows with s >= t")
for f in (0.8, 0.5, 0.3):
    for zeta in (3.0, 4.0):
        n = 2_000_000
        a = rng.normal(0, math.sqrt(f), n); e = rng.normal(0, math.sqrt(1 - f), n); s = a + e
        tau = math.sqrt(1 - f); t = zeta
        near = (s >= t) & (s < t + 0.05); above = s >= t
        line = f"   f {f} zeta {zeta}: rows above t {int(above.sum())};"
        for psi in (4.0, 5.0):
            keep = a + psi * tau >= t
            line += f"  psi {psi}: {keep[near].mean():.3f} | {norm.cdf((psi - zeta * math.sqrt(1 - f)) / math.sqrt(f)):.3f} ; {keep[above].mean():.3f}"
        for psi0 in (2.0,):
            pe = psi0 * math.sqrt(f) + zeta * math.sqrt(1 - f)
            keep = a + pe * tau >= t
            line += f"  psi_eff(psi0 {psi0}) = {pe:.2f}: {keep[near].mean():.3f} | {norm.cdf(psi0):.3f} ; {keep[above].mean():.3f}"
        print(line)

print("\nB. exponential upper tail: a Laplace (so the standardised scores decay at rate sqrt(2/f) a spread), e normal, independent")
for f in (0.8, 0.5, 0.3):
    n = 6_000_000
    b = math.sqrt(f / 2); tau = math.sqrt(1 - f)
    a = rng.laplace(0, b, n); e = rng.normal(0, tau, n); s = a + e
    lam = 1 / b
    for zeta in (4.0, 5.0):
        t = zeta
        near = (s >= t) & (s < t + 0.1)
        if near.sum() < 200:
            continue
        me, sde = e[near].mean(), e[near].std()
        line = (f"   f {f} rate {lam:.2f} zeta {zeta} ({int(near.sum())} rows at t): E[e|s=t]/tau measured {me / tau:.2f}, tau x decay rate {tau * lam:.2f}, normal model {zeta * math.sqrt(1 - f):.2f};"
                f" sd[e|s=t]/tau measured {sde / tau:.2f}, normal model {math.sqrt(f):.2f}; kept at s = t:")
        for psi in (4.0,):
            line += f" psi {psi}: {(a[near] + psi * tau >= t).mean():.3f} (decay-rate form {norm.cdf(psi - tau * lam):.3f}, normal form {norm.cdf((psi - zeta * math.sqrt(1 - f)) / math.sqrt(f)):.3f});"
        pe = 2.0 * math.sqrt(f) + zeta * math.sqrt(1 - f)
        pt = 2.0 + tau * lam
        line += f" normal psi_eff {pe:.2f}: {(a[near] + pe * tau >= t).mean():.3f}; decay-rate psi_eff {pt:.2f}: {(a[near] + pt * tau >= t).mean():.3f} (aim {norm.cdf(2.0):.3f})"
        print(line)

print("\nC. psi_eff from m sampled rows (normal cluster; 4,000 samples a cell): keep probability at s = t, measured | formula ; and the plain psi with the sampled tau")
for f in (0.8, 0.5):
    for zeta in (4.0, 5.0):
        for m in (32, 64, 128):
            line = f"   f {f} zeta {zeta} m {m}:"
            for psi0 in (2.0, 3.0):
                got = []
                A = rng.normal(0, math.sqrt(f), (4000, m)); E = rng.normal(0, math.sqrt(1 - f), (4000, m)); S = A + E
                mu, sd = S.mean(1), S.std(1, ddof=1)
                te = E.std(1, ddof=1)
                # the regression of e on s, from the sample: slope and residual spread
                cs = ((E - E.mean(1, keepdims=True)) * (S - mu[:, None])).sum(1) / (m - 1)
                beta = cs / sd ** 2
                res = np.sqrt(np.maximum(te ** 2 - beta ** 2 * sd ** 2, 1e-12))
                thr = psi0 * res + beta * (zeta - mu) + E.mean(1)               # keep when e <= thr, in the sampled units
                # the row at s = t has e ~ N((1-f) zeta, f(1-f))
                p = norm.cdf((thr - (1 - f) * zeta) / math.sqrt(f * (1 - f)))
                nu2 = (zeta ** 2 + psi0 ** 2 / 2 + (1 - f) / f) / m
                line += f"  psi0 {psi0}: {p.mean():.3f} | {norm.cdf(psi0 / math.sqrt(1 + nu2)):.3f} (exact stats {norm.cdf(psi0):.4f})"
            for psi in (5.0,):
                p = norm.cdf((psi * te - (1 - f) * zeta) / math.sqrt(f * (1 - f)))
                line += f"  ; plain psi {psi}: {p.mean():.3f} (exact tau {norm.cdf((psi - zeta * math.sqrt(1 - f)) / math.sqrt(f)):.3f})"
            print(line)

print("\nD. rows passed at equal recall of the rows above t: one psi against psi_eff (30 clusters of 2,000 rows, zeta 3 to 7, normal model)")


def cluster(zeta, f, psi):
    """(expected rows above t that are kept, rows passed), per row of the cluster."""
    tau = math.sqrt(1 - f); cut = zeta - psi * tau
    g = lambda x: norm.pdf(x / math.sqrt(f)) / math.sqrt(f) * norm.sf((zeta - x) / tau)
    kept = integrate.quad(g, cut, cut + 14, epsabs=0, epsrel=1e-8, limit=200)[0]
    return kept, norm.sf(cut / math.sqrt(f))


zs = np.linspace(3, 7, 30)
for f in (0.8, 0.5, 0.3):
    tot = sum(norm.sf(z) for z in zs)
    def curve(rule, par):
        k = r = 0.0
        for z in zs:
            psi = par if rule == "plain" else par * math.sqrt(f) + z * math.sqrt(1 - f)
            kk, rr = cluster(z, f, psi); k += kk; r += rr
        return k / tot, 2000 * r
    line = f"   f {f}:"
    for target in (0.9, 0.95, 0.99):
        out = {}
        for rule, lo, hi in (("plain", 0.0, 14.0), ("eff", -4.0, 8.0)):
            par = optimize.brentq(lambda p: curve(rule, p)[0] - target, lo, hi, xtol=1e-6)
            out[rule] = (par, curve(rule, par)[1])
        line += f"  recall {target}: plain psi {out['plain'][0]:.2f} rows {out['plain'][1]:.0f}, psi0 {out['eff'][0]:.2f} rows {out['eff'][1]:.0f} ({out['eff'][1] / out['plain'][1]:.2f}x);"
    print(line)
    # what one plain psi does to the keep probability across the clusters
    psi = optimize.brentq(lambda p: curve("plain", p)[0] - 0.95, 0.0, 14.0, xtol=1e-6)
    print(f"        at recall 0.95 the plain psi {psi:.2f} keeps a row at s = t with probability " + ", ".join(f"{norm.cdf((psi - z * math.sqrt(1 - f)) / math.sqrt(f)):.2f} (zeta {z:.0f})" for z in (3, 4, 5, 6)))
