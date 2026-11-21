# C5, parts C and D: C3 (c)'s table and C4.3's split with scalars counted from the sources and the constant by number of coded blocks.
# Group 12, c as built (6.931 with one block coded, 7.963 with several). usage: c5d.py
import json, math, numpy as np
from scipy.stats import norm
from design_lib import *
S = spectra(); a, _ = S[12]; F = float(a.sum()); tot = sum(v[1] for v in S.values())
FG = np.array([float(v[0].sum()) for v in S.values()]); WG = np.array([v[1] / tot for v in S.values()])
CUTS = [0, 96, 224, 480, 768]; SIZES = [96, 128, 256, 288]; C1, CM = 6.931, 7.963
RATES = [[2, 0, 0, 0], [3, 1, .25, 0], [4, 3, 1, 0], [7, 4, 2, .25]]
lam = wf(a, 256 * math.log(2))[1]; ap = np.minimum(a, lam); A0 = np.array([ap[CUTS[j]:CUTS[j + 1]].sum() for j in range(4)]) / F
def sched(tf, rd):
    w = [rd * wf(a, 8 * L * math.log(2))[0] / F for L in (4, 8, 12, 16, 24, 32)]
    for rates in RATES:
        J = sum(r > 0 for r in rates); c = C1 if J == 1 else CM
        w.append(c * math.sqrt(sum((rd * A0[j]) ** 2 * tf(rates[j]) / (SIZES[j] - 1) for j in range(4) if rates[j] > 0)) + sum(rd * A0[j] for j in range(4) if rates[j] == 0))
    return w
w_ideal = sched(t_ideal, 1.0); w_built = sched(t_built, RHO_DET)
LEV = [4, 8, 12, 16, 24, 32, 56, 96, 160, 256]
STORED = [4.5, 9, 13.5, 18, 26.5, 35, 64.5, 104.9, 174.8, 274.3]       # bd_p2c.out build line, less the 2 B key
CODE = [4, 8, 12, 16, 24, 32, 56, 92, 160, 253]                          # code bytes by group 0's rates
RISES = [0, 0, 0, 0, 0, 0, 1, 3, 3, 4]
print("built schedule, h/F: ideal curves", np.round(w_ideal, 4).tolist()); print("                    built curves", np.round(w_built, 4).tolist())
# part C: against the scalar-charged optimum of c5b.py
rows = json.load(open("c5b.json")); get = lambda name, by: next((r for r in rows if r["name"].startswith(name) and abs(r["bytes"] - by) < 0.01), None)
print("--- C3 (c) redone, randomised levels: cost measure | level | budget | schedule h/F | optimum h/F (J) | ratio")
for li in range(4):
    r = get("stored bytes", STORED[6 + li]); b = r["byJ"][str(r["best"])] if isinstance(next(iter(r["byJ"])), str) else r["byJ"][r["best"]]
    print(f"  stored bytes, 2.5 B per coded block | {LEV[6 + li]} B | {STORED[6 + li]} | {w_built[6 + li]:.4f} | {b['h_over_F']:.4f} (J={r['best']}) | {w_built[6 + li] / b['h_over_F']:.2f}")
cum = np.cumsum(RISES)
for s in (14, 45):
    for li in range(4):
        by = CODE[6 + li] + s * int(cum[6 + li]); r = get("56 B level" if li == 0 else f"time-equivalent, s = {s}", by)
        if r is None: print(f"  time-equivalent, s = {s} | {LEV[6 + li]} B | {by} | not solved"); continue
        b = r["byJ"][str(r["best"])] if isinstance(next(iter(r["byJ"])), str) else r["byJ"][r["best"]]
        print(f"  time-equivalent bytes, s = {s} per block-rise | {LEV[6 + li]} B | {by} | {w_built[6 + li]:.4f} | {b['h_over_F']:.4f} (J={r['best']}) | {w_built[6 + li] / b['h_over_F']:.2f}")
# part D: C4.3's split in bytes, and a split in time
fr = json.load(open("c3b_g12_c8_ideal.json"))["rows"]; fb = np.array([r["bytes"] for r in fr], float); fh = np.array([r["byJ"][str(r["best"])]["h_over_F"] for r in fr]); FULL = 1536.0
def front(by): return float(np.exp(np.interp(math.log(by), np.log(np.concatenate([[0.25], fb])), np.log(np.concatenate([[1.0], fh])))))
def beta_star(u):
    u = np.asarray(u, float); out = np.zeros_like(u); lb = np.log(np.concatenate([[0.25], fb])); lh = np.log(np.concatenate([[1.0], fh]))
    ins = (u < 1) & (u >= fh[-1]); out[ins] = np.exp(np.interp(-np.log(u[ins]), -lh, lb)); deep = u < fh[-1]; out[deep] = fb[-1] + 96 * np.log2(fh[-1] / np.maximum(u[deep], 1e-300))
    return np.minimum(out, FULL)
Z = np.linspace(-9, 9, 36001); PZ = norm.pdf(Z); PZ /= PZ.sum()
def entrants(p, ratio, widths):                     # share of N entering each level, and reading the full vector
    zt = norm.isf(p); e = np.zeros(len(widths) + 1)
    for r, wg in zip(ratio, WG):
        u = np.abs(Z - zt) / r; cur = (u < 1).astype(float)
        for i, w in enumerate(widths): e[i] += wg * float((PZ * cur).sum()); cur = np.minimum(cur, (u < w).astype(float))
        e[-1] += wg * float((PZ * cur).sum())
    return e
def bytes_of(e, byts): return float(sum(e[i] * (byts[i] - (byts[i - 1] if i else 0)) for i in range(len(byts))) + FULL * e[-1])
T0, TB, TY, TF = 3.2, 0.133, 0.7, 110.0
MEAS = {"item 1%": (18.6, 4.5), "item 50%": (276.2, 12.3), "text 1%": (67.0, 8.8), "text 50%": (322.8, 12.4), "item 0.01%": (2.9, None)}
CELLS = {"item 0.01%": (1e-4, FG / 0.068), "item 1%": (1e-2, FG / 0.068), "item 50%": (.5, FG / 0.068), "text 1%": (1e-2, 1.742 * FG / 0.040), "text 50%": (.5, 1.742 * FG / 0.040)}
print("--- C4.3 redone (bytes per item of N): cell | floor | + levels | + layout | + code | + scalars and key | measured || shares of the last")
for cell, (p, ratio) in CELLS.items():
    zt = norm.isf(p); r1 = sum(wg * float((PZ * beta_star(np.abs(Z - zt) / r)).sum()) for r, wg in zip(ratio, WG))
    r2 = bytes_of(entrants(p, ratio, [front(b) for b in LEV]), LEV); r3 = bytes_of(entrants(p, ratio, w_ideal), LEV)
    eb = entrants(p, ratio, w_built); r4 = bytes_of(eb, CODE); r5 = bytes_of(eb, STORED) + 2.0 * eb[0]
    sh = [r1, r2 - r1, r3 - r2, r4 - r3, r5 - r4]
    print(f"  {cell:10s} | {r1:6.1f} | {r2:6.1f} | {r3:6.1f} | {r4:6.1f} | {r5:6.1f} | {MEAS[cell][0]} || " + " / ".join(f"{x / r5:.0%}" for x in sh))
    scal_b = sum(eb[i] * (STORED[i] - (STORED[i - 1] if i else 0) - (CODE[i] - (CODE[i - 1] if i else 0))) for i in range(10))
    line = []
    for tau in (1.8, 3.0, 10.0):
        lev = T0 * eb[:10].sum(); dec = TB * sum(eb[i] * (CODE[i] - (CODE[i - 1] if i else 0)) for i in range(10)); sc = tau * sum(eb[i] * RISES[i] for i in range(10)) + TY * 4 * eb[6]; fu = TF * eb[-1]
        tt = lev + dec + sc + fu; line.append(f"tau {tau}: {tt:.1f} ns = levels {lev / tt:.0%}, decode {dec / tt:.0%}, scalars {sc / tt:.0%}, full {fu / tt:.0%}")
    print(f"             entering 56/96/160/256 B, full: {np.round(eb[6:], 3).tolist()}; scalars {scal_b:.1f} B = {scal_b / r5:.0%} of bytes; time per item of N: " + " | ".join(line) + (f" | measured scalar decode {MEAS[cell][1]}% of the profiled wall" if MEAS[cell][1] else ""))
