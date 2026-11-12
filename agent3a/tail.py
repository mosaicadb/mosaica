# Exact tail of one coordinate of a uniform unit vector in R^m against the sub-Gaussian bound exp(-m s^2/2).
# s(m, delta) solves P(|u_1| >= s) = delta; the single-block randomised half-width is s * A * |Delta| (deterministic: 1 * A * |Delta|).
import numpy as np, math
def tail(m, s, npts=400_001):
    p = (m - 3) / 2
    def integ(a, b):
        x = np.linspace(a, b, npts); f = np.exp(p * np.log1p(-np.minimum(x * x, 1 - 1e-300))) if p != 0 else np.ones_like(x)
        return float(((f[1:] + f[:-1]) * 0.5).sum() * (x[1] - x[0]))
    return integ(s, 1.0) / integ(0.0, 1.0)
def solve(m, delta):
    lo, hi = 0.0, 1.0
    for _ in range(40):
        mid = 0.5 * (lo + hi)
        if tail(m, mid, 40_001) > delta: lo = mid
        else: hi = mid
    return hi
print("block dims n_b | c | sub-Gaussian c/sqrt(m) | exact s | exact/sub-Gaussian | simple bound sqrt(1-delta^(2/m))")
for c in (8.0, 10.0):
    delta = 2 * math.exp(-c * c / 2)
    for nb in (24, 48, 96, 128, 256, 384, 768, 2048, 5000):
        m = nb - 1; s = solve(m, delta); sg = c / math.sqrt(m)
        print(f"  {nb:5d} | {c:4.0f} | {sg:6.3f}{' (>1: no gain)' if sg >= 1 else '':15s} | {s:.3f} | {s / min(sg, 1):.3f} | {math.sqrt(1 - delta ** (2 / m)):.3f}")
# check the simple bound P(|u_1| >= s) <= 2 (1-s^2)^(m/2) numerically for s <= 1/sqrt 2
worst = max(tail(m, s, 20_001) / (2 * (1 - s * s) ** (m / 2)) for m in (3, 5, 23, 47, 127, 767) for s in np.linspace(0.01, 0.70, 24))
print("max over a grid of exact tail / [2 (1-s^2)^(m/2)], s <= 0.70:", round(worst, 4), "(<= 1 confirms the simple bound on the grid)")
