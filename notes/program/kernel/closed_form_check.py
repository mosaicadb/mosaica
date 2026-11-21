"""The closed forms of sup_set and inf_set against the bound evaluated on a grid of keys (model: range_model.py)."""
import math
import numpy as np
import range_model as rm

rng = np.random.default_rng(5)
bad = 0
loose = 0.0
N = 20000
for _ in range(N):
    p = rng.standard_normal() * rng.choice([1.0, 1e-3, 0.0])
    r = abs(rng.standard_normal()) * rng.choice([1.0, 1e-3, 1e-9])
    rr = 1.0 + rng.uniform(0, 5e-4)
    qn = math.hypot(p, r)
    t = rng.uniform(-1.2, 1.2) * qn * rr
    a = np.concatenate([np.linspace(-rr, rr, 4001), [rr * math.cos(math.atan2(r, p))], [-rr * math.cos(math.atan2(r, p))]])
    w = np.sqrt(np.maximum(rr * rr - a * a, 0.0))
    U = p * a + r * w
    L = p * a - r * w
    for f, vals, inside in ((rm.sup_set, U, U >= t), (rm.inf_set, L, L < t)):
        iv = f(p, r, rr, t)
        tol = 1e-12
        if iv is None:
            m = (vals - t) if f is rm.sup_set else (t - vals)
            if inside.any() and m[inside].max() > 1e-12:
                bad += 1
            continue
        out_of = inside & ((a < iv[0] - tol) | (a > iv[1] + tol))
        if out_of.any():
            bad += 1
        # tightness: grid points well inside the returned interval must satisfy the inequality
        deep = (a > iv[0] + 1e-9) & (a < iv[1] - 1e-9)
        if (deep & ~inside).any():
            m = np.abs(vals - t)[deep & ~inside].max()
            loose = max(loose, m)
print(f"{N} random (p, r, R, t): grid points satisfying the inequality but outside the closed-form interval: {bad}; largest |bound - t| at a grid point inside the interval that fails the inequality: {loose:.2e}")
