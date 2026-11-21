"""Synthetic checks of the random-energy claims in CLAIMS.md (A1). No real data. Small numpy.

A. The leaf floor (claim A1.2): the exact rows(r) = M P(A > a_r) from the bivariate normal against the closed form
   M Phibar(z_T sqrt f - z_r sqrt(1-f)), and the local exponent d ln rows / d ln M at fixed target count.
B. The tree law in the regime f_j > g_j (hierarchy/theory/model_out.txt, shares 0.6/0.25/0.15: measured 13, predicted 7):
   the annealed level against the realised top-L level.
C. The work exponent max_j (g_j - x^2 f_{j-1}) and the level positions that equalise the terms, for a profile A.
"""
import math
import numpy as np
from scipy.stats import norm
from scipy import integrate, optimize

rng = np.random.default_rng(7)

print("A. leaf floor: exact against closed form (v = 1; f the share explained by the last internal level)")
print("   M        L     f     r     rows exact   rows closed form   ratio   local exponent in M (exact) | 1 - f")
for M in (1e6, 1e9):
    for L in (10, 100):
        for f in (0.3, 0.5, 0.7):
            for r in (0.9, 0.99):
                def rows(Mx):
                    zT = norm.isf(L / Mx)
                    s = math.sqrt(1 - f)
                    # P(A > a, U >= T) with A ~ N(0, f), U | A ~ N(A, 1 - f)
                    def joint(a):
                        g = lambda x: norm.pdf(x / math.sqrt(f)) / math.sqrt(f) * norm.sf((zT - x) / s)
                        return integrate.quad(g, a, a + 12, epsabs=0, epsrel=1e-9, limit=200)[0]
                    tot = norm.sf(zT)
                    a_r = optimize.brentq(lambda a: joint(a) - r * tot, -6, zT + 2, xtol=1e-10)
                    return Mx * norm.sf(a_r / math.sqrt(f)), Mx * norm.sf(zT * math.sqrt(f) - norm.ppf(r) * s)
                ex, cf = rows(M)
                ex2, _ = rows(M * 4)
                print(f"   {M:.0e}  {L:4d}  {f:.1f}  {r:.2f}  {ex:11.0f}  {cf:11.0f}        {ex / cf:5.2f}   {math.log(ex2 / ex) / math.log(4):.3f} | {1 - f:.1f}")

print("\nB. tree law where a level explains more than its share of the entropy (F 28, shares 0.6/0.25/0.15, top 64 of 21,952)")
F, shares, L = 28, (0.6, 0.25, 0.15), 64
D = 3; Kl = F ** D
ranks = [[] for _ in range(D - 1)]; thr = []
per_trial = [[] for _ in range(D - 1)]
for trial in range(200):
    part = [rng.normal(0, math.sqrt(shares[0]), F)]
    for l in range(1, D):
        part.append(np.repeat(part[-1], F) + rng.normal(0, math.sqrt(shares[l]), F ** (l + 1)))
    top = np.argpartition(-part[-1], L)[:L]
    thr.append(np.sort(part[-1])[-L])
    for l in range(D - 1):
        anc = top // F ** (D - 1 - l)
        rk = np.argsort(np.argsort(-part[l]))
        ranks[l].append(rk[anc]); per_trial[l].append(np.quantile(rk[anc], 0.99) + 1)
zT = norm.isf(L / Kl); ze = norm.ppf(0.99)
thr = np.array(thr)
print(f"   annealed level z_T {zT:.3f}; realised top-64 level: mean {thr.mean():.3f}, median {np.median(thr):.3f}, p10 {np.quantile(thr, .1):.3f}, p90 {np.quantile(thr, .9):.3f}")
for l in range(D - 1):
    f = sum(shares[:l + 1]); nodes = F ** (l + 1)
    meas = np.quantile(np.concatenate(ranks[l]), 0.99) + 1
    pred = nodes * norm.sf(zT * math.sqrt(f) - ze * math.sqrt(1 - f))
    pred_real = np.mean([nodes * norm.sf(t * math.sqrt(f) - ze * math.sqrt(1 - f)) for t in thr])
    print(f"   level {l + 1} ({nodes} nodes, f {f:.2f}, g {math.log(nodes) / math.log(Kl):.2f}): pooled 99th pct rank {meas:.0f}; mean of per-trial 99th pct {np.mean(per_trial[l]):.1f}; "
          f"law at the annealed level {pred:.1f}; law at each trial's realised level, mean {pred_real:.1f}")

print("\nC. work exponent of a beam, eta = max_j (g_j - x^2 f_{j-1}); and positions that equalise the terms for A(t) piecewise linear")

def eta(gs, fs, x=1.0):
    g = [0] + list(gs) + [1]; f = [0] + list(fs) + [1]
    terms = [g[j] - x * x * f[j - 1] for j in range(1, len(g))]
    return max(terms), terms

def A_of(profile):
    pts = [(0, 0)] + sorted(profile) + [(1, 1)]
    def A(t):
        for (a, fa), (b, fb) in zip(pts, pts[1:]):
            if a <= t <= b:
                return fa + (fb - fa) * (t - a) / (b - a) if b > a else fb
        return 1.0
    return A

def equalised(A, D, x=1.0):
    # eta with D - 1 internal levels at g_1 = eta, g_j = eta + x^2 A(g_{j-1}); solve g_D = 1
    def last(e):
        g = e
        for _ in range(D - 1):
            g = e + x * x * A(g)
        return g - 1
    e = optimize.brentq(last, 1e-9, 1.0)
    g = [e]
    for _ in range(D - 2):
        g.append(e + x * x * A(g[-1]))
    return e, g

for name, prof in (("homogeneous A(t) = t", [(0.25, 0.25), (0.5, 0.5), (0.75, 0.75)]),
                   ("A(t) = t^2 (weak, convex)", [(t / 20, (t / 20) ** 2) for t in range(1, 20)]),
                   ("A(t) = sqrt t (strong, concave)", [(t / 20, math.sqrt(t / 20)) for t in range(1, 20)])):
    A = A_of(prof)
    sup = max(t / 200 - A(t / 200) for t in range(201))
    line = f"   {name}: sup_t (t - A(t)) = {sup:.3f};"
    for D in (2, 3, 4, 6, 10):
        e, g = equalised(A, D)
        line += f"  D={D}: eta {e:.3f}"
    print(line)
