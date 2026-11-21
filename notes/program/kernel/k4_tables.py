# Referee checks 4 and 5: (a) inequality (1), |sigma Theta + B - A| <= e, for the tables as built by build_lut (step
# from int_sigma) and by combine_lut (step from enable_int's det_spans rule), worst case over all codes, against the
# real table on the same weights; the accumulator budget; clamping. (b) End to end: integer decisions at thresholds
# placed on the decision boundary against the real certificate est -/+ hh evaluated from the real table sum.
import math
import sys
from fractions import Fraction as F

import numpy as np

from kemu import *

rng = np.random.default_rng(int(sys.argv[1]) if len(sys.argv) > 1 else 11)
EPS = 2.0 ** -46


def thresholds(s, mr, t, tol, fixed, qm, alr, dmax, sigma):
    cin = t - tol + fixed - qm
    cout = t - dmax * fixed - qm
    mag = abs(t) + abs(fixed) + abs(qm)
    vinv, vse, r = 1.0 / sigma, EPS / sigma, 1.0 / s
    a = alr * mr
    slack = (mag + abs(a)) * r * vse
    yi = (cin - a) * r * vinv
    yo = (cout - a) * r * vinv
    ti = math.ceil(yi + slack + abs(yi) * EPS)
    to = math.floor(yo - slack - abs(yo) * EPS)
    return int(min(max(ti, TMIN), TMAX)), int(min(max(to, TMIN), TMAX))


stats = dict(trials=0, worst_ratio=0.0, viol1=0, clamped=0, qover=0, dec_in=0, dec_out=0, imp_fail=0, tight=0, sent=0,
             e_steps_max=0.0)
kinds = ["plain", "spread", "cancel", "big_alpha", "tiny"]
schedules = [[8, 8, 8, 8, 16, 16], [32, 32], [64], [8, 56], [4, 4, 4, 4]]
per_kind = {k: [0, 0.0] for k in kinds}
esteps = {(k, g): [] for k in kinds for g in (False, True)}

for trial in range(600):
    kind = kinds[trial % len(kinds)]
    sched = schedules[(trial // len(kinds)) % len(schedules)]
    tkind = ["bits", "tree", "mixed"][trial % 3]
    levels, nw = [], 0
    for u in sched:
        lv, nw = make_level(rng, u, nw, tkind)
        levels.append(lv)
    scale = np.exp(rng.normal(size=nw) * (2.5 if kind == "spread" else 1.0))
    wq = (rng.normal(size=nw) * scale * (1e-20 if kind == "tiny" else 1.0)).astype(f32)
    wm = (rng.normal(size=nw) * scale * (1e-20 if kind == "tiny" else 1.0)).astype(f32)
    alpha = f32(rng.normal() * (200.0 if kind == "big_alpha" else 1.0))
    if kind == "cancel":   # the query nearly alpha times the centre in these coordinates
        wq = (float(alpha) * wm.astype(np.float64) * (1.0 + 1e-3 * rng.normal(size=nw))).astype(f32)
    for gtab in (False, True):
        if gtab:
            gts = [group_tab(wq, lv) for lv in levels]
            cts = [centre_tab(wm, lv) for lv in levels]
            sigma = det_sigma_gtab(gts, cts, levels, alpha)
            w64 = wq.astype(np.float64) - float(alpha) * wm.astype(np.float64)
            luts = [combine_lut(gt, ct, lv, alpha, sigma) for gt, ct, lv in zip(gts, cts, levels)]
        else:
            w = (wq - (alpha * wm).astype(f32)).astype(f32)
            sigma = int_sigma(w, levels)
            w64 = w.astype(np.float64)
            luts = [build_lut(w, lv, sigma) for lv in levels]
        sg = float(sigma)
        b = e = q = 0.0
        hi_dev = lo_dev = 0.0
        codes = [rng.integers(0, 16, size=(lv.units, 40)) for lv in levels]   # 40 items
        theta_items = np.zeros(40, dtype=np.int64)
        a_items = np.zeros(40)
        for li, (lv, lut) in enumerate(zip(levels, luts)):
            ideal = ideal_table(w64, lv)
            dev = sg * lut["theta"].astype(np.float64) - ideal
            hi_dev += float(dev.max(axis=1).sum())
            lo_dev += float(dev.min(axis=1).sum())
            b += float(lut["off"])
            e += float(lut["err"]) * (1.0 + 1e-6)
            q += lut["qsum"]
            stats["clamped"] += lut["clamped"]
            if q > 256 * IHMAX or float(lut["theta"].max(axis=1).sum()) > lut["qsum"]:
                stats["qover"] += 1
            # inequality (1), worst case over every code of every unit through this level
            ratio = max(hi_dev + b, -(lo_dev + b)) / e
            stats["worst_ratio"] = max(stats["worst_ratio"], ratio)
            per_kind[kind][0] += 1
            per_kind[kind][1] = max(per_kind[kind][1], ratio)
            stats["viol1"] += ratio > 1.0
            stats["e_steps_max"] = max(stats["e_steps_max"], e / sg / sum(l.units for l in levels[:li + 1]))
            esteps[(kind, gtab)].append(e / sg / sum(l.units for l in levels[:li + 1]))
            # end to end at this level
            uu = np.arange(lv.units)[:, None]
            theta_items += lut["theta"][uu, codes[li]].sum(axis=0)
            a_items += ideal[uu, codes[li]].sum(axis=0)
            assert theta_items.max() <= THETA_MAX
            d_l = float(f32(rng.choice([1.0, 1.0, 0.6, 0.2])))
            dmax = max(d_l, float(f32(rng.choice([1.0, 0.9]))))
            x_n = abs(rng.normal()) * abs(float(np.abs(w64).sum())) * 10 ** rng.uniform(-3, 0) + 1e-9 * abs(float(np.abs(w64).sum()))
            ri, ro = margins(b, e, sg, d_l, x_n)
            stats["sent"] += (ri == RSENT) + (ro == RSENT)
            for it in range(40):
                mn = float(f32(rng.uniform(0.5, 1.0)))
                akey = rng.integers(-32767, 32768) / 32767.0
                s = float(f32(math.sqrt(max(1 + mn * mn - 2 * mn * akey, 1e-12) / 10 ** rng.uniform(-2, 1))))
                mr = float(f32(mn * akey - mn * mn))
                qm = float(f32(rng.uniform(-1, 1)))
                fixed = float(f32(10 ** rng.uniform(-6, -3)))
                alr = float(alpha)
                tol = 0.0 if rng.random() < 0.7 else 10 ** rng.uniform(-6, -1)
                A = float(a_items[it])
                est = qm + alr * mr + s * A
                hh = s * x_n + fixed
                # thresholds on the two decision boundaries, a few steps either side
                for side in (0, 1):
                    edge = est - hh + tol if side == 0 else est + d_l * s * x_n + dmax * fixed
                    t = edge + rng.normal() * 3.0 * s * sg * (1.0 if rng.random() < 0.8 else 40.0)
                    tin, tout = thresholds(s, mr, t, tol, fixed, qm, alr, dmax, sg)
                    th = int(theta_items[it])
                    is_in = th - ri >= tin
                    is_out = th + ro < tout
                    # the real statements, in exact rationals from the f64 inputs
                    lb = F(qm) + F(alr) * F(mr) + F(s) * F(A) - (F(s) * F(x_n) + F(fixed))
                    ub = F(qm) + F(alr) * F(mr) + F(s) * F(A) + F(d_l) * (F(s) * F(x_n) + F(fixed))
                    if is_in:
                        stats["dec_in"] += 1
                        m = lb - (F(t) - F(tol))
                        stats["tight"] += m < 4 * F(s) * F(sg) + 2 * F(e) * F(s)
                        if m < 0:
                            stats["imp_fail"] += 1
                            print("IN not implied", kind, gtab, float(m))
                    if is_out:
                        stats["dec_out"] += 1
                        m = F(t) - ub
                        stats["tight"] += m < 4 * F(s) * F(sg) + 2 * F(e) * F(s)
                        if m <= 0:
                            stats["imp_fail"] += 1
                            print("OUT not implied", kind, gtab, float(m))
                    assert not (is_in and is_out and tol == 0.0)
    stats["trials"] += 1

print(stats)
print("worst |sigma Theta + B - A| / e by kind (levels checked, worst ratio):", {k: (v[0], round(v[1], 4)) for k, v in per_kind.items()})
print("e in fine steps per unit (median, largest), by kind; the design's figures are 0.5 (own tables) and 3 (combined):")
for (k, g), v in esteps.items():
    print(f"   {k:10s} {'combined' if g else 'own     '}  median {np.median(v):10.3f}  largest {np.max(v):12.3f}")
