"""Referee 1, checks on A1.4 (the beam's work) and A1.5 item 4 (the realised top level). Small numpy; one core.

A. A cascade on synthetic Model T trees: nodes actually scored against W = sum_j B_(j-1) F_j.
B. The terms of W at the sizes of A1.9, with their exponent ln(term)/ln M next to g_j - f_(j-1).
C. Realised top-L level over z_REM(L) against the hull sum, for several share vectors (A1.5 item 4 has one).
"""
import math
import numpy as np
from scipy.stats import norm

rng = np.random.default_rng(5)

print("A. cascade against W (F 28, D 3, L 64, eps 0.01, 400 trees); counts are means per tree")
F, D, L, eps = 28, 3, 64, 0.01
M = F ** D
zT = norm.isf(L / M); ze = norm.ppf(1 - eps)
for shares in ((1 / 3, 1 / 3, 1 / 3), (0.15, 0.25, 0.6), (0.2, 0.3, 0.5), (0.6, 0.25, 0.15)):
    f1, f2 = shares[0], shares[0] + shares[1]
    th1 = zT * f1 - ze * math.sqrt(f1 * (1 - f1)); th2 = zT * f2 - ze * math.sqrt(f2 * (1 - f2))
    B1 = F * norm.sf(th1 / math.sqrt(f1)); B2 = F * F * norm.sf(th2 / math.sqrt(f2))
    s2 = []; s3 = []; above2 = []; tgt = []; got = []
    for _ in range(400):
        y1 = rng.normal(0, math.sqrt(shares[0]), F)
        y2 = np.repeat(y1, F) + rng.normal(0, math.sqrt(shares[1]), F * F)
        u = np.repeat(y2, F) + rng.normal(0, math.sqrt(shares[2]), M)
        k1 = y1 >= th1
        k2 = (y2 >= th2) & np.repeat(k1, F)
        s2.append(F * k1.sum()); s3.append(F * k2.sum()); above2.append((y2 >= th2).sum())
        t = u >= zT
        tgt.append(t.sum()); got.append((t & np.repeat(k2, F)).sum())
    W = F + B1 * F + B2 * F
    print(f"   shares {tuple(round(s, 2) for s in shares)}: W {W:8.0f} = {F} + {B1 * F:.0f} + {B2 * F:.0f};  cascade scored {F + np.mean(s2) + np.mean(s3):8.0f} = {F} + {np.mean(s2):.0f} + {np.mean(s3):.0f};"
          f"  level-2 nodes above their cut {np.mean(above2):.1f} (B_2 {B2:.1f}), of which with parent kept {np.mean(s3) / F:.1f};"
          f"  targets reached {np.sum(got) / max(np.sum(tgt), 1):.4f} (floor {1 - 2 * eps:.2f})")

print("\nB. the terms of W at the sizes of A1.9 (L 10, total miss 0.1 split over the pruning levels)")
fits = {"yahoo": (677305, [(0.31, 0.28), (0.43, 0.42), (0.54, 0.48), (0.64, 0.53), (0.74, 0.58)]),
        "LAION": (1000448, [(0.30, 0.28), (0.45, 0.45), (0.55, 0.51), (0.65, 0.55), (0.75, 0.59)]),
        "glove": (1183514, [(0.30, 0.20), (0.46, 0.36), (0.56, 0.41), (0.66, 0.45), (0.75, 0.49)])}
designs = {"clusters, rows": [1], "groups, clusters, rows": [0, 1], "clusters, a16, rows": [1, 3],
           "clusters, a4, a16, a64, rows": [1, 2, 3, 4]}
for name, (N, prof) in fits.items():
    zT = norm.isf(10 / N); lnN = math.log(N)
    for dname, idx in designs.items():
        lv = [prof[i] for i in idx]
        e = 0.1 / len(lv); ze = norm.ppf(1 - e)
        g = [0.0] + [p[0] for p in lv] + [1.0]; f = [0.0] + [p[1] for p in lv] + [1.0]
        out = []
        for j in range(1, len(g)):
            Kprev = N ** g[j - 1]; Fan = N ** (g[j] - g[j - 1])
            Bprev = 1.0 if j == 1 else Kprev * norm.sf(zT * math.sqrt(f[j - 1]) - ze * math.sqrt(1 - f[j - 1]))
            term = Bprev * Fan
            out.append((term, math.log(term) / lnN, g[j] - f[j - 1]))
        big_exact = int(np.argmax([o[0] for o in out])); big_exp = int(np.argmax([o[2] for o in out]))
        s = "; ".join(f"{o[0]:9.0f} ({o[1]:.2f} | {o[2]:.2f})" for o in out)
        print(f"   {name:5s} {dname:30s} terms, count (ln count/ln N | g_j - f_(j-1)): {s};  largest by count: level {big_exact + 1}, by exponent: level {big_exp + 1}")

print("\nC. realised top-L level over z_REM(L), against sum over the hull of sqrt(df dg) (F 28, D 3, 200 trees)")


def hull_sum(shares):
    pts = [(0.0, 0.0)]
    for i, s in enumerate(shares):
        pts.append(((i + 1) / len(shares), pts[-1][1] + s))
    hull = [pts[0]]; i = 0
    while i < len(pts) - 1:
        best = max(range(i + 1, len(pts)), key=lambda j: (pts[j][1] - pts[i][1]) / (pts[j][0] - pts[i][0]))
        hull.append(pts[best]); i = best
    star = sum(math.sqrt((b[1] - a[1]) * (b[0] - a[0])) for a, b in zip(pts, pts[1:]))
    return sum(math.sqrt((b[1] - a[1]) * (b[0] - a[0])) for a, b in zip(hull, hull[1:])), star


for shares in ((0.6, 0.25, 0.15), (0.8, 0.15, 0.05), (0.9, 0.07, 0.03), (0.34, 0.33, 0.33), (0.15, 0.25, 0.6),
               (0.60, 0.07, 0.33)):
    hs, st = hull_sum(shares)
    line = f"   shares {shares}: hull sum {hs:.3f}, sum over the levels {st:.3f};"
    lev = {1: [], 8: [], 64: [], 512: []}
    for _ in range(200):
        y1 = rng.normal(0, math.sqrt(shares[0]), F)
        y2 = np.repeat(y1, F) + rng.normal(0, math.sqrt(shares[1]), F * F)
        u = np.repeat(y2, F) + rng.normal(0, math.sqrt(shares[2]), M)
        srt = np.sort(u)
        for Lx in lev:
            lev[Lx].append(srt[-Lx])
    for Lx in lev:
        z = norm.isf(Lx / M)
        line += f"  L={Lx}: {np.mean(lev[Lx]) / z:.3f}"
    print(line)
