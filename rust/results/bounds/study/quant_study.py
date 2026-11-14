# tan^2 of the nested randomised code at each depth: uniform halving deltas (as built) against other per-depth
# deltas, on Gaussian blocks (m = 128), the encoder scale chosen per item from LAMS for the sum over rises of log tan^2.
import numpy as np, itertools, sys
rng = np.random.default_rng(1)
m, nv = 128, 3000
Y = rng.standard_normal((nv, m))
LAMS = [2.0, 2.3, 2.6, 3.0, 3.5, 4.2]
def run(deltas, rises, lams=LAMS):
    fin = max(rises)
    rms = np.sqrt((Y * Y).mean(1, keepdims=True))
    best = np.full(nv, np.inf); out = np.zeros((nv, len(rises)))
    for lam in lams:
        v = Y / (lam * rms); c = np.zeros_like(v); vals = []
        for d in range(fin):
            t = np.where(v - c >= 0, 1.0, -1.0); c = c + t * deltas[d]; vals.append(c.copy())
        t2 = []
        for r in rises:
            cv = vals[r - 1]; dot = (cv * Y).sum(1); cn = (cv * cv).sum(1); yn = (Y * Y).sum(1)
            t2.append(np.maximum(cn * yn - dot * dot, 1e-300) / np.maximum(dot * dot, 1e-300))
        t2 = np.array(t2).T; obj = np.log(t2).sum(1)
        bt = obj < best; best[bt] = obj[bt]; out[bt] = t2[bt]
    return out.mean(0)
uni = [0.5 ** (d + 1) for d in range(8)]
for rises in ([1, 2], [2, 3], [2, 3, 6], [1, 2, 3, 4], [2, 4], [3, 4, 7]):
    u = run(uni, rises)
    fin = max(rises)
    # coordinate search on the ratios delta_{d+1}/delta_d
    rat = [0.5] * (fin - 1); bestv = np.log(u).sum(); bests = u
    for it in range(3):
        for k in range(fin - 1):
            for x in (0.40, 0.45, 0.5, 0.55, 0.6, 0.65):
                r2 = list(rat); r2[k] = x
                dl = [1.0];
                for q in r2: dl.append(dl[-1] * q)
                v = run(dl, rises)
                if np.log(v).sum() < bestv - 1e-9: bestv, rat, bests = np.log(v).sum(), r2, v
    print(f"rises {rises}: uniform tan2 {np.round(u,4)}  best ratios {rat} tan2 {np.round(bests,4)}  tan ratio {np.round(np.sqrt(bests/u),3)}", flush=True)
