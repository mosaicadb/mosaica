# Two items under ONE shared Haar rotation: are their normalised errors independent?
# Only the images of span{x1, x2, q} matter, so draw a uniform orthonormal 3-frame in R^n per trial.
import math, numpy as np
rng = np.random.default_rng(3); n = 100; m = n - 1; TR = 400000; B = 20000; t = 0.5
print(f'n={n}, 1-bit code, {TR} rotations, both items at score {t}; z = error / sd (unit variance by the law)')
print('cos(x1,x2)  corr(z1,z2)  sd(z1)   P[z1>2.5]   P[z1>2.5 & z2>2.5]   product    ratio')
for rho in (0.0, 0.3, 0.6, 0.9, 0.99, 0.999):
    a = t / (1 + rho); b = math.sqrt(1 - 2 * a * a * (1 + rho))
    # orthonormal basis e1,e2,e3: x1 = e1, x2 = rho e1 + sqrt(1-rho^2) e2, q = a(x1+x2) + b e3
    x1 = np.array([1.0, 0, 0]); x2 = np.array([rho, math.sqrt(1 - rho * rho), 0]); q = a * (x1 + x2) + np.array([0, 0, b])
    zs = []
    for b0 in range(0, TR, B):
        G = rng.standard_normal((B, n, 3)); F, _ = np.linalg.qr(G)        # uniform orthonormal frames
        out = []
        Pq = F @ q
        for x in (x1, x2):
            Z = F @ x; code = np.sign(Z) / math.sqrt(n); cos = (code * Z).sum(1)
            est = (code * Pq).sum(1) / cos; tan = np.sqrt(1 - cos * cos) / cos
            out.append((est - t) / (tan * math.sqrt(1 - t * t)) * math.sqrt(m))
        zs.append(np.stack(out, 1))
    z = np.concatenate(zs); p1 = (z[:, 0] > 2.5).mean(); p12 = ((z[:, 0] > 2.5) & (z[:, 1] > 2.5)).mean()
    print(f'{rho:9.3f}  {np.corrcoef(z.T)[0, 1]:10.4f}  {z[:, 0].std():6.4f}  {p1:10.2e}  {p12:18.2e}  {p1 * p1:9.2e}  {p12 / (p1 * p1):7.1f}')
