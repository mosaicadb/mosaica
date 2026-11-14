# Is F(v) = H(sqrt(v d))^2 concave on the cone v >= 0? Direct test with the exact quantile: second differences along
# random segments of the simplex (including the faces), and tangent planes against exact values.
import numpy as np, math
from ctable import quantiles
ds = [95, 127, 255, 287]; LN = math.log(1e-9 / 1e9); lnd = LN - math.log(4)
rng = np.random.default_rng(5)
def F(W): return quantiles(np.atleast_2d(W), ds, [lnd])[:, 0] ** 2
worst = (-1, None)
for trial in range(400):
    al = rng.choice([0.3, 1.0, 3.0])
    p = rng.dirichlet([al] * 4); q = rng.dirichlet([al] * 4)
    if rng.random() < 0.3: p[rng.integers(4)] = 0; p /= p.sum()
    if rng.random() < 0.3: q[rng.integers(4)] = 0; q /= q.sum()
    t = np.linspace(0, 1, 9)
    pts = p[None] * (1 - t[:, None]) + q[None] * t[:, None]
    f = F(pts)
    sd2 = f[:-2] + f[2:] - 2 * f[1:-1]
    if sd2.max() > worst[0]: worst = (sd2.max(), (p, q, sd2))
print("largest second difference along 400 random segments:", worst[0], "(positive would refute concavity)")
print("   at p", np.round(worst[1][0], 4), "q", np.round(worst[1][1], 4), "second differences", np.round(worst[1][2], 5))
# tangent plane at the level-1 mean share against exact values at many points
def grad_F(w0, eps=1e-4):
    pts = [w0] + [w0 + s * eps * np.eye(4)[j] for j in range(4) for s in (1, -1)]
    q = F(np.array(pts)); return q[0], np.array([(q[1 + 2 * j] - q[2 + 2 * j]) / (2 * eps) for j in range(4)])
for w0 in ([0.266, 0.411, 0.312, 0.011], [0.097, 0.158, 0.632, 0.113], [0.6, 0.2, 0.15, 0.05]):
    w0 = np.array(w0) / sum(w0); F0, gk = grad_F(w0)
    P = np.vstack([rng.dirichlet([a_] * 4, 3000) for a_ in (0.3, 1.0, 4.0)] + [np.eye(4)])
    ex = F(P); pl = P @ gk
    i = np.argmax(ex - pl)
    print(f"anchor {np.round(w0,3)}: max over 9004 points of F - plane = {np.max(ex - pl):+.6f} at {np.round(P[i], 4)} (F {ex[i]:.4f}); min {np.min(ex - pl):+.4f}")
    # near the anchor in the direction of small shares
    for eps in (1e-3, 1e-2):
        for j in range(4):
            wv = w0.copy(); wv[j] = max(wv[j] - eps, 0); wv /= wv.sum()
            print(f"     lower share {j} by {eps}: F - plane = {F(wv)[0] - wv @ gk:+.6f}", end=";")
        print()
