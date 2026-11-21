# C5, the third curve: the first randomised level under the shrunk-centre interval w* (CLAIMS.md, claim A4, unrefereed).
# Curves: plain rule with the built code; plain rule with the ideal code; w* at the built code's angle.
# Several blocks under w*, two readings: S1 "per term" (each block's w*_j^2 summed; an approximation, unproved and optimistic at low rate);
# S2 a construction (shrink m_j per block, bias terms added linearly, random terms in quadrature under the several-block constant,
# worst case over the blocks' s_j, best m). usage: c5c.py
import math, itertools, json, numpy as np
from scipy.optimize import minimize, brentq
from scipy.special import betainc
from scipy.stats import norm
from design_lib import *
def c1(d):
    f = lambda s: math.log(0.5 * betainc((d - 1) / 2, 0.5, 1 - s * s) + 1e-300) + 35.97
    return brentq(f, 1e-3, 0.999) * math.sqrt(d)
CM = 7.963; C1 = {}
S = spectra(); a, _ = S[12]; n = len(a); F = float(a.sum())
tot = sum(v[1] for v in S.values()); FG = np.array([float(v[0].sum()) for v in S.values()]); WG = np.array([v[1] / tot for v in S.values()])
CUTS = [0, 96, 224, 480, 768]; SIZES = [96, 128, 256, 288]
lam = wf(a, 256 * math.log(2))[1]; ap = np.minimum(a, lam); A0 = np.array([ap[CUTS[j]:CUTS[j + 1]].sum() for j in range(4)]) / F
def cc(rates):
    idx = [j for j in range(4) if rates[j] > 0]
    if len(idx) == 1:
        d = SIZES[idx[0]] - 1
        if d not in C1: C1[d] = c1(d)
        return C1[d]
    return CM
def s2(Ac, k):                                                 # coded blocks' sums and k_j: min over shrinks of the worst case over s_j in [0,1]
    J = len(Ac)
    def worst(m):
        al = m * Ac; be2 = (1 - m) ** 2 * Ac ** 2 * k
        g = lambda x: -(float(al @ x) + math.sqrt(max(float(be2 @ (1 - x * x)), 0.0)))
        return -min(minimize(g, x0, method="L-BFGS-B", bounds=[(0, 1)] * J).fun for x0 in (np.full(J, 0.3), np.full(J, 0.9)))
    f = lambda z: worst(1 / (1 + np.exp(-z)))
    m0 = np.clip(k / (1 + k), 1e-3, 1 - 1e-3); z0 = np.log(m0 / (1 - m0))
    r = minimize(f, z0, method="Nelder-Mead", options={"xatol": 1e-3, "fatol": 1e-7, "maxiter": 400})
    return min(r.fun, f(z0))
def width(rates, curve):
    idx = [j for j in range(4) if rates[j] > 0]; unc = [j for j in range(4) if rates[j] == 0]; c = cc(rates)
    A = A0 * (1.0 if curve == "plain ideal" else RHO_DET); tf = t_ideal if curve == "plain ideal" else t_built
    k = np.array([c * c * tf(rates[j]) / (SIZES[j] - 1) for j in idx]); Ac = np.array([A[j] for j in idx]); det = sum(A[j] for j in unc)
    if curve.startswith("plain"): return det + math.sqrt(float((Ac ** 2 * k).sum()))
    if curve == "w* S1": return det + math.sqrt(float((Ac ** 2 * k / (1 + k)).sum()))
    return det + s2(Ac, k)
BITS = 192; MENU = [0, .125, .25, .5, 1, 2, 3, 4]
COMBOS = [r for r in itertools.product(MENU, repeat=4) if 144 <= sum(s * x for s, x in zip(SIZES, r)) <= BITS]
def best_free(curve, J):
    best = None
    for sub in itertools.product((0, 1), repeat=4):
        if sum(sub) != J: continue
        idx = [j for j in range(4) if sub[j]]; w = np.array([SIZES[j] for j in idx], float)
        def f(x):
            r = [0.0] * 4
            for kk, j in enumerate(idx): r[j] = max(x[kk], 1e-4)
            return math.log(width(r, curve))
        for x0 in (np.full(len(idx), BITS / w.sum()), np.array([0.8 * BITS / w[0]] + [0.2 * BITS / max(w[1:].sum(), 1)] * (len(idx) - 1))):
            x0 = np.maximum(x0, 0.02); x0 *= BITS / float(w @ x0)
            r = minimize(f, x0, method="SLSQP", bounds=[(0.02, 12)] * len(idx), constraints=[{"type": "eq", "fun": lambda z: float(w @ z) - BITS, "jac": lambda z: w}], options={"maxiter": 200, "ftol": 1e-10})
            if abs(float(w @ r.x) - BITS) < 1e-3 * BITS and (best is None or r.fun < best[0]):
                rr = [0.0] * 4
                for kk, j in enumerate(idx): rr[j] = float(r.x[kk])
                best = (r.fun, rr)
    return math.exp(best[0]), best[1]
Z = np.linspace(-9, 9, 36001); PZ = norm.pdf(Z); PZ /= PZ.sum()
def share(p, ratio, w):
    zt = norm.isf(p); return sum(wg * float((PZ * (np.abs(Z - zt) / r < w)).sum()) for r, wg in zip(ratio, WG))
CELLS = {"item 1%": (1e-2, FG / 0.068), "item 50%": (.5, FG / 0.068), "text 1%": (1e-2, 1.742 * FG / 0.040), "text 50%": (.5, 1.742 * FG / 0.040)}
T0, TB, CODE96, SB = 3.2, 0.133, 36.0, 1.5
res = {}
for curve in ("plain built", "plain ideal", "w* S1", "w* S2"):
    w32 = (1.0 if curve == "plain ideal" else RHO_DET) * wf(a, 256 * math.log(2))[0] / F
    alts = {"built [2,0,0,0]": (width([2, 0, 0, 0], curve), [2, 0, 0, 0])}
    for J in (2, 3, 4):
        if curve != "w* S2": alts[f"{J} blocks, free"] = best_free(curve, J)
        else: r = res["w* S1"][f"{J} blocks, free"][1]; alts[f"{J} blocks, at S1's free rates"] = (width(r, curve), r)
        mn = min(((width(list(r), curve), list(r)) for r in COMBOS if sum(x > 0 for x in r) == J), key=lambda v: v[0]); alts[f"{J} blocks, menu"] = mn
    res[curve] = alts; hb = alts["built [2,0,0,0]"][0]
    print(f"=== {curve}: width at 32 B {w32:.4f}; first level as built {hb:.4f}")
    for kname, (h, r) in alts.items(): print(f"   {kname:30s} h/F {h:.4f} ({h / hb - 1:+.1%}) rates {[round(x, 2) for x in r]}")
    for cell, (p, ratio) in CELLS.items():
        e56 = share(p, ratio, w32); e96b = share(p, ratio, hb); line = []
        for kname, (h, r) in alts.items():
            J = sum(x > 0 for x in r)
            if J == 1: continue
            d96 = e96b - share(p, ratio, h); extra = e56 * (J - 1)
            tflip = d96 * (T0 + TB * CODE96) / (extra - 3 * d96) if extra > 3 * d96 else float("inf")
            line.append(f"{kname}: 96 B {-d96 / e96b:+.0%}, {extra * SB - d96 * (CODE96 + 3 * SB):+.2f} B, pays below {tflip:.2f} ns")
        print(f"   {cell} (enter 56 B {e56:.3f}, 96 B {e96b:.3f}): " + " | ".join(line))
json.dump({c: {k: [v[0], v[1]] for k, v in al.items()} for c, al in res.items()}, open("c5c.json", "w"))
# hand-over of one flat block under w*: against its own Cauchy-Schwarz bound and against the same bits spent deterministically
print("flat block, ideal angle, w* against F 2^-R (deterministic floor at the same bits): ratio at R = 1/8, 1/4, 1/2, 1, 2")
for d, c in ((95, 6.931), (255, 7.639), (767, 7.953), (63, 6.434), (31, 5.259)):
    q2 = c * c / d; print(f"   d = {d}: q^2 = {q2:.3f}: " + ", ".join(f"{math.sqrt(q2 / (4 ** R - 1 + q2)) * 2 ** R:.3f}" for R in (.125, .25, .5, 1, 2)))
