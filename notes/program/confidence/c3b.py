# C3 (b): the best hybrid layout at a fixed number of bits. usage: c3b.py group c curve(ideal|built) [quick]
# A layout is cuts 0 = n_0 < n_1 < ... < n_J <= n: blocks [n_{j-1}, n_j) are rotated, [n_J, n) is certified
# deterministically. Variables: prefix bits on D and on each block, rate of each block. Width (0.1) of DESIGN-CLAIMS.
import sys, json, math, time, numpy as np
from scipy.optimize import minimize
from design_lib import *
g = int(sys.argv[1]); c = float(sys.argv[2]); curve = sys.argv[3]; quick = len(sys.argv) > 4
a, items = spectra()[g]; n = len(a); F = float(a.sum())
GRID = [0, 16, 32, 48, 64, 96, 128, 160, 192, 224, 256, 320, 384, 480, 576, 672, 768]
BYTES = [2, 4, 8, 12, 16, 24, 32, 40, 48, 56, 72, 96, 128, 160, 208, 256, 384]
if quick: BYTES = [8, 32, 56, 96, 256]
built = curve == "built"
SCAL_BLOCK = 16 if built else 0      # |y| 8 bits, kappa 4, tan phi 4, per coded block
SCAL_DET = 4 if built else 0         # one norm code
_T = {}
def table(lo, hi):
    if (lo, hi) not in _T:
        s = a[lo:hi]
        lam = np.exp(np.linspace(math.log(s.max()), math.log(s.min()), 400))
        H = np.log2(np.maximum(s[None, :] / lam[:, None], 1)).sum(1); W = np.minimum(s[None, :], lam[:, None]).sum(1)
        _T[(lo, hi)] = (H, W, hi - lo)
    return _T[(lo, hi)]
def Wof(tab, H):                      # sum left by a prefix of H bits
    Hg, Wg, m = tab
    if H >= Hg[-1]: w = Wg[-1] * 2.0 ** (-(H - Hg[-1]) / m)
    else: w = float(np.interp(H, Hg, Wg))
    if built: w *= 1 + (RHO_DET - 1) * (1 - math.exp(-H / 16))
    return w
tfun = t_built if built else t_ideal
def width(x, cuts):
    J = len(cuts) - 1
    hD = Wof(table(cuts[-1], n), x[0]) if cuts[-1] < n else 0.0
    v = 0.0
    for j in range(J):
        Wj = Wof(table(cuts[j], cuts[j + 1]), x[1 + j]); R = max(x[1 + J + j], 1e-4)
        v += Wj * Wj * tfun(R) / (cuts[j + 1] - cuts[j] - 1)
    return hD + c * math.sqrt(v)
def solve(cuts, bits, x0=None):
    J = len(cuts) - 1; sizes = [cuts[j + 1] - cuts[j] for j in range(J)]; hasD = cuts[-1] < n
    free = bits - J * SCAL_BLOCK - (SCAL_DET if hasD else 0)
    if free <= 0: return None
    if J == 0: return width([free], cuts), [free]
    w = np.array([1.0 if hasD else 0.0] + [1.0] * J + sizes)
    starts = [x0] if x0 is not None else []
    for share in (0.15, 0.5):
        x = np.zeros(1 + 2 * J); x[1:1 + J] = share * free / J; x[1 + J:] = (1 - share) * free / sum(sizes); starts.append(x)
    best = None
    for x in starts:
        x = np.maximum(np.array(x, float), 0); x[1 + J:] = np.maximum(x[1 + J:], 0.02); x *= free / float(w @ x)
        r = minimize(lambda z: math.log(width(z, cuts)), x, method="SLSQP", bounds=[(0, 0 if not hasD else None)] + [(0, None)] * J + [(0.02, 12)] * J,
                     constraints=[{"type": "eq", "fun": lambda z: float(w @ z) - free, "jac": lambda z: w}], options={"maxiter": 200, "ftol": 1e-10})
        if abs(float(w @ r.x) - free) > 1e-3 * free: continue
        h = width(r.x, cuts)
        if best is None or h < best[0]: best = (h, r.x.tolist())
    return best
def descend(cuts, bits):
    cur = solve(cuts, bits)
    if cur is None: return None
    for sweep in range(6):
        moved = False
        for k in range(1, len(cuts)):
            i = GRID.index(cuts[k])
            for ii in (i - 2, i - 1, i + 1, i + 2):
                if not 0 < ii < len(GRID): continue
                nc = list(cuts); nc[k] = GRID[ii]
                if any(nc[m] >= nc[m + 1] for m in range(len(nc) - 1)): continue
                r = solve(nc, bits, cur[1])
                if r is not None and r[0] < cur[0] * (1 - 1e-6): cur, cuts, moved = r, nc, True
        if not moved: break
    return cur[0], cuts, cur[1]
STARTS = {0: [[0]], 1: [[0, 224], [0, 768]], 2: [[0, 96, 224], [0, 224, 768]], 3: [[0, 96, 224, 480], [0, 128, 384, 768]],
          4: [[0, 96, 224, 480, 768], [0, 64, 192, 384, 768]]}
out = {"group": g, "c": c, "curve": curve, "F": F, "rows": []}; t0 = time.time()
for By in BYTES:
    row = {"bytes": By, "byJ": {}}
    for J in range(5):
        best = None
        for st in STARTS[J]:
            r = descend(st, 8 * By)
            if r is not None and (best is None or r[0] < best[0]): best = r
        if best is not None: row["byJ"][J] = {"h_over_F": best[0] / F, "cuts": best[1], "x": [round(v, 3) for v in best[2]]}
    jb = min(row["byJ"], key=lambda j: row["byJ"][j]["h_over_F"]); row["best"] = jb
    out["rows"].append(row); b = row["byJ"][jb]; J = len(b["cuts"]) - 1
    print(f"{By:4d} B: det {row['byJ'][0]['h_over_F']:.4f} | " + " ".join(f"J{j} {row['byJ'][j]['h_over_F']:.4f}" for j in range(1, 5) if j in row["byJ"])
          + f" | best J={jb} cuts {b['cuts']} prefix bits D {b['x'][0]:.0f} blocks {[round(v) for v in b['x'][1:1 + J]]} rates {[round(v, 2) for v in b['x'][1 + J:]]}  ({time.time() - t0:.0f} s)", flush=True)
    json.dump(out, open(f"c3b_g{g}_c{c:g}_{curve}.json", "w"))
