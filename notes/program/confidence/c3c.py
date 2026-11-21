# C3 (c): the built schedule in the model of DESIGN-CLAIMS section 0, against the optimum of c3b.py.
# usage: c3c.py group c    (reads c3b_g<g>_c<c>_{ideal,built}.json if present)
import sys, json, math, os, itertools, numpy as np
from scipy.optimize import minimize
from design_lib import *
g = int(sys.argv[1]); c = float(sys.argv[2]); a, items = spectra()[g]; n = len(a); F = float(a.sum())
CUTS = [0, 96, 224, 480, 768]; SIZES = [96, 128, 256, 288]
DET = [4, 8, 12, 16, 24, 32]                                  # code bytes of the deterministic levels
RAND = {56: [2, 0, 0, 0], 96: [3, 1, .25, 0], 160: [4, 3, 1, 0], 256: [7, 4, 2, .25]}   # group 0 of the 1M build (codes/NOTEBOOK entry 1)
STORED = {4: 4.5, 8: 9, 12: 13.5, 16: 18, 24: 27, 32: 36, 56: 65.5, 96: 107.5, 160: 174.6, 256: 275.3}   # with norm codes and scalars, without the key
lam = wf(a, 256 * math.log(2))[1]; ap = np.minimum(a, lam); A = np.array([ap[CUTS[j]:CUTS[j + 1]].sum() for j in range(4)])
print(f"group {g}, c = {c:g}, F = {F:.4f}. After a 256-bit ideal prefix: block sums / F = {np.round(A / F, 4).tolist()}, total {A.sum() / F:.4f}; block mean amplitudes / level = {np.round(A / np.array(SIZES) / lam, 3).tolist()}")
def hrand(rates, tf, Ab, cc=c):
    v = sum(Ab[j] ** 2 * tf(rates[j]) / (SIZES[j] - 1) for j in range(4) if rates[j] > 0)
    return cc * math.sqrt(v) + sum(Ab[j] for j in range(4) if rates[j] == 0), cc * math.sqrt(v)
def best_rates(bits, tf, Ab):                                # built cuts, built 256-bit prefix, free rates, any coded subset
    best = None
    for sub in itertools.product((0, 1), repeat=4):
        idx = [j for j in range(4) if sub[j]]
        if not idx: continue
        w = np.array([SIZES[j] for j in idx], float)
        def f(x):
            r = [0.0] * 4
            for k, j in enumerate(idx): r[j] = max(x[k], 1e-4)
            return math.log(hrand(r, tf, Ab)[0])
        for x0 in (np.full(len(idx), bits / w.sum()), np.array([bits / w[0]] + [0.02] * (len(idx) - 1))):
            x0 = np.maximum(x0, 0.02); x0 *= bits / float(w @ x0)
            r = minimize(f, x0, method="SLSQP", bounds=[(0.02, 12)] * len(idx), constraints=[{"type": "eq", "fun": lambda z: float(w @ z) - bits, "jac": lambda z: w}], options={"maxiter": 200, "ftol": 1e-10})
            if abs(float(w @ r.x) - bits) > 1e-3 * bits: continue
            h = math.exp(r.fun)
            if best is None or h < best[0]:
                rr = [0.0] * 4
                for k, j in enumerate(idx): rr[j] = round(float(r.x[k]), 2)
                best = (h, rr)
    return best
opt = {}
for cv in ("ideal", "built"):
    fn = f"c3b_g{g}_c{c:g}_{cv}.json"
    if os.path.exists(fn):
        rows = json.load(open(fn))["rows"]
        opt[cv] = (np.array([r["bytes"] for r in rows], float), np.array([r["byJ"][str(r["best"])]["h_over_F"] for r in rows]), np.array([r["byJ"]["0"]["h_over_F"] for r in rows]))
def at(cv, by, k=1):                                         # optimum's h/F at a number of bytes (log-log interpolation)
    if cv not in opt: return float("nan")
    return float(np.exp(np.interp(math.log(by), np.log(opt[cv][0]), np.log(opt[cv][k]))))
def bytes_for(cv, h):
    if cv not in opt: return float("nan")
    x, y = opt[cv][0], opt[cv][1]
    if h > y[0] or h < y[-1]: return float("nan")
    return float(np.exp(np.interp(-math.log(h), -np.log(y), np.log(x))))
res = []
for cv, tf, rd in (("ideal", t_ideal, 1.0), ("built", t_built, RHO_DET)):
    print(f"--- {cv} curves: level | bytes charged | h/F of the built schedule (of which c sd) | optimum h/F at the same bytes | ratio | optimum's bytes for the same h | bytes ratio | built cuts and prefix with free rates: h/F, rates")
    for L in DET:
        by = L if cv == "ideal" else STORED[L]
        h = rd * wf(a, 8 * L * math.log(2))[0] / F; o = at(cv, by); bo = bytes_for(cv, h)
        print(f"  det {L:3d} B | {by:6.1f} | {h:.4f} | {o:.4f} | {h / o:.3f} | {bo:6.1f} | {by / bo:.2f}")
        res.append({"curve": cv, "level": L, "bytes": by, "h_over_F": h, "opt": o, "kind": "det"})
    for L, rates in RAND.items():
        by = L if cv == "ideal" else STORED[L]; Ab = A * rd
        h, hs = hrand(rates, tf, Ab); h /= F; hs /= F; o = at(cv, by); bo = bytes_for(cv, h)
        fr = best_rates(8 * (L - 32), tf, Ab)
        print(f"  rand {L:3d} B | {by:6.1f} | {h:.4f} ({hs:.4f}) | {o:.4f} | {h / o:.3f} | {bo:6.1f} | {by / bo:.2f} | {fr[0] / F:.4f} {fr[1]}")
        res.append({"curve": cv, "level": L, "bytes": by, "h_over_F": h, "sd_part": hs, "opt": o, "kind": "rand", "free_rates": fr[0] / F})
json.dump({"group": g, "c": c, "F": F, "rows": res}, open(f"c3c_g{g}_c{c:g}.json", "w"))
