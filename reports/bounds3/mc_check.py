# Monte Carlo check of the law used throughout: X = sum_j a_j U_j with U_j a coordinate of a uniform unit vector in
# R^{d_j}, drawn as actual normalised Gaussian vectors' first coordinates; tail frequencies against the inversion
# integral at moderate levels (the 1e-19 levels cannot be sampled).
import numpy as np, math
from law import tail
rng = np.random.default_rng(11)
ds = [95, 127, 255, 287]
for w in ([0.1254, 0.167, 0.333, 0.3746], [0.266, 0.411, 0.312, 0.011], [0.9, 0.0, 0.0, 0.1]):
    a = [math.sqrt(x * d) for x, d in zip(w, ds)]
    n = 4_000_000; X = np.zeros(n)
    for aj, d in zip(a, ds):
        if aj == 0: continue
        # U = g_1 / |g|: g_1 ~ N(0,1), |g|^2 = g_1^2 + chi2_{d-1}
        g1 = rng.standard_normal(n); r2 = g1 * g1 + rng.chisquare(d - 1, n)
        X += aj * g1 / np.sqrt(r2)
    line = f"shares {np.round(w, 3)}: sd {X.std():.4f};"
    for x in (2.0, 3.0, 3.5, 4.0):
        emp = (X >= x).mean(); ex = tail(a, ds, x)[0]; se = math.sqrt(ex / n)
        line += f"  P[X>={x}] mc {emp:.3e} exact {ex:.3e} ({(emp-ex)/se:+.1f} se)"
    print(line, flush=True)
