# A direct check of the inner inequality of Theorem F1 in one dimension (CLAIMS.md, section 3, step 4):
#   for a partition of the line into intervals with masses P_c under N(0,1),  C_bar = sum_c P_c C(nu_c) >= e^(-H(c)),
# where nu_c is N(0,1) restricted to the cell and C(.) the Gaussian correlation of its quantile function, and
#   E h = E|g| sum_c P_c (length_c / 2) >= C_bar   (the coupling step).
# The cell laws are discretised on 600 sub-cells each (a piecewise-constant law, for which C is a closed form).
# Run:  nice -n 10 taskset -c 6 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 check_chain1d.py
import math
import numpy as np
from scipy import stats

exec(open("check_lemma1d.py").read().split("worst = (9e9, None)")[0])     # stats_pc and its helpers
rng = np.random.default_rng(7)
w1 = 9e9; w2 = 9e9; rows = []
for t in range(160):
    kind = t % 3
    if kind == 0:
        w = math.exp(rng.uniform(-3, 1.5)); e = (np.arange(-int(9 / w) - 2, int(9 / w) + 3) + rng.uniform(0, 1)) * w
    elif kind == 1:
        e = np.sort(np.concatenate([[-9.0, 9.0], rng.normal(0, rng.uniform(0.3, 3), int(rng.integers(1, 25)))]))
    else:
        L = rng.uniform(0.5, 4); q = rng.uniform(1.1, 2); out = [L]
        while out[-1] < 9: out.append(out[-1] * q)
        e = np.array(sorted([-x for x in out] + out))
    e = e[(e >= -9.5) & (e <= 9.5)]
    P = np.diff(stats.norm.cdf(e)); keep = P > 1e-13
    Cbar = 0.0; Eh = 0.0; H = 0.0; Ptot = P[keep].sum()
    for a, b, p in zip(e[:-1][keep], e[1:][keep], P[keep]):
        sub = np.linspace(a, b, 601); ps = np.diff(stats.norm.cdf(sub)); ps /= ps.sum()
        h, C, m2, KL = stats_pc(sub, ps)
        p /= Ptot; Cbar += p * C; Eh += p * math.sqrt(2 / math.pi) * (b - a) / 2; H -= p * math.log(p)
    w1 = min(w1, Cbar * math.exp(H)); w2 = min(w2, Eh / Cbar)
    if t < 3: rows.append((kind, H, Cbar, math.exp(-H), Eh))
for r in rows: print("  example (kind %d): H = %.3f nats, C_bar = %.4f, e^-H = %.4f, E h = %.4f" % r)
print("160 interval partitions: min C_bar * e^H = %.4f (>= 1 required by F1's step 4); min E h / C_bar = %.4f (>= 1, the coupling step)" % (w1, w2))
