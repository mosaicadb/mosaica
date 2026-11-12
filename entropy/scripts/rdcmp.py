# Gaussian rate (reverse water-filling on held-out per-dimension variances, MSE in residual space) of alternative cluster/group configurations at the distortion of the default one.
import numpy as np, sys, glob, math
BASE = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/entropy/logs/"
def curve(f):
    s = np.load(f); v = np.maximum(s["varfull"].mean(1), 1e-14); w = s["cnt"].sum(1) / s["cnt"].sum(); return v, w, int(s["K"]), int(s["G"]), int(s["n"])
def R(v, w, th): return float((w[:, None] * 0.5 * np.log2(np.maximum(v / th, 1))).sum())
def D(v, w, th): return float((w[:, None] * np.minimum(v, th)).sum())
def solve(f, lo, hi, target):                          # f decreasing in theta for R, increasing for D
    for _ in range(200):
        m = math.sqrt(lo * hi)
        if f(m) > target: lo = m
        else: hi = m
    return m
for d in sys.argv[1:]:
    v0, w0, K0, G0, n = curve(BASE + f"spec_{d}.npz"); print(f"{d}: default K={K0} G={G0}; total residual energy {D(v0, w0, 1e9):.4f}")
    for f in sorted(glob.glob(BASE + f"spec_{d}_K*_G*.npz")):
        v, w, K, G, _ = curve(f); row = []
        for mult in (1 / 3, 1, 2, 4):
            th0 = solve(lambda t: R(v0, w0, t), 1e-30, v0.max(), mult * n); d0 = D(v0, w0, th0)
            if D(v, w, 1e9) <= d0: r = 0.0
            else: th = solve(lambda t: -D(v, w, t), 1e-30, v.max(), -d0); r = R(v, w, th)
            row.append(mult * n - r - math.log2(K / K0))
        print(f"   K={K:5d} G={G:3d} energy {D(v, w, 1e9):.4f} | bits saved at n/3, n, 2n, 4n bits (pointer log2(K/K0) = {math.log2(K / K0):.0f} charged): " + " ".join(f"{x:8.1f}" for x in row))
