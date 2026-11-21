"""G1 to G3 checked directly: the nested dithered code on a fine grid of dither values.

(1) For several fixed values y and first depths d1, the error at every depth d >= d1 is uniform on its cell and the
    cell index never clamps when |y| <= s (1 - 2^-d1).  (2) With overload the error's mean is the overshoot.
(3) The example of G3 (ii): choosing between s = 2a and s' = 4a by the larger normalised error doubles the tail.
(4) A weighted sum over 64 positions with mixed depths: variance s^2 sum Delta^2 4^-d / 3 for two very different items.
"""
import numpy as np

M = 1 << 20
tau = (np.arange(M) + 0.5) / M  # a fine, evenly spaced dither


def code(y, s, d1, D, tau):
    """Returns the depth-D cell index of y + u and u, with u = w_{d1} (tau - 1/2)."""
    w1 = 2 * s * 2.0 ** -d1
    u = w1 * (tau - 0.5)
    wD = 2 * s * 2.0 ** -D
    k_raw = np.floor((y + u + s) / wD)
    return np.clip(k_raw, 0, 2 ** D - 1), u, (k_raw < 0) | (k_raw > 2 ** D - 1)


def err(y, s, d1, D, d, tau):
    k, u, clamped = code(y, s, d1, D, tau)
    wd = 2 * s * 2.0 ** -d
    m = -s + (np.floor(k / 2 ** (D - d)) + 0.5) * wd
    return m - u - y, wd, clamped


print("(1) no overload: largest deviation of the error's distribution from uniform on its cell (Kolmogorov distance), and clamps")
s = 1.0
for d1 in (1, 2, 3):
    lim = s * (1 - 2.0 ** -d1)
    for y in (0.0, 0.123456, -0.3, lim, -lim, 0.999 * lim):
        row = []
        for d in range(d1, 6):
            e, wd, cl = err(y, s, d1, 5, d, tau)
            x = np.sort(e / wd + 0.5)  # should be uniform on [0, 1]
            ks = np.max(np.abs(x - (np.arange(M) + 0.5) / M))
            row.append(f"{ks:.1e}")
            assert not cl.any()
        print(f"   d1 = {d1}, y = {y:+.4f}: depths {d1}..5: " + " ".join(row))

print("(2) overload, d1 = 1, depth 3: mean error against the overshoot")
for y in (0.6, 0.8, 1.0, 1.6):
    e, wd, cl = err(y, s, 1, 3, 3, tau)
    print(f"   y = {y}: share of draws clamped {cl.mean():.3f}, mean error {e.mean():+.4f}, sd {e.std():.4f} (uniform sd {wd/np.sqrt(12):.4f})")

print("(3) G3 (ii): a = 1, first depth 1, read at depth 2, candidates s = 2 and s' = 4, keep the larger error/w")
a = 1.0
e1, w1, _ = err(a, 2.0, 1, 2, 2, tau)
e2, w2, _ = err(a, 4.0, 1, 2, 2, tau)
mx = np.maximum(e1 / w1, e2 / w2)
for x in (0.1, 0.25, 0.4, 0.49):
    print(f"   Pr[eps/w >= {x}]: fixed step {np.mean(e1/w1 >= x):.4f} (law {0.5-x:.4f}); chosen step {np.mean(mx >= x):.4f} (1 - 2x = {1-2*x:.4f})")

print("(4) 64 positions, depths 1 to 3 in thirds, independent dithers, 200,000 draws: variance of the level's error over s^2 sum Delta^2 4^-d / 3")
rng = np.random.default_rng(3)
n = 64
depth = np.r_[np.full(22, 3), np.full(21, 2), np.full(21, 1)]
Dl = rng.standard_normal(n)
for name, y in (("a Gaussian item", rng.standard_normal(n)), ("an item with one large coordinate", np.r_[5.0, 0.01 * rng.standard_normal(n - 1)]),
                ("the zero item", np.zeros(n))):
    s_item = max(np.abs(y).max() / (1 - 0.5), 1e-3)  # every position is first coded at depth 1 here
    T = rng.random((200_000, n))
    tot = np.zeros(200_000)
    for p in range(n):
        e, wd, cl = err(y[p], s_item, 1, 3, depth[p], T[:, p])
        assert not cl.any()
        tot += Dl[p] * e
    var = s_item ** 2 * np.sum(Dl ** 2 * 4.0 ** -depth) / 3
    z = tot / np.sqrt(var)
    print(f"   {name}: variance ratio {np.mean(z**2):.4f}, kurtosis {np.mean(z**4)/np.mean(z**2)**2:.3f}, Pr[|z| > 3] {np.mean(np.abs(z) > 3):.2e}")
