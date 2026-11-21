# Referee check of C3 (b): an independent implementation of width (0.1), ideal curves, group 12, c = 8, at 32 and 56 B.
# Exact water-filling by closed form (no table), random cuts on a grid of 16 with J up to 6, and layouts with a head in D.
# Every figure is again a local optimum: it can only show that the committed search missed something, not that it did not.
import sys, math, numpy as np
sys.dont_write_bytecode = True
sys.path.insert(0, "..")
from scipy.optimize import minimize
from design_lib import spectra
a, _ = spectra()[12]; n = len(a); F = float(a.sum()); C = 8.0
def W(seg_cum, seg, H):            # exact water-filling for a descending segment; seg_cum = cumulative sum of ln
    m = len(seg)
    if H <= 0: return float(seg.sum())
    k = np.arange(1, m + 1)
    lam = np.exp((seg_cum - H) / k)          # level if exactly the top k are above water
    ok = (seg >= lam * (1 - 1e-12)) & (np.append(seg[1:], 0.0) <= lam * (1 + 1e-12))
    if not ok.any():
        from design_lib import wf
        return wf(seg, H)[0]
    kk = int(np.argmax(ok)) + 1; l = min(lam[kk - 1], seg[kk - 1])
    return float(kk * l + seg[kk:].sum())
def t(R): return 1.0 / (4.0 ** R - 1.0)
def make(parts):                   # parts: list of (lo, hi, kind) with kind 'D' or 'B'; D parts share one water level
    D = np.sort(np.concatenate([a[lo:hi] for lo, hi, k in parts if k == "D"]))[::-1] if any(k == "D" for _, _, k in parts) else np.array([])
    Bs = [a[lo:hi] for lo, hi, k in parts if k == "B"]
    segs = ([D] if len(D) else []) + Bs
    cums = [np.cumsum(np.log(s)) for s in segs]
    hasD = len(D) > 0; J = len(Bs)
    def width(x):                  # x = [H_D (bits)] + H_j (bits) + R_j
        off = 0; hD = 0.0
        if hasD: hD = W(cums[0], segs[0], x[0] * math.log(2)); off = 1
        v = 0.0
        for j in range(J):
            Wj = W(cums[off + j], segs[off + j], x[1 + j] * math.log(2)); R = max(x[1 + J + j], 1e-4)
            v += Wj * Wj * t(R) / (len(Bs[j]) - 1)
        return max(hD + C * math.sqrt(v), 1e-300)
    sizes = [len(b) for b in Bs]
    return width, hasD, J, sizes
def solve(parts, bits, rng, nstart=4):
    width, hasD, J, sizes = make(parts)
    w = np.array([1.0 if hasD else 0.0] + [1.0] * J + sizes, float)
    best = None
    for s in range(nstart):
        x = np.zeros(1 + 2 * J); share = rng.uniform(0.05, 0.6)
        x[0] = rng.uniform(0, 0.3) * bits if hasD else 0.0
        x[1:1 + J] = rng.dirichlet(np.ones(J)) * share * bits
        x[1 + J:] = np.maximum(rng.dirichlet(np.ones(J)) * (1 - share) * bits / np.array(sizes), 0.02)
        x *= bits / float(w @ x)
        r = minimize(lambda z: math.log(width(z)), x, method="SLSQP",
                     bounds=[(0, None if hasD else 0)] + [(0, None)] * J + [(0.02, 12)] * J,
                     constraints=[{"type": "eq", "fun": lambda z: float(w @ z) - bits, "jac": lambda z: w}],
                     options={"maxiter": 300, "ftol": 1e-11})
        if abs(float(w @ r.x) - bits) > 1e-3 * bits: continue
        h = width(r.x)
        if best is None or h < best[0]: best = (h, r.x)
    return best
rng = np.random.default_rng(1)
grid = list(range(16, 768, 16))
committed = {32: 0.1972, 56: 0.1024}
for By in (32, 56):
    bits = 8 * By; best = (None, None)
    # the committed layouts, re-evaluated by this implementation
    for cuts in ([0, 64, 128, 256, 768], [0, 96, 192, 320, 768]):
        parts = [(cuts[i], cuts[i + 1], "B") for i in range(len(cuts) - 1)] + ([(cuts[-1], n, "D")] if cuts[-1] < n else [])
        r = solve(parts, bits, rng, 8); print(f"{By} B, committed cuts {cuts}: h/F {r[0] / F:.4f} x {[round(float(v), 3) for v in r[1]]}", flush=True)
    for trial in range(240):
        J = int(rng.integers(1, 7)); head = int(rng.choice([0, 0, 16, 32, 48]))
        inner = sorted(rng.choice([g for g in grid if g > head], size=J, replace=False).tolist())
        tail_end = inner[-1] if rng.random() < 0.6 else 768
        cuts = [head] + inner[:-1] + [tail_end] if tail_end == 768 else [head] + inner
        if len(cuts) < 2 or cuts[-1] <= cuts[0]: continue
        parts = [(cuts[i], cuts[i + 1], "B") for i in range(len(cuts) - 1)]
        if head > 0: parts.append((0, head, "D"))
        if cuts[-1] < n: parts.append((cuts[-1], n, "D"))
        if any(hi - lo < 4 for lo, hi, k in parts if k == "B"): continue
        r = solve(parts, bits, rng, 3)
        if r is not None and (best[0] is None or r[0] < best[0]): best = (r[0], parts)
    print(f"{By} B: best of random layouts h/F {best[0] / F:.4f} (committed {committed[By]}) layout {best[1]}", flush=True)
