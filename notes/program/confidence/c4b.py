# C4, second form: text (out-of-distribution) queries with the measured shells of sublinear/REPORT.md 5.4 in place of a Gaussian tail.
# usage: c4b.py group c kappa
import sys, json, math, numpy as np
from design_lib import *
g = int(sys.argv[1]); c = float(sys.argv[2]); kap = float(sys.argv[3])
S = spectra(); tot = sum(v[1] for v in S.values()); FG = np.array([float(v[0].sum()) for v in S.values()]); WG = np.array([v[1] / tot for v in S.values()])
fr = json.load(open(f"c3b_g{g}_c{c:g}_ideal.json"))["rows"]; fb = np.array([r["bytes"] for r in fr], float); fh = np.array([r["byJ"][str(r["best"])]["h_over_F"] for r in fr])
sc = json.load(open(f"c3c_g{g}_c{c:g}.json"))["rows"]; w_built = [r["h_over_F"] for r in sc if r["curve"] == "built"]
STORED = [4.5, 9, 13.5, 18, 27, 36, 65.5, 107.5, 174.6, 275.3]; FULL = 1536.0; SIG = 0.040
def beta_star(u):
    u = np.asarray(u, float); out = np.zeros_like(u); lb = np.log(np.concatenate([[0.25], fb])); lh = np.log(np.concatenate([[1.0], fh]))
    ins = (u < 1) & (u >= fh[-1]); out[ins] = np.exp(np.interp(-np.log(u[ins]), -lh, lb)); deep = u < fh[-1]; out[deep] = fb[-1] + 96 * np.log2(fh[-1] / np.maximum(u[deep], 1e-300))
    return np.minimum(out, FULL)
def stair(u):
    cur = (u < 1).astype(float); out = 2.0 * cur; prev = 0.0
    for w, b in zip(w_built, STORED): out += (b - prev) * cur; prev = b; cur = np.minimum(cur, (u < w).astype(float))
    return out + FULL * cur
X = np.array([0, 1, 2, 3, 4, 6, 9.0])                         # sd below the threshold
SHELL = {1e-2: [0, .053, .185, .482, .840, .999, 1], 1e-3: [0, .012, .052, .160, .403, .944, 1], 1e-4: [0, .0041, .023, .080, .225, .800, 1]}
MEAS = {1e-2: 67.0, 1e-3: 26.1, 1e-4: 20.6}
xs = np.linspace(0, 9, 9001)[1:] - 0.0005
for p, cdf in SHELL.items():
    # log-linear interpolation of the measured shell shares between the tabulated points, linear in the first interval
    G = np.interp(xs, X, cdf); G[xs > 1] = np.exp(np.interp(xs[xs > 1], X[1:], np.log(cdf[1:]))); dG = np.diff(np.concatenate([[0], G]))
    fl = sum(wg * float((beta_star(xs * SIG / (kap * f)) * dG).sum()) for f, wg in zip(FG, WG)); st = sum(wg * float((stair(xs * SIG / (kap * f)) * dG).sum()) for f, wg in zip(FG, WG))
    cand = sum(wg * float(((xs * SIG / (kap * f) < 1) * dG).sum()) for f, wg in zip(FG, WG))
    print(f"text, answer {p:.2%}, kappa {kap}: candidates {cand:.1%} | floor {fl:.1f} B/item | built schedule with built curves and scalars {st:.1f} | measured {MEAS[p]}")
