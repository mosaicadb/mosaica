# Step 3: where the bytes of large in-distribution answers go, against the model of DESIGN-CLAIMS C4.2 row (5).
# usage: python3 step3.py c0 c1 c2 c3 tag
# A chain of substitutions, each in bytes per item of N (49,500), split into deterministic levels (<= 32 B, with the key)
# and randomised levels with full vectors:
#   X0 model: Gaussian scores (sd 0.068 item, 0.040 text), eta = F_g (kappa = 1), the built widths of c4.py row (5)
#   X1 the same with this index's measured score margins (all 49,500 rows, the run's thresholds)
#   X2 the index's deterministic bytes and entrants at 56 B, then the model's absolute widths w_l F_g on those entrants
#   X3a the model's normalised widths (w_l / w_32) times each entrant's own eta = sum_j R_j
#   X3 each entrant's measured half-width at the given constants, decided on its true margin (the law, no estimate error)
#   X4 the index: decided on the estimate (replayed at the given constants)
import sys, json, csv, math, numpy as np, dt
sys.path.insert(0, ".."); from design_lib import spectra
from scipy.stats import norm
C = [float(x) for x in sys.argv[1:5]]; tag = sys.argv[5]
S = spectra(); tot = sum(v[1] for v in S.values()); FG = np.array([float(v[0].sum()) for v in S.values()]); WG = np.array([v[1] / tot for v in S.values()])
sc = json.load(open("../c3c_g12_c8.json"))["rows"]; WB = np.array([r["h_over_F"] for r in sc if r["curve"] == "built"])   # 10 levels, c = 8
ST = np.array([6.5, 11.0, 15.5, 20.0, 28.5, 37.0, 66.5, 107.5, 175.2, 276.3])        # this build's stored bytes, key included
INC = np.diff(np.concatenate([[0.0], ST])); SIG = {0: 0.068, 1: 0.040}
KAP = {0: 1.0, 1: 1.742}                                  # c4.py: item nothing fitted; text with its fitted scale
def model_bytes(mu, kap):
    """Model staircase for margins mu (array), mixture over groups: (det bytes, rand bytes, entrants share by level 0..9, full share)."""
    det = rnd = full = 0.0; ent = np.zeros(10)
    for F, wg in zip(FG, WG):
        eta = kap * F; cur = (mu < eta).astype(float)
        for l in range(10):
            ent[l] += wg * cur.mean()
            b = INC[l] * cur.mean() * wg
            if l < 6: det += b
            else: rnd += b
            cur = cur * (mu < WB[l] * eta)
        full += wg * cur.mean(); rnd += wg * dt.FULL * cur.mean()
    return det, rnd, ent, full
Zg = np.linspace(-9, 9, 36001); PZ = norm.pdf(Zg); PZ /= PZ.sum()
def model_gauss(p, cls):
    zt = norm.isf(p); mu = np.abs(Zg - zt) * SIG[cls]
    det = rnd = full = 0.0; ent = np.zeros(10)
    for F, wg in zip(FG, WG):
        eta = KAP[cls] * F; cur = (mu < eta).astype(float)
        for l in range(10):
            m = float((PZ * cur).sum()); ent[l] += wg * m
            if l < 6: det += INC[l] * m * wg
            else: rnd += INC[l] * m * wg
            cur = cur * (mu < WB[l] * eta)
        m = float((PZ * cur).sum()); full += wg * m; rnd += wg * dt.FULL * m
    return det, rnd, ent, full
# measured
Sc = np.load("work/scores.npy"); T = np.load("work/thresholds.npy")
rows = list(csv.DictReader(open("work/study_res/verify2.csv")))[1::2]
BR = np.zeros((20, 7)); FR = np.zeros((20, 7))
for i, rr in enumerate(rows): BR[int(rr["q"]), i % 7] = float(rr["bytes_read"]); FR[int(rr["q"]), i % 7] = int(rr["full_reads"])
r = dt.load(); lv = r["lv"].astype(int); mu = np.abs(r["tr"].astype(np.float64))
R = r["R"].astype(np.float64); eta = R.sum(1)[r["i0"]]
h933 = dt.halfwidth(r, [dt.C933] * 4); e933, s933 = dt.replay(r, h933)
h = dt.halfwidth(r, C); ent, surv = dt.replay(r, h)               # X4: the index at C
entL, survL = dt.replay(r, h, err=False)                          # X3: the law on measured widths
qs = r["qs"]; uq = np.unique(qs); qi = np.searchsorted(uq, qs)
def per_qs(weights): return np.bincount(qi, weights=weights, minlength=len(uq)) / dt.NROWS
rand933 = per_qs(dt.BY[lv] * e933 + dt.FULL * (e933 & s933 & (lv == 3)))
full933 = np.bincount(qi, weights=(e933 & s933 & (lv == 3)), minlength=len(uq))
X4 = per_qs(dt.BY[lv] * ent + dt.FULL * (ent & surv & (lv == 3)))
X3 = per_qs(dt.BY[lv] * entL + dt.FULL * (entL & survL & (lv == 3)))
# missing records for X3: an item that survives on its true margin but has no record at the next level
nxt_missing = np.zeros(len(lv), bool)
last = np.ones(len(lv), bool); last[:-1] = r["key"][1:] != r["key"][:-1]
nxt_missing = entL & survL & last & (lv < 3)
# X2 and X3a on the measured entrants (level-0 records); each entrant pays 29.5 B at 56 B
m0 = lv == 0; mu0 = mu[m0]; eta0 = eta[m0]; q0 = qi[m0]
def stair_rand(mu_, widths_abs):        # widths_abs: list over the 4 randomised levels of arrays (per entrant) or scalars
    cur = np.ones(len(mu_)); out = np.zeros(len(mu_))
    for l in range(4):
        out += dt.BY[l] * cur; cur = cur * (mu_ < widths_abs[l])
    return out + dt.FULL * cur
X2 = np.zeros(len(uq))
for F, wg in zip(FG, WG):
    X2 += wg * np.bincount(q0, weights=stair_rand(mu0, [WB[6 + l] * F for l in range(4)]), minlength=len(uq)) / dt.NROWS   # kappa = 1: item; text below
X2t = np.zeros(len(uq))
for F, wg in zip(FG, WG):
    X2t += wg * np.bincount(q0, weights=stair_rand(mu0, [WB[6 + l] * KAP[1] * F for l in range(4)]), minlength=len(uq)) / dt.NROWS
X3a = np.bincount(q0, weights=stair_rand(mu0, [WB[6 + l] / WB[5] * eta0 for l in range(4)]), minlength=len(uq)) / dt.NROWS
# the law with one term of the half-width removed
def law_with(parts):
    hh = dt.halfwidth(r, C, parts); e_, s_ = dt.replay(r, hh, err=False)
    return per_qs(dt.BY[lv] * e_ + dt.FULL * (e_ & s_ & (lv == 3)))
X3_nounc = law_with(("rand", "det")); X3_norand = law_with(("unc", "det")); X3_nodet = law_with(("rand", "unc"))
out = {"constants": C, "cells": {}}
print(f"constants {C}; model eta = kappa F_g, mean F = {float((FG * WG).sum()):.4f}; model w_l (built, c = 8) = {np.round(WB, 4).tolist()}")
print(f"measured eta = sum_j R_j at 56 B: mean over entrants, item {eta[m0 & ~r['text']].mean():.4f}, text {eta[m0 & r['text']].mean():.4f}; model's equivalent w_32 F = {WB[5] * float((FG * WG).sum()):.4f}")
print(f"records that the law (true margin) would keep but that have no record at the next level: {int(nxt_missing.sum())} of {int((entL & survL & (lv < 3)).sum())}")
for cls in (0, 1):
    kap = KAP[cls]
    for s in (2, 3, 4, 5, 6):
        qset = [k for k, q_ in enumerate(uq) if ((q_ >> 5) >= dt.NQ_ITEM) == cls and (q_ & 31) == s]
        qids = [int(uq[k] >> 5) for k in qset]
        g = model_gauss(dt.SIZES[s], cls)
        x1 = [model_bytes(np.abs(Sc[q] - T[q, s]), kap) for q in qids]
        x1d = np.mean([a[0] for a in x1]); x1r = np.mean([a[1] for a in x1])
        det_meas = np.mean([BR[q, s] - rand933[k] for q, k in zip(qids, qset)])
        x2 = np.mean((X2 if cls == 0 else X2t)[qset]); x3a = np.mean(X3a[qset]); x3 = np.mean(X3[qset]); x4 = np.mean(X4[qset])
        ent56 = np.mean([float((m0 & (qi == k)).sum()) for k in qset]) / dt.NROWS
        fullchk = max(abs(full933[k] - FR[q, s]) for q, k in zip(qids, qset))
        cell = {"X0 det": g[0], "X0 rand": g[1], "X0 total": g[0] + g[1], "X0 entrants 56..256 B": g[2][6:].tolist(), "X0 full": g[3],
                "X1 det": x1d, "X1 rand": x1r, "X1 total": x1d + x1r, "X1 entrants 56 B": float(np.mean([a[2][6] for a in x1])), "X1 full": float(np.mean([a[3] for a in x1])),
                "measured det": det_meas, "measured entrants 56 B": ent56, "X2 rand": x2, "X3a rand": x3a, "X3 rand": x3, "X4 rand": x4,
                "X3 no uncoded CS": np.mean(X3_nounc[qset]), "X3 no random term": np.mean(X3_norand[qset]), "X3 no other det": np.mean(X3_nodet[qset]),
                "full reads: run vs replay at 9.33, largest difference": float(fullchk)}
        out["cells"][f"{'text' if cls else 'item'} {dt.SIZES[s]}"] = cell
        print(f"\n{'text' if cls else 'item'} {dt.SIZES[s]:.0%} (kappa {kap}): bytes per item of N, deterministic + randomised = total")
        print(f"  X0 model, Gaussian margins         {g[0]:7.1f} + {g[1]:7.1f} = {g[0] + g[1]:7.1f}   entrants 56 B {g[2][6]:.3f}, full {g[3]:.4f}")
        print(f"  X1 model, measured margins         {x1d:7.1f} + {x1r:7.1f} = {x1d + x1r:7.1f}   entrants 56 B {cell['X1 entrants 56 B']:.3f}, full {cell['X1 full']:.4f}")
        print(f"  X2 index det, model abs widths     {det_meas:7.1f} + {x2:7.1f} = {det_meas + x2:7.1f}   entrants 56 B {ent56:.3f}")
        print(f"  X3a model w_l/w_32 x own eta       {det_meas:7.1f} + {x3a:7.1f} = {det_meas + x3a:7.1f}")
        print(f"  X3 measured widths, true margins   {det_meas:7.1f} + {x3:7.1f} = {det_meas + x3:7.1f}   (without uncoded CS {cell['X3 no uncoded CS']:.1f}, without c sd {cell['X3 no random term']:.1f}, without other det {cell['X3 no other det']:.1f})")
        print(f"  X4 index (estimate)                {det_meas:7.1f} + {x4:7.1f} = {det_meas + x4:7.1f}   [full-read check vs run: largest diff {fullchk:.0f}]")
json.dump(out, open(f"step3_{tag}.json", "w"), indent=1)
