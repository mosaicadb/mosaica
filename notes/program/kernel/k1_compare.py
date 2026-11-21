# Referee check 1: the two-word saturating compare of kern.rs (cmp2w) against the full-width integer compare, over the
# ranges the code allows (signed margins down to RIN_MIN / ROUT_MIN, the sentinel, clamped thresholds), and what happens
# to the low-word sum past 127 units. Pure integers; numpy int64 used to emulate i16 saturating and wrapping words.
import numpy as np

IHMAX = 8128
THETA_MAX = 256 * IHMAX + 64 * 255
TMIN = -24000 * 256
TMAX = 24000 * 256 + 255
RSENT = 32767 * 256
RIN_MIN = THETA_MAX - TMAX + 1
ROUT_MIN = TMIN


def sat(x):
    return np.clip(x, -32768, 32767)


def wrap(x):
    return ((x + 32768) & 0xFFFF) - 32768


def cmp2w(hs, ls, tin, tout, rin, rout):
    """hs, ls: accumulators as i16 words; tin, tout, rin, rout: full integers (split here as the code splits them)."""
    th_i, tl_i = tin >> 8, tin & 255
    th_o, tl_o = tout >> 8, tout & 255
    rh_i, rl_i = rin >> 8, rin & 255
    rh_o, rl_o = rout >> 8, rout & 255
    d = sat(sat(hs - th_i) - rh_i)
    g = wrap(wrap(ls - tl_i) - rl_i)
    vin = sat(d + (g >> 8))
    d = sat(sat(hs - th_o) + rh_o)
    g = wrap(wrap(ls + rl_o) - tl_o)
    vout = sat(d + (g >> 8))
    return vin > -1, vout < 0


rng = np.random.default_rng(1)


def edge(lo, hi, n):
    """n values in [lo, hi]: the ends, values near multiples of 256 and of 32768*256, and uniform ones."""
    a = rng.integers(lo, hi + 1, n)
    k = rng.integers(0, 4, n)
    near = (rng.integers(lo >> 8, (hi >> 8) + 1, n) << 8) + rng.integers(-2, 3, n)
    ends = np.where(rng.integers(0, 2, n) == 0, lo + rng.integers(0, 600, n), hi - rng.integers(0, 600, n))
    out = np.where(k == 0, a, np.where(k == 1, near, ends))
    return np.clip(out, lo, hi)


def run(units, n=4_000_000, tag=""):
    hs = rng.integers(0, IHMAX + 1, n)
    hs = np.where(rng.integers(0, 4, n) == 0, np.where(rng.integers(0, 2, n) == 0, 0, IHMAX) , hs)
    ls = rng.integers(0, 255 * units + 1, n)
    ls = np.where(rng.integers(0, 4, n) == 0, 255 * units - rng.integers(0, 3, n), ls)
    # the code asserts sum of the largest entries <= 256 IHMAX (id.q), so Theta never exceeds that whatever the units
    hs = np.minimum(hs, (256 * IHMAX - ls) >> 8)
    theta = 256 * hs + ls
    assert theta.max() <= 256 * IHMAX
    tin, tout = edge(TMIN, TMAX, n), edge(TMIN, TMAX, n)
    rin, rout = edge(RIN_MIN, RSENT, n), edge(ROUT_MIN, RSENT, n)
    # half the cases near the decision boundary
    h = rng.integers(0, 2, n) == 0
    d = rng.integers(-3, 4, n)
    rin = np.where(h, np.clip(theta - tin + d, RIN_MIN, RSENT), rin)
    rout = np.where(h, np.clip(tout - theta + d, ROUT_MIN, RSENT), rout)
    # a kept margin is < RSENT; RSENT itself is the sentinel
    vin, vout = cmp2w(wrap(hs), wrap(ls), tin, tout, rin, rout)
    tin_true = theta - rin >= tin
    tout_true = theta + rout < tout
    bi = np.flatnonzero(vin != tin_true)
    bo = np.flatnonzero(vout != tout_true)
    print(f"{tag} units={units}: {n} cases, IN mismatches {len(bi)}, OUT mismatches {len(bo)}")
    for name, b, good in (("IN", bi, tin_true), ("OUT", bo, tout_true)):
        if len(b):
            i = b[0]
            print(f"   first {name} mismatch: Hs={hs[i]} Ls={ls[i]} Tin={tin[i]} Tout={tout[i]} Rin={rin[i]} Rout={rout[i]} "
                  f"kernel={'yes' if (vin if name == 'IN' else vout)[i] else 'no'} exact={'yes' if good[i] else 'no'}")
    # the sentinel never decides; the clamps never decide
    s_in, s_out = cmp2w(wrap(hs), wrap(ls), tin, tout, np.full(n, RSENT), np.full(n, RSENT))
    c_in, _ = cmp2w(wrap(hs), wrap(ls), np.full(n, TMAX), tout, np.clip(rin, RIN_MIN, RSENT - 1), rout)
    _, c_out = cmp2w(wrap(hs), wrap(ls), tin, np.full(n, TMIN), rin, np.clip(rout, ROUT_MIN, RSENT - 1))
    print(f"   sentinel decides: in {int(s_in.sum())}, out {int(s_out.sum())}; Tin=TMAX takes in {int(c_in.sum())}; Tout=TMIN puts out {int(c_out.sum())}")


print("constants", dict(THETA_MAX=THETA_MAX, TMIN=TMIN, TMAX=TMAX, RSENT=RSENT, RIN_MIN=RIN_MIN, ROUT_MIN=ROUT_MIN))
run(64, tag="as built")
run(127, tag="largest safe")
run(128, tag="one more")
run(160, tag="80-byte deterministic prefix")

# exhaustive small sweep around every saturation corner: d in the i16 extremes, g in its extremes
bad = 0
for hs in (0, 1, IHMAX - 1, IHMAX):
    for ls in (0, 1, 255, 256, 16319, 16320):
        th = np.array([TMIN, TMIN + 1, TMIN + 255, TMIN + 256, -1, 0, 1, 255, 256, TMAX - 256, TMAX - 255, TMAX - 1, TMAX])
        for r_lo, r_hi, side in ((RIN_MIN, RSENT, 0), (ROUT_MIN, RSENT, 1)):
            r = np.concatenate([np.arange(r_lo, r_lo + 600), np.arange(r_hi - 600, r_hi + 1), np.arange(-600, 600)])
            T, R = np.meshgrid(th, r)
            T, R = T.ravel(), R.ravel()
            H, L = np.full(T.shape, hs), np.full(T.shape, ls)
            vin, vout = cmp2w(H, L, T, T, R, R)
            theta = 256 * hs + ls
            if side == 0:
                bad += int((vin != (theta - R >= T)).sum())
            else:
                bad += int((vout != (theta + R < T)).sum())
print("corner sweep mismatches:", bad)
