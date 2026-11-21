# Referee check 2: int_thresholds (kern.rs) and its caller's cin / cout (q.rs 1315-1319), the same f64 operations in
# the same order, against exact rational arithmetic. Sound means Tin >= ceil(Kin/sigma) and Tout <= floor(Kout/sigma)
# before clamping, with Kin = (t - tol + fixed - qm - alr mr)/s and Kout = (t - dmax fixed - qm - alr mr)/s exact.
import math
import random
from fractions import Fraction as F

import numpy as np

EPS = 2.0 ** -46
TMIN = -24000 * 256
TMAX = 24000 * 256 + 255


def f32(x):
    return float(np.float32(x))


def thresholds(s, mr, t, tol, fixed, qm, alr, dmax, sigma):
    """The code's operations. s, mr, fixed, qm, alr, dmax, sigma are f32 values held in f64; t, tol are f64."""
    cin = t - tol + fixed - qm
    cout = t - dmax * fixed - qm
    mag = abs(t) + abs(fixed) + abs(qm)
    vinv = 1.0 / sigma
    vse = EPS / sigma
    r = 1.0 / s
    a = alr * mr
    slack = (mag + abs(a)) * r * vse
    yi = (cin - a) * r * vinv
    yo = (cout - a) * r * vinv
    ti = math.ceil(yi + slack + abs(yi) * EPS)
    to = math.floor(yo - slack - abs(yo) * EPS)
    return ti, to


def exact(s, mr, t, tol, fixed, qm, alr, dmax, sigma):
    S, MR, T, TOL, FX, QM, AL, DM, SG = map(F, (s, mr, t, tol, fixed, qm, alr, dmax, sigma))
    kin = (T - TOL + FX - QM - AL * MR) / S / SG
    kout = (T - DM * FX - QM - AL * MR) / S / SG
    return kin, kout


random.seed(7)
rng = np.random.default_rng(7)
worst_in, worst_out = None, None
bad = 0
n = 0
min_margin_in, min_margin_out = float("inf"), float("inf")


def one(s, mr, t, tol, fixed, qm, alr, dmax, sigma):
    global bad, n, worst_in, worst_out, min_margin_in, min_margin_out
    ti, to = thresholds(s, mr, t, tol, fixed, qm, alr, dmax, sigma)
    kin, kout = exact(s, mr, t, tol, fixed, qm, alr, dmax, sigma)
    n += 1
    if F(ti) < kin or F(to) > kout:
        bad += 1
        if bad <= 5:
            print("VIOLATION", dict(s=s, mr=mr, t=t, tol=tol, fixed=fixed, qm=qm, alr=alr, dmax=dmax, sigma=sigma), ti, float(kin), to, float(kout))
    # how many steps the outward rounding costs beyond the exact ceiling / floor (0 is the least possible)
    min_margin_in = min(min_margin_in, ti - math.ceil(kin))
    min_margin_out = min(min_margin_out, math.floor(kout) - to)


def draw(kind):
    lam = 10 ** rng.uniform(-3, 1)
    mn = f32(rng.uniform(0.3, 1.0))
    a = rng.uniform(-1, 1)
    s2 = max(1 + mn * mn - 2 * mn * a, 1e-12) if kind != "tiny_s" else 1e-12
    s = f32(math.sqrt(s2 / lam))
    mr = f32(mn * a - mn * mn)
    alr = f32(rng.normal() * (1 if kind != "big_alpha" else 300))
    qm = f32(rng.uniform(-1, 1))
    fixed = f32(10 ** rng.uniform(-6, -2))
    sigma = f32(10 ** rng.uniform(-9, -3))
    dmax = f32(1.0 if kind not in ("big_dial",) else 10 ** rng.uniform(0, 6))
    tol = 0.0 if kind != "tol" else 10 ** rng.uniform(-6, 3)
    t = rng.uniform(-1, 1)
    return s, mr, t, tol, fixed, qm, alr, dmax, sigma


for kind in ("plain", "tiny_s", "big_alpha", "big_dial", "tol"):
    b0, n0 = bad, n
    for _ in range(20000):
        s, mr, t, tol, fixed, qm, alr, dmax, sigma = draw(kind)
        one(s, mr, t, tol, fixed, qm, alr, dmax, sigma)
        # put the exact quotient on an integer (to f64 accuracy) and step t by ulps around it: the tie cases
        for side in (0, 1):
            k = int(rng.integers(-3000, 3000))
            base = k * s * sigma + alr * mr + qm + (tol - fixed if side == 0 else dmax * fixed)
            for u in (-3, -2, -1, 0, 1, 2, 3):
                tt = base
                for _ in range(abs(u)):
                    tt = math.nextafter(tt, math.inf if u > 0 else -math.inf)
                one(s, mr, tt, tol, fixed, qm, alr, dmax, sigma)
        # cancellation: cin - a near 0
        tt = alr * mr + qm - fixed + tol
        one(s, mr, tt, tol, fixed, qm, alr, dmax, sigma)
        one(s, mr, math.nextafter(tt, 1), tol, fixed, qm, alr, dmax, sigma)
    print(f"{kind}: {n - n0} cases, violations {bad - b0}")
print(f"total {n} cases, violations {bad}; least excess over the exact rounding: Tin {min_margin_in}, Tout {min_margin_out} steps")

# the clamp: what a NaN or infinite input becomes (x86 max_pd / min_pd return the second operand when either is NaN,
# so min(max(v, lo), hi) sends NaN to lo = TMIN on both thresholds)
def clamp_x86(v):
    m = TMIN if (v != v or TMIN != TMIN) else (v if v > TMIN else TMIN)   # max_pd(v, lo): NaN -> lo
    return TMAX if m != m else (m if m < TMAX else TMAX)                  # min_pd(m, hi)


for v in (float("nan"), float("inf"), float("-inf")):
    print("clamp", v, "->", clamp_x86(v), "(TMIN is the unsafe end for Tin, the safe end for Tout)")
