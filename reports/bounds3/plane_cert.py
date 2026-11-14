# Certificates for per-block constants (tangent planes of F(v) = H(sqrt(v d))^2): for an anchor share vector w0,
# kappa = grad F(w0), and the claim certified is   H(a)^2 <= (1 + e)^2 sum_j kappa_j a_j^2 / d_j   for every a >= 0,
# i.e. P[sum_j a_j U_j >= (1 + e) c0] <= delta whenever sum_j a_j^2 kappa_j / (d_j c0^2) = 1.
import numpy as np, math, sys, time
from ctable import quantiles
from cert2 import certify
ds = [95, 127, 255, 287]; lnd = math.log(1e-9 / 1e9) - math.log(4)
def grad_F(w0, eps=1e-4):
    pts = [w0] + [w0 + s * eps * np.eye(4)[j] for j in range(4) for s in (1, -1)]
    q = quantiles(np.array(pts), ds, [lnd])[:, 0] ** 2
    return q[0], np.array([(q[1 + 2 * j] - q[2 + 2 * j]) / (2 * eps) for j in range(4)])
anchors = {"level 1 mean shares": [0.266, 0.411, 0.312, 0.011], "level 2 mean shares": [0.273, 0.246, 0.445, 0.036], "level 3 mean shares": [0.097, 0.158, 0.632, 0.113]}
which = sys.argv[1:] or list(anchors)
for name in which:
    w0 = np.array(anchors[name]); w0 = w0 / w0.sum()
    F0, kap = grad_F(w0)
    c0 = 8.6
    X = math.ceil(c0 * 1.00105 * 1000) / 1000
    t0 = time.time()
    for attempt in range(4):
        ok, mx, K = certify(ds, lnd, round(X, 3), Kmax=4096, scale=list(c0 * c0 / kap), verbose=False)
        e = X / c0 - 1
        print(f"{name}: w0 {np.round(w0, 4)} c(w0) {math.sqrt(F0):.4f} kappa {np.round(kap, 4)} (sqrt {np.round(np.sqrt(kap), 4)}); "
              f"inflation 1 + e = {1 + e:.5f}: certified {ok} (level K {K}, max ln tail {mx:.5f} vs {lnd:.5f}) [{time.time()-t0:.0f}s]", flush=True)
        if ok: break
        X += 0.002
    print(f"   certified per-block constants sqrt(kappa_j) (1 + e): {np.round(np.sqrt(kap) * (1 + e), 4)}; at the anchor the half-width is {math.sqrt(F0) * (1 + e):.4f} sd (worst-case constant 8.704)", flush=True)
