"""Referee 2, item 6: does a filter change the law of a row's error given its score?
One cluster, sigma = 1. Two row laws: (N) (a, e) jointly normal, f = 0.82, rho_es = 0.56; (C) the author's Model C
at the yahoo setting. A second pair of variables stands for other directions of the row: g_a, correlated with the
part of the midpoint the score does not explain, and g_e, correlated with the part of the error it does not explain.
Filters: random quarter; bottom / top quarter by score; a soft filter on score; a quarter by midpoint; a region
along g_a; a region along g_e; a region along an unrelated direction.
Printed: tau_vis/tau_all, sigma_vis/sigma_all, mean of e over visible rows (in tau), and in three score bins that
hold visible rows the mean and sd of e/tau among visible rows against all rows. Synthetic."""
import math
import numpy as np
rng = np.random.default_rng(11)
N = 6_000_000

def rows(law):
    if law == "N":
        f, rho = 0.82, 0.56; tau = math.sqrt(1 - f)
        s = rng.normal(size=N); e = rho * tau * s + math.sqrt(1 - rho * rho) * tau * rng.normal(size=N)
    else:
        f, kap, b = 0.82, 0.39, 1 / 2.2; vG = 1 - b * b
        omf = ((1 - f) - kap * kap * b * b) / vG
        R = rng.exponential(b, N) - b
        aG = rng.normal(0, math.sqrt((1 - omf) * vG), N); eG = rng.normal(0, math.sqrt(omf * vG), N)
        e = kap * R + eG; s = (1 - kap) * R + aG + e
    return s, e

for law in ("N", "C"):
    s, e = rows(law); a = s - e
    tau, sig = e.std(), s.std()
    # residual of e on s, and two "other direction" variables
    be = np.cov(e, s)[0, 1] / s.var(); res_e = e - be * s
    g_a = -res_e / res_e.std() * 0.7 + rng.normal(size=N) * math.sqrt(1 - 0.49)   # a's part beyond s is -res_e
    g_e = res_e / res_e.std() * 0.7 + rng.normal(size=N) * math.sqrt(1 - 0.49)
    g_0 = rng.normal(size=N)
    q = lambda v, p: np.quantile(v[::50], p)
    filt = {
        "none": np.ones(N, bool),
        "random quarter": rng.random(N) < 0.25,
        "bottom quarter by score": s <= q(s, 0.25),
        "top quarter by score": s >= q(s, 0.75),
        "soft on score (logistic)": rng.random(N) < 1 / (1 + np.exp(-2 * s / sig)),
        "top quarter by midpoint": a >= q(a, 0.75),
        "region along a coded direction": g_a >= q(g_a, 0.75),
        "region along a left-out direction": g_e >= q(g_e, 0.75),
        "region along an unrelated direction": g_0 >= q(g_0, 0.75),
    }
    print(f"\nrow law {law}: tau {tau:.3f}, sigma {sig:.3f}, rho_es {np.corrcoef(e, s)[0, 1]:.2f}")
    for name, V in filt.items():
        sv, ev = s[V], e[V]
        line = f"  {name:36s} tau_vis/tau {ev.std() / tau:.2f}  sigma_vis/sigma {sv.std() / sig:.2f}  mean e {ev.mean() / tau:+.2f} tau |"
        for p in (0.1, 0.5, 0.9):                       # three heights inside the visible rows' range
            c = np.quantile(sv[::20], p)
            mv = np.abs(sv - c) < 0.03; ma = np.abs(s - c) < 0.03
            line += f"  x {c / sig:+.2f}: e/tau visible {ev[mv].mean() / tau:+.2f} +- {ev[mv].std() / tau:.2f}, all {e[ma].mean() / tau:+.2f} +- {e[ma].std() / tau:.2f};"
        print(line)
