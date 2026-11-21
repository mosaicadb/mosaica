"""G: shared subtractive dither with a nested uniform code and a per-item step, against the code as built.

Part 1: the confidence constant for a weighted sum of independent uniforms, against the sphere's.
Part 2: the spread per unit |Delta| against the built code at 1 to 4 bits per coordinate, and at sub-bit rates.
"""
import sys
import numpy as np
import mpmath as mp
from scipy import optimize, special, stats
from lib import built_code, estimate_error

rng = np.random.default_rng(41)

# ---------------- part 1: constants ----------------

def sphere_c(d, lnd):
    """c with Pr[sqrt(d) U >= c] = exp(-lnd), U a coordinate of a uniform unit vector in R^d (one-sided)."""
    f = lambda c: float(mp.log(mp.betainc((d - 1) / 2.0, 0.5, 0, 1 - c * c / d, regularized=True) / 2)) + lnd
    return optimize.brentq(f, 1.0, np.sqrt(d) * 0.999999)


def gauss_c(lnd):
    return float(stats.norm.isf(np.exp(-lnd)))


def irwin_hall_tail(m, c, dps=None):
    """Pr[sum of m independent uniforms on (-1,1) >= c*sqrt(m/3)], exactly."""
    mp.mp.dps = dps or (60 + m)
    a = (mp.mpf(m) - mp.mpf(c) * mp.sqrt(mp.mpf(m) / 3)) / 2
    if a <= 0:
        return mp.mpf(0)
    s = mp.mpf(0)
    for k in range(int(mp.floor(a)) + 1):
        s += (-1) ** k * mp.binomial(m, k) * (a - k) ** m
    return s / mp.factorial(m)


def irwin_hall_c(m, lnd):
    f = lambda c: float(mp.log(irwin_hall_tail(m, c))) + lnd
    hi = np.sqrt(3 * m) * 0.9999
    if f(hi) > 0:
        return float("nan")
    return optimize.brentq(f, 1.0, hi, xtol=1e-6)


def lr_tail(w, x):
    """Lugannani-Rice approximation of Pr[sum w_p V_p >= x], V_p uniform on (-1,1)."""
    w = np.asarray(w, float)
    def K(l):
        t = l * w
        return np.sum(t + np.log(-np.expm1(-2 * t) / (2 * t))) if l != 0 else 0.0
    def K1(l):
        t = l * w
        return np.sum(w * (1 / np.tanh(t) - 1 / t))
    def K2(l):
        t = l * w
        with np.errstate(over="ignore"):
            return np.sum(w * w * (1 / t ** 2 - 1 / np.sinh(t) ** 2))
    if x >= np.sum(np.abs(w)):
        return 0.0
    lo, hi = 1e-9, 1.0
    while K1(hi) < x:
        hi *= 2
        if hi * w.max() > 600:
            break
    l = optimize.brentq(lambda l: K1(l) - x, lo, hi)
    r = np.sqrt(2 * (l * x - K(l)))
    q = l * np.sqrt(K2(l))
    return float(stats.norm.sf(r) + stats.norm.pdf(r) * (1 / q - 1 / r))


def lr_c(w, lnd):
    w = np.asarray(w, float)
    sd = np.sqrt(np.sum(w * w) / 3)
    f = lambda c: np.log(max(lr_tail(w, c * sd), 1e-300)) + lnd
    hi = min(np.sum(np.abs(w)) / sd * 0.98, 12.0)
    if not np.isfinite(f(hi)) or f(hi) > 0:
        return float("nan")
    return optimize.brentq(f, 1.0, hi, xtol=1e-6)


def chernoff_flat_c(m, lnd):
    """Chernoff with the exact moment generating function, flat weights on m coordinates (the worst case by G5)."""
    def bound(c):
        x = c * np.sqrt(m / 3.0)
        g = lambda l: -l * x + m * (l + np.log(-np.expm1(-2 * l) / (2 * l)))
        res = optimize.minimize_scalar(g, bounds=(1e-6, 50), method="bounded")
        return res.fun
    return optimize.brentq(lambda c: bound(c) + lnd, 1.0, np.sqrt(3 * m) * 0.999)


def part1():
    print("Part 1: one-sided constants in units of sd")
    for label, lnd in (("N = 1e9, L = 4, eps = 1e-9: ln(1/delta) = 42.83", np.log(4e18)),
                       ("N = 2^20: ln(1/delta) = 35.97", 35.97), ("N = 2^16: ln(1/delta) = 33.20", np.log(2 ** 16 * 4 / 1e-9))):
        print(f" {label}")
        print(f"   check against chapter 3: sphere d = 95: {sphere_c(95, lnd):.3f} (7.373 at N = 1e9)")
        print("   sphere, one coordinate, d = 15 / 63 / 255 / 767 / 2046: " + " / ".join(f"{sphere_c(d, lnd):.3f}" for d in (15, 63, 255, 767, 2046)))
        zg = gauss_c(lnd)
        zbd = gauss_c(lnd + np.log(3.178))
        print(f"   Gaussian quantile {zg:.3f}; sub-Gaussian sqrt(2 ln(1/delta)) {np.sqrt(2*lnd):.3f}; Gaussian at delta/3.178 {zbd:.3f}")
        print("   uniform sum, flat on m coordinates, exact (Irwin-Hall): " + ", ".join(f"m={m}: {irwin_hall_c(m, lnd):.3f}" for m in (32, 64, 128, 256)))
        print("   same by Lugannani-Rice: " + ", ".join(f"m={m}: {lr_c(np.ones(m), lnd):.3f}" for m in (32, 64, 128, 256, 768, 2048)))
        print("   Chernoff with the exact mgf, flat on m: " + ", ".join(f"m={m}: {chernoff_flat_c(m, lnd):.3f}" for m in (64, 256, 768, 2048)))
        sys.stdout.flush()
    lnd = np.log(4e18)
    print(" is flat the worst case? Lugannani-Rice at N = 1e9, weights on 256 coordinates:")
    g = np.random.default_rng(2)
    cases = {"flat 256": np.ones(256), "flat 128 (rest zero)": np.r_[np.ones(128), np.zeros(128) + 1e-9],
             "two levels 1 and 0.5, half each": np.r_[np.ones(128), 0.5 * np.ones(128)],
             "two levels 1 and 0.2": np.r_[np.ones(128), 0.2 * np.ones(128)],
             "32 at 1, 224 at 0.3": np.r_[np.ones(32), 0.3 * np.ones(224)],
             "|Gaussian| weights": np.abs(g.standard_normal(256)) + 1e-6,
             "geometric, ratio 0.98": 0.98 ** np.arange(256)}
    for name, w in cases.items():
        print(f"   {name}: c = {lr_c(w, lnd):.3f}")


# ---------------- part 2: widths ----------------

def built_tans(n, m=300):
    Z = rng.standard_normal((m, n))
    single = {}; nested = {}
    for B in (1, 2, 3, 4, 5, 6):
        cell = built_code(Z, [B])
        single[B] = np.sqrt((estimate_error(Z, cell, B, B)[3] ** 2).mean())
        cell = built_code(Z, list(range(1, B + 1)))
        nested[B] = (np.sqrt((estimate_error(Z, cell, B, B)[3] ** 2).mean()), np.sqrt((estimate_error(Z, cell, B, 1)[3] ** 2).mean()))
    crest = np.abs(Z).max(1) / np.sqrt((Z * Z).mean(1))
    return single, nested, crest, Z


def part2():
    print()
    print("Part 2: spread of the estimate per unit |Delta|, dither / built code (both in units of the block's rms)")
    print("  built: |y| tan(phi)/sqrt(n-1) = rms * tan(phi) * sqrt(n/(n-1));  dither: w_B/sqrt(12), w_B = 2 s 2^-B, s = y_max/(1 - 2^-d1)")
    for n in (64, 256, 768):
        single, nested, crest, Z = built_tans(n)
        print(f" n = {n}: crest factor y_max/rms of a rotated (Gaussian) block: mean {crest.mean():.2f}, 10% to 90%: {np.quantile(crest, 0.1):.2f} to {np.quantile(crest, 0.9):.2f}")
        print("   built tan(phi), one rise at B = 1..6: " + " ".join(f"{single[B]:.4f}" for B in range(1, 7)))
        print("   built tan(phi) at depth B, code nested over rises 1..B: " + " ".join(f"{nested[B][0]:.4f}" for B in range(1, 7)))
        for d1 in (1, 2, 3):
            row = []
            for B in range(max(d1, 1), 5):
                coef = (2.0 / (1 - 2.0 ** -d1)) * 2.0 ** -B / np.sqrt(12) / (single[B] * np.sqrt(n / (n - 1)))
                row.append(f"B={B}: {coef:.3f} x crest = {coef*crest.mean():.2f}")
            print(f"   first coded depth d1 = {d1}: ratio of spreads  " + ";  ".join(row))
        row = []
        for B in range(1, 5):
            coef = (2.0 / (1 - 2.0 ** -B)) * 2.0 ** -B / np.sqrt(12) / (single[B] * np.sqrt(n / (n - 1)))
            row.append(f"B={B}: {coef:.3f} x crest = {coef*crest.mean():.2f}")
        print("   not nested (d1 = B):                        " + ";  ".join(row))
        # clipping with a deterministic remainder, c = 7.8, d1 = 1
        c = 7.8
        rms = np.sqrt((Z * Z).mean(1)); yn = np.sqrt((Z * Z).sum(1))
        row = []
        for B in (1, 2, 4):
            best = None
            for t in np.linspace(1.0, 4.5, 36):
                L = np.minimum(t * rms, np.abs(Z).max(1))
                r = np.sqrt((np.maximum(np.abs(Z) - L[:, None], 0) ** 2).sum(1))
                h = c * (4 * L * 2.0 ** -B) / np.sqrt(12) + r
                ratio = (h / (c * yn * single[B] / np.sqrt(n - 1))).mean()
                if best is None or ratio < best[0]:
                    best = (ratio, t)
            row.append(f"B={B}: {best[0]:.2f} at clip {best[1]:.1f} rms")
        print("   dither with clipping and a Cauchy-Schwarz remainder, best clip, half-width ratio at c = 7.8 (d1 = 1): " + "; ".join(row))
        # sub-bit: a share f of the positions coded at one bit, the rest uncoded
        row = []
        for frac in (0.5, 0.25):
            kk = int(n * frac)
            cosf = np.abs(Z[:, :kk]).sum(1) / (np.sqrt(kk) * yn)
            tan = np.sqrt(1 - cosf ** 2) / cosf
            built = c * yn * tan / np.sqrt(n - 1)
            ymax = np.abs(Z[:, :kk]).max(1)
            yu = np.sqrt((Z[:, kk:] ** 2).sum(1))
            # query spread evenly over the block: |Delta_coded|^2 = frac, |Delta_uncoded|^2 = 1 - frac
            dith = c * (2 * ymax) / np.sqrt(12) * np.sqrt(frac) + yu * np.sqrt(1 - frac)
            row.append(f"{frac} bit: built tan {np.sqrt((tan**2).mean()):.3f}, half-width ratio {np.mean(dith/built):.2f} (of which uncoded part {np.mean(yu*np.sqrt(1-frac)/built):.2f})")
        print("   sub-bit, query spread evenly, c = 7.8: " + "; ".join(row))
    print(" crest factor of a Gaussian block by size (mean of max|g|/rms, 2000 draws): "
          + ", ".join(f"n={n}: {np.mean(np.abs(g).max(1)/np.sqrt((g*g).mean(1))):.2f}" for n in (16, 32, 64, 128, 256, 768) for g in [rng.standard_normal((2000, n))]))


if __name__ == "__main__":
    which = sys.argv[1] if len(sys.argv) > 1 else "12"
    if "1" in which:
        part1()
    if "2" in which:
        part2()
