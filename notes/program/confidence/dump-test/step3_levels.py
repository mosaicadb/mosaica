# Step 3 per level and step 4: entrants, decisions and margins in units of the level's half-width; needless entries and
# early exits; the half-width's parts; coded-block scalar reads. usage: python3 step3_levels.py c0 c1 c2 c3 tag
import sys, json, math, numpy as np, dt
from scipy.stats import norm
C = [float(x) for x in sys.argv[1:5]]; tag = sys.argv[5]
r = dt.load(); lv = r["lv"].astype(int); mu = np.abs(r["tr"].astype(np.float64)); es = r["es"].astype(np.float64)
h = dt.halfwidth(r, C); ent, surv = dt.replay(r, h)
unc = dt.uncoded_cs(r); rnd = np.asarray(C)[lv] * r["sd"]; det = r["hn"] - unc
cod = (r["cod"][:, None] >> np.arange(4)) & 1
R = r["R"].astype(np.float64); a = r["a"].astype(np.float64); tanp = np.where(cod == 1, a / np.maximum(R, 1e-30), 0.0)
# a rise of block j at level l: coded at l and (not coded at l-1, or its tan phi changed); level 0: coded there
prev = np.roll(np.arange(len(lv)), 1); rise = cod.copy().astype(bool)
d = lv > 0
rise[d] = (cod[d] == 1) & ((cod[prev[d]] == 0) | (np.abs(tanp[d] - tanp[prev[d]]) > 1e-6 * np.maximum(tanp[d], 1e-30)))
hprev = np.where(lv > 0, h[prev], np.inf)
x = mu / h                                         # margin in units of this level's half-width
inn = ent & ~surv & (es >= 0); out = ent & ~surv & (es < 0); und = ent & surv
needless = ent & (lv > 0) & (mu > hprev)           # entered although its margin already exceeded the previous half-width
early = ent & ~surv & (mu <= h)                    # decided on the estimate although its true margin is inside the half-width
late = und & (mu > h)                              # kept although its true margin is outside the half-width
res = {"constants": C, "cells": {}}
def q(v, p): return float(np.percentile(v, p)) if len(v) else float("nan")
print(f"constants {C}")
for cls in (0, 1):
    for s in (3, 4, 5, 6):
        cm = (r["text"] == cls) & (r["size"] == s); nq = 10
        print(f"\n{'text' if cls else 'item'} {dt.SIZES[s]:.0%}: per level, counts summed over {nq} queries (share of N per query in brackets)")
        print("  level | entrants | in | out | undecided | mu/h decided p10/p50/p90 | mu/h undecided p10/p50/p90 | needless entries (mu > previous h) | B/item they add here | early exits | kept with mu > h | h parts: c sd / uncoded CS / other det (share of mean h) | mean h | (item, coded block) | rises")
        cell = []
        for l in range(4):
            m = cm & (lv == l); e = m & ent; n = int(e.sum())
            dec = e & ~surv; un = e & surv
            ne = int((m & needless).sum()); mh = h[e].mean()
            sh = [rnd[e].mean() / mh, unc[e].mean() / mh, det[e].mean() / mh]
            ncb = int(cod[e].sum()); nrise = int(rise[e].sum())
            row = {"level": l, "entrants": n, "in": int((m & inn).sum()), "out": int((m & out).sum()), "undecided": int(un.sum()),
                   "mu/h decided": [q(x[dec], 10), q(x[dec], 50), q(x[dec], 90)], "mu/h undecided": [q(x[un], 10), q(x[un], 50), q(x[un], 90)],
                   "needless": ne, "needless B/item": ne * dt.BY[l] / dt.NROWS / nq, "early exits": int((m & early).sum()), "kept with mu > h": int((m & late).sum()),
                   "h shares rand/unc/det": sh, "mean h": mh, "item-coded-block pairs": ncb, "rises": nrise}
            cell.append(row)
            print(f"  {l} | {n:7d} ({n / nq / dt.NROWS:.3f}) | {row['in']:6d} | {row['out']:6d} | {row['undecided']:7d} | "
                  f"{row['mu/h decided'][0]:.2f}/{row['mu/h decided'][1]:.2f}/{row['mu/h decided'][2]:.2f} | {row['mu/h undecided'][0]:.2f}/{row['mu/h undecided'][1]:.2f}/{row['mu/h undecided'][2]:.2f} | "
                  f"{ne:6d} ({ne / max(n, 1):.3f}) | {row['needless B/item']:.2f} | {row['early exits']:6d} | {row['kept with mu > h']:6d} | "
                  f"{sh[0]:.3f} / {sh[1]:.3f} / {sh[2]:.4f} | {mh:.5f} | {ncb / nq:.0f} per query | {nrise / nq:.0f} per query")
        full = int((cm & ent & surv & (lv == 3)).sum())
        print(f"  full vectors: {full} ({full / nq / dt.NROWS:.4f} of N per query)")
        res["cells"][f"{'text' if cls else 'item'} {dt.SIZES[s]}"] = {"levels": cell, "full": full}
# step 4 for every size: shares of mean h and scalar reads per query by level
print("\nstep 4: share of each level's mean half-width (c sd / uncoded CS / other det) and (item, coded block) pairs and rises per query, every size")
for cls in (0, 1):
    for s in range(7):
        cm = (r["text"] == cls) & (r["size"] == s); parts = []
        for l in range(4):
            e = cm & (lv == l) & ent; mh = h[e].mean()
            parts.append(f"L{l}: {rnd[e].mean() / mh:.2f}/{unc[e].mean() / mh:.2f}/{det[e].mean() / mh:.3f} pairs {cod[e].sum() / 10:.0f} rises {rise[e].sum() / 10:.0f}")
        print(f"  {'text' if cls else 'item'} {dt.SIZES[s]:7.2%}: " + " | ".join(parts))
# margins against the Gaussian: share of all N within x of t, measured, model Gaussian (sd 0.068 / 0.040), Gaussian with each query's own sd
Sc = np.load("work/scores.npy"); T = np.load("work/thresholds.npy"); SIG = {0: 0.068, 1: 0.040}
print("\nshare of all rows within x of the threshold: measured | Gaussian, model sd | Gaussian, the query's own mean and sd   (mean over the 10 queries)")
XS = [0.0025, 0.005, 0.01, 0.02, 0.04, 0.08]
for cls in (0, 1):
    for s in (2, 3, 4, 5, 6):
        meas = []; gm = []; go = []
        for qq in range(10 * cls, 10 * cls + 10):
            sc = Sc[qq]; t = T[qq, s]; mu_ = np.abs(sc - t)
            meas.append([float((mu_ < xx).mean()) for xx in XS])
            zt = norm.isf(dt.SIZES[s]); go_ = []
            gm.append([float(norm.cdf(zt + xx / SIG[cls]) - norm.cdf(zt - xx / SIG[cls])) for xx in XS])
            m_, s_ = sc.mean(), sc.std(); zq = (t - m_) / s_
            go.append([float(norm.cdf(zq + xx / s_) - norm.cdf(zq - xx / s_)) for xx in XS])
        meas, gm, go = np.mean(meas, 0), np.mean(gm, 0), np.mean(go, 0)
        print(f"  {'text' if cls else 'item'} {dt.SIZES[s]:5.0%}: " + "  ".join(f"x={xx}: {a_:.4f} | {b_:.4f} | {c_:.4f}" for xx, a_, b_, c_ in zip(XS, meas, gm, go)))
json.dump(res, open(f"levels_{tag}.json", "w"), indent=1)
