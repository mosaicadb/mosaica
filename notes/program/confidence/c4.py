# C4: the rate-margin law evaluated for a Gaussian score distribution. usage: c4.py group c
# Reads c3b_g<g>_c<c>_ideal.json (the optimum frontier) and c3c_g<g>_c<c>.json (the built schedule in the model).
import sys, json, math, numpy as np
from scipy.stats import norm
from design_lib import *
g = int(sys.argv[1]); c = float(sys.argv[2])
S = spectra(); tot = sum(v[1] for v in S.values()); FG = np.array([float(v[0].sum()) for v in S.values()]); WG = np.array([v[1] / tot for v in S.values()])
fr = json.load(open(f"c3b_g{g}_c{c:g}_ideal.json"))["rows"]
fb = np.array([r["bytes"] for r in fr], float); fh = np.array([r["byJ"][str(r["best"])]["h_over_F"] for r in fr]); fd = np.array([r["byJ"]["0"]["h_over_F"] for r in fr])
sc = json.load(open(f"c3c_g{g}_c{c:g}.json"))["rows"]
LEV = [4, 8, 12, 16, 24, 32, 56, 96, 160, 256]; FULL = 1536.0
STORED = [4.5, 9, 13.5, 18, 27, 36, 65.5, 107.5, 174.6, 275.3]
def front(hh, by):                       # optimum width at a number of bytes, log-log through (0+, 1)
    return float(np.exp(np.interp(math.log(by), np.log(np.concatenate([[0.25], fb])), np.log(np.concatenate([[1.0], hh])))))
def beta_star(u, hh):                    # bytes the frontier needs for a normalised width u; 96 B per halving beyond the grid
    u = np.asarray(u, float); out = np.zeros_like(u)
    lb = np.log(np.concatenate([[0.25], fb])); lh = np.log(np.concatenate([[1.0], hh]))
    inside = (u < 1) & (u >= hh[-1]); out[inside] = np.exp(np.interp(-np.log(u[inside]), -lh, lb))
    deep = u < hh[-1]; out[deep] = fb[-1] + 96 * np.log2(hh[-1] / np.maximum(u[deep], 1e-300))
    return np.minimum(out, FULL)
w_opt = [front(fh, b) for b in LEV]
w_ideal = [r["h_over_F"] for r in sc if r["curve"] == "ideal"]; w_built = [r["h_over_F"] for r in sc if r["curve"] == "built"]
w_free = [r.get("free_rates", r["h_over_F"]) for r in sc if r["curve"] == "ideal"]
print("level widths h/F:"); print("  levels          ", LEV)
for nm, w in (("optimum frontier", w_opt), ("built, ideal curves", w_ideal), ("built cuts+prefix, free rates", w_free), ("built, built curves", w_built)): print(f"  {nm:30s}", np.round(w, 4).tolist())
def stair(u, widths, byts, smear, key=0.0):
    """Mean bytes for normalised margins u: an item enters level l when it survived level l-1; survival of a level of width w is
    |u + (w/c_e) Z| <= w with c_e = 8 when smear is on, else u < w. Undecided after the last level reads the full vector."""
    u = np.asarray(u, float)
    def surv(w):
        if not smear: return (u < w).astype(float)
        s = w / 8.0; return norm.cdf((w - u) / s) - norm.cdf((-w - u) / s)
    cur = surv(1.0); cand = cur.copy(); out = key * cur; prev = 0.0
    for w, b in zip(widths, byts):
        out += (b - prev) * cur; prev = b; cur = np.minimum(cur, surv(w))
    return out + FULL * cur, cur, cand
Z = np.linspace(-9, 9, 36001); PZ = norm.pdf(Z); PZ /= PZ.sum()
def expect(fn, p, ratio):                # ratio = eta_g / sigma_s per group; scores standard Gaussian, threshold at answer share p
    zt = norm.isf(p); acc = None
    for r, wg in zip(ratio, WG):
        v = fn(np.abs(Z - zt) / r); v = v if isinstance(v, tuple) else (v,)
        cur = [float((PZ * x).sum()) * wg for x in v]; acc = cur if acc is None else [x + y for x, y in zip(acc, cur)]
    return acc
MEAS = {"in": {1e-4: (2.9, .11), 1e-3: (5.8, .24), 1e-2: (18.6, .71), .1: (133.2, 2.85), .5: (276.2, 8.43)},
        "ood": {1e-4: (20.6, .55), 1e-3: (26.1, .40), 1e-2: (67.0, .78), .1: (184.5, 4.80), .5: (322.8, 10.38)}}
CAND = {"in": 0.22, "ood": 0.95}; SIG = {"in": 0.068, "ood": 0.040}
out = {}
for q in ("in", "ood"):
    # kappa: eta_g = kappa F_g. Calibrated so that the share of items within eta of the threshold at a 1% answer is the measured candidate share.
    def candshare(k): return expect(lambda u: (u < 1).astype(float), 1e-2, k * FG / SIG[q])[0]
    lo, hi = 0.05, 20.0
    for _ in range(60):
        mid = math.sqrt(lo * hi)
        if candshare(mid) < CAND[q]: lo = mid
        else: hi = mid
    kcal = hi
    for kname, kap in (("kappa = 1", 1.0), (f"kappa = {kcal:.3f} (calibrated on the candidate share at 1%)", kcal)):
        ratio = kap * FG / SIG[q]
        print(f"--- {q}-distribution queries, sigma_s = {SIG[q]}, {kname}: mean eta/sigma_s = {float((ratio * WG).sum()):.2f}")
        print("   answer | candidates | (1) floor | (2) + levels | (3) + built layout | (3b) built cuts, free rates | (4) + built code | (5) + scalars, key | (5) with smear | measured B/item | full-vector share: (5) smear / measured")
        for p in (1e-4, 1e-3, 1e-2, .1, .5):
            r1 = expect(lambda u: beta_star(u, fh), p, ratio)[0]
            r1d = expect(lambda u: beta_star(u, fd), p, ratio)[0]
            r2 = expect(lambda u: stair(u, w_opt, LEV, False)[0], p, ratio)[0]
            r3 = expect(lambda u: stair(u, w_ideal, LEV, False)[0], p, ratio)[0]
            r3b = expect(lambda u: stair(u, w_ideal[:6] + w_free[6:], LEV, False)[0], p, ratio)[0]
            r4 = expect(lambda u: stair(u, w_built, LEV, False)[0], p, ratio)[0]
            r5 = expect(lambda u: stair(u, w_built, STORED, False, 2.0), p, ratio)
            r6 = expect(lambda u: stair(u, w_built, STORED, True, 2.0), p, ratio)
            m = MEAS[q][p]
            print(f"   {p:7.4%} | {r5[2]:6.1%} | {r1:6.1f} | {r2:6.1f} | {r3:6.1f} | {r3b:6.1f} | {r4:6.1f} | {r5[0]:6.1f} | {r6[0]:6.1f} | {m[0]:6.1f} | {r6[1]:6.2%} / {m[1]:.2f}%   (deterministic-only floor {r1d:.1f})")
            out[f"{q}|{kname}|{p}"] = [r1, r2, r3, r3b, r4, r5[0], r6[0], m[0], r6[1], r5[2], r1d]
json.dump(out, open(f"c4_g{g}_c{c:g}.json", "w"))
