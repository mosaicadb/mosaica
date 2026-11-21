# C5, part A: the layout programme of C3 (b) with a charge of s bytes (or byte-equivalents of time) per coded block and the
# constant depending on the number of coded blocks. Group 12, built curves. usage: c5b.py
import sys, json, math, time, numpy as np
from scipy.optimize import minimize, brentq
from scipy.special import betainc
from design_lib import *
a, items = spectra()[12]; n = len(a); F = float(a.sum())
GRID = [0, 16, 32, 48, 64, 96, 128, 160, 192, 224, 256, 320, 384, 480, 576, 672, 768]
def c1(d):
    f = lambda s: math.log(0.5 * betainc((d - 1) / 2, 0.5, 1 - s * s) + 1e-300) + 35.97
    return brentq(f, 1e-3, 0.999) * math.sqrt(d)
C1 = {}; CM = 7.963
_T = {}
def table(lo, hi):
    if (lo, hi) not in _T:
        s = a[lo:hi]; lam = np.exp(np.linspace(math.log(s.max()), math.log(s.min()), 400))
        _T[(lo, hi)] = (np.log2(np.maximum(s[None, :] / lam[:, None], 1)).sum(1), np.minimum(s[None, :], lam[:, None]).sum(1), hi - lo)
    return _T[(lo, hi)]
def Wof(tab, H):
    Hg, Wg, m = tab
    w = Wg[-1] * 2.0 ** (-(H - Hg[-1]) / m) if H >= Hg[-1] else float(np.interp(H, Hg, Wg))
    return w * (1 + (RHO_DET - 1) * (1 - math.exp(-H / 16)))
def width(x, cuts):
    J = len(cuts) - 1; hD = Wof(table(cuts[-1], n), x[0]) if cuts[-1] < n else 0.0; v = 0.0
    for j in range(J):
        Wj = Wof(table(cuts[j], cuts[j + 1]), x[1 + j]); v += Wj * Wj * t_built(max(x[1 + J + j], 1e-4)) / (cuts[j + 1] - cuts[j] - 1)
    if J == 1:
        d = cuts[1] - cuts[0] - 1
        if d not in C1: C1[d] = c1(d)
        cc = C1[d]
    else: cc = CM
    return hD + cc * math.sqrt(v)
def solve(cuts, bits, sblk, x0=None):
    J = len(cuts) - 1; sizes = [cuts[j + 1] - cuts[j] for j in range(J)]; hasD = cuts[-1] < n
    free = bits - J * sblk - (4 if hasD else 0)
    if free <= 8: return None
    if J == 0: return width([free], cuts), [free]
    w = np.array([1.0 if hasD else 0.0] + [1.0] * J + sizes); starts = [x0] if x0 is not None else []
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
def descend(cuts, bits, sblk):
    cur = solve(cuts, bits, sblk)
    if cur is None: return None
    for sweep in range(5):
        moved = False
        for k in range(1, len(cuts)):
            i = GRID.index(cuts[k])
            for ii in (i - 2, i - 1, i + 1, i + 2):
                if not 0 < ii < len(GRID): continue
                nc = list(cuts); nc[k] = GRID[ii]
                if any(nc[m] >= nc[m + 1] for m in range(len(nc) - 1)): continue
                r = solve(nc, bits, sblk, cur[1])
                if r is not None and r[0] < cur[0] * (1 - 1e-6): cur, cuts, moved = r, nc, True
        if not moved: break
    return cur[0], cuts, cur[1]
STARTS = {0: [[0]], 1: [[0, 96], [0, 224], [0, 768]], 2: [[0, 96, 224], [0, 64, 192], [0, 224, 768]], 3: [[0, 96, 224, 480], [0, 64, 192, 768]], 4: [[0, 96, 224, 480, 768], [0, 64, 192, 384, 768]]}
RUNS = [("56 B level, code 56 B + s for its one block", 56 + s, s) for s in (0, 1.5, 2.5, 5, 8, 11, 14, 18, 23, 30, 45, 75)]
RUNS += [("stored bytes", b, 2.5) for b in (64.5, 104.9, 174.8, 274.3)]
RUNS += [(f"time-equivalent, s = {s}", code + s * rises, s) for s in (14, 45) for code, rises in ((92, 4), (160, 7), (253, 11))]
out = []; t0 = time.time()
for name, by, s in RUNS:
    row = {"name": name, "bytes": by, "s": s, "byJ": {}}
    for J in range(5):
        best = None
        for st in STARTS[J]:
            r = descend(st, 8 * by, 8 * s)
            if r is not None and (best is None or r[0] < best[0]): best = r
        if best is not None: row["byJ"][J] = {"h_over_F": best[0] / F, "cuts": best[1], "x": [round(v, 3) for v in best[2]]}
    jb = min(row["byJ"], key=lambda j: row["byJ"][j]["h_over_F"]); row["best"] = jb; out.append(row); b = row["byJ"][jb]; J = len(b["cuts"]) - 1
    print(f"{name} | budget {by:6.1f} | s {s:4.1f} | " + " ".join(f"J{j} {row['byJ'][j]['h_over_F']:.4f}" for j in row["byJ"]) + f" | best J={jb} cuts {b['cuts']} prefix {[round(v) for v in b['x'][:1 + J]]} rates {[round(v, 2) for v in b['x'][1 + J:]]} ({time.time() - t0:.0f} s)", flush=True)
    json.dump(out, open("c5b.json", "w"))
print("DONE", flush=True)
