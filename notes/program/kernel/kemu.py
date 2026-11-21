# A numpy transcription of kern.rs's table builders, written from a reading of the code at 8c0b316f9 (not run
# against the binary). f32 operations are numpy float32; a fused multiply-add of f32 operands is emulated as
# float32(float64(a) * float64(b) + float64(c)) (the product is exact in f64; the double rounding can differ from a
# true FMA by one f32 ulp at a tie, which is far inside the float terms the bounds carry).
import numpy as np

f32 = np.float32
U32 = 1.0 / 16777216.0
IHMAX = 8128
THETA_MAX = 256 * IHMAX + 64 * 255
TMIN = -24000 * 256
TMAX = 24000 * 256 + 255
RSENT = 32767 * 256
RIN_MIN = THETA_MAX - TMAX + 1
ROUT_MIN = TMIN


def fma32(a, b, c):
    return (np.asarray(a, np.float64) * np.asarray(b, np.float64) + np.asarray(c, np.float64)).astype(f32)


class Level:
    """One deterministic level, one part: units x up to 4 fields. field (u, k): weight index widx[u, k] (-1: none) and
    a 16-entry table E[u, k, :] of f32 values by unit code (already spread over the unit's 16 codes)."""

    def __init__(self, widx, E):
        self.widx = widx
        self.E = E.astype(f32)
        self.units = widx.shape[0]
        self.lo = self.E.min(axis=2)
        self.hi = self.E.max(axis=2)


def make_level(rng, units, w0, kind):
    """Units of four 1-bit fields (+-delta), or of one 4-bit tree field (16 centroids); weights w0.. in order."""
    widx = -np.ones((units, 4), dtype=np.int64)
    E = np.zeros((units, 4, 16), dtype=f32)
    codes = np.arange(16)
    w = w0
    for u in range(units):
        four_bit = kind == "tree" or (kind == "mixed" and rng.random() < 0.3)
        if four_bit:
            cent = np.sort(rng.normal(size=16)).astype(f32) if kind != "tree" else np.sort(rng.normal(size=16) * 1.2).astype(f32)
            widx[u, 0] = w
            E[u, 0] = cent
            w += 1
        else:
            for k in range(4):
                delta = f32(rng.choice([0.798, 0.4, 0.2, 0.1]) * rng.uniform(0.8, 1.2))
                widx[u, k] = w
                E[u, k] = np.where((codes >> k) & 1 == 1, delta, -delta).astype(f32)
                w += 1
    return Level(widx, E), w


def float_table(w, lv):
    """The f32 table of a level as build_lut / group_tab make it: t = fma(w, E, t) over the unit's fields."""
    t = np.zeros((lv.units, 16), dtype=f32)
    for k in range(4):
        has = lv.widx[:, k] >= 0
        wk = np.where(has, w[np.maximum(lv.widx[:, k], 0)], f32(0)).astype(f32)
        t = np.where(has[:, None], fma32(wk[:, None], lv.E[:, k, :], t), t)
    return t


def extremes(w, lv):
    """lo, hi (f32 sums of f32 products), sa as build_lut computes them per unit."""
    lo = np.zeros(lv.units, dtype=f32)
    hi = np.zeros(lv.units, dtype=f32)
    sa = np.zeros(lv.units, dtype=f32)
    for k in range(4):
        has = lv.widx[:, k] >= 0
        wk = np.where(has, w[np.maximum(lv.widx[:, k], 0)], f32(0)).astype(f32)
        a = (wk * lv.lo[:, k]).astype(f32)
        b = (wk * lv.hi[:, k]).astype(f32)
        lo = np.where(has, (lo + np.minimum(a, b)).astype(f32), lo)
        hi = np.where(has, (hi + np.maximum(a, b)).astype(f32), hi)
        sa = np.where(has, (sa + np.abs(wk) * np.maximum(np.abs(lv.hi[:, k]), np.abs(lv.lo[:, k]))).astype(f32), sa)
    return lo, hi, sa


def ideal_table(w64, lv):
    """The real table on the given weights (f64; the checks' margins are 1e-6 relative, f64 is 1e-16)."""
    t = np.zeros((lv.units, 16))
    for k in range(4):
        has = lv.widx[:, k] >= 0
        wk = np.where(has, w64[np.maximum(lv.widx[:, k], 0)], 0.0)
        t += wk[:, None] * lv.E[:, k, :].astype(np.float64)
    return t


def int_sigma(w, levels):
    rmax = rsum = 0.0
    units = 0
    for lv in levels:
        units += lv.units
        for u in range(lv.units):
            r = 0.0
            for k in range(4):
                if lv.widx[u, k] >= 0:
                    r += abs(float(w[lv.widx[u, k]])) * abs(float(lv.hi[u, k]) - float(lv.lo[u, k]))
            rmax = max(rmax, r)
            rsum += r
    assert units <= 64
    budget = float(256 * IHMAX - 3 * units)
    return max(f32(max(rsum * (1.0 + 1.0 / 512.0) / budget, rmax * (1.0 + 1.0 / 512.0) / 65535.0)), f32(1e-30))


def build_lut(w, lv, step):
    """Returns theta (units x 16 ints), off (f32), scale (f32), err (f32), qsum, and whether a clamp changed a value."""
    t = float_table(w, lv)
    lo, hi, sa = extremes(w, lv)
    rmax = float(np.max((hi - lo).astype(f32)))
    qsum = 0.0
    if step is not None:
        qsum = float(np.sum(np.ceil((hi - lo).astype(f32).astype(np.float64) / float(step) * (1.0 + 1e-5)) + 1.0))
    sabs = float(np.sum(sa.astype(np.float64)))
    off = float(np.sum(lo.astype(np.float64)))
    tab = (t - lo[:, None]).astype(f32)
    scale = f32(step) if step is not None else (f32(rmax) / f32(65535.0) if rmax > 0 else f32(1e-30))
    inv = f32(1.0) / scale
    y = (tab * inv).astype(f32)
    q = np.rint(y.astype(np.float64)).astype(np.int64)          # cvtps_epi32: round to nearest even
    clamped = bool(np.any((q < 0) | (q > 65535)))
    q = np.clip(q, 0, 65535)
    nu = lv.units
    err = nu * float(scale) * 0.5 * (1.0 + 1e-6) + (48.0 + 2.0 * nu) * U32 * sabs + 1e-30
    return dict(theta=q, off=f32(off), scale=scale, err=f32(err), qsum=qsum, clamped=clamped, lo=lo)


def group_tab(wq, lv):
    t = float_table(wq, lv)
    _, _, sa = extremes(wq, lv)       # sabs: sum |w| max|exp|, accumulated in f64 from f32 products in the code
    sabs = 0.0
    for k in range(4):
        has = lv.widx[:, k] >= 0
        wk = np.where(has, wq[np.maximum(lv.widx[:, k], 0)], f32(0)).astype(f32)
        sabs += float(np.sum(np.where(has, np.abs(wk).astype(np.float64) * np.maximum(np.abs(lv.hi[:, k]), np.abs(lv.lo[:, k])).astype(np.float64), 0.0)))
    return dict(t=t, tmin=t.min(axis=1), tmax=t.max(axis=1), err=8.0 * U32 * sabs * (1.0 + 1e-6))


def centre_tab(wm, lv):
    l = build_lut(wm, lv, None)
    return dict(th=l["theta"], tmax=l["theta"].max(axis=1), step=l["scale"], off=float(l["off"]), err=float(l["err"]))


def unit_v(gt, ct, alpha):
    am = f32(alpha) * ct["step"]                                   # f32 product
    v = fma32(-ct["th"].astype(f32), am, gt["t"])                   # fnmadd(m, am, tq) = -(m am) + tq, fused
    return v, v.min(axis=1), v.max(axis=1)


def det_sigma_gtab(gts, cts, levels, alpha):
    s = m = 0.0
    units = 0
    for gt, ct, lv in zip(gts, cts, levels):
        _, lo, hi = unit_v(gt, ct, alpha)
        r = (hi - lo).astype(f32).astype(np.float64)
        s += float(r.sum())
        m = max(m, float(r.max()))
        units += lv.units
    budget = float(256 * IHMAX - 3 * units)
    return max(f32(max(s * (1.0 + 1.0 / 512.0) / budget, m * (1.0 + 1.0 / 512.0) / 65533.0)), f32(1e-30))


def combine_lut(gt, ct, lv, alpha, step):
    alpha = f32(alpha)
    v, lo, hi = unit_v(gt, ct, alpha)
    rmax = float(np.max((hi - lo).astype(f32)))
    sc = f32(step) if step is not None else max(f32(rmax) / f32(65533.0), f32(1e-30))
    inv = f32(1.0) / sc
    asm = abs(float(alpha) * float(ct["step"]))
    off = float(np.sum(lo.astype(np.float64)))
    qsum = float(np.sum(np.ceil((hi - lo).astype(f32).astype(np.float64) / float(sc) * (1.0 + 1e-6)) + 1.0)) if step is not None else 0.0
    mag = (np.maximum(np.abs(gt["tmin"]), np.abs(gt["tmax"])).astype(np.float64) + asm * ct["tmax"].astype(np.float64)
           + np.maximum(np.abs(lo), np.abs(hi)).astype(np.float64)) / float(sc)
    ey = float(np.sum(4.0 * U32 * mag + 3.0 * U32 * 65536.0))
    y = ((v - lo[:, None]).astype(f32) * inv).astype(f32)
    q = np.rint(y.astype(np.float64)).astype(np.int64)
    clamped = bool(np.any((q < 0) | (q > 65535)))
    q = np.clip(q, 0, 65535)                                        # packus_epi32
    off -= float(alpha) * ct["off"]
    nu = lv.units
    err = gt["err"] + abs(float(alpha)) * ct["err"] + float(sc) * (nu * 0.5 * (1.0 + 1e-6) + ey) + 4.0 * U32 * abs(off) + 1e-30
    return dict(theta=q, off=f32(off), scale=sc, err=f32(err), qsum=qsum, clamped=clamped)


def up(x):
    return np.ceil(x + (abs(x) + 1.0) * 1e-12)


def margins(b, e, sg, d, x):
    """rtab on one code: (Rin, Rout) as integers, RSENT when out of range."""
    ri = up((x + e - b) / sg)
    ro = up((b + e + d * x) / sg)
    ri = int(ri) if (ri >= RIN_MIN and ri < RSENT) else RSENT
    ro = int(ro) if (ro >= ROUT_MIN and ro < RSENT) else RSENT
    return ri, ro
