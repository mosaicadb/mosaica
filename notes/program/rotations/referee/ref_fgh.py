"""Referee spot checks for D2 (sign), F2 (R4 constants), F3/F5 (one row each), G5a (constants)."""
import numpy as np
from scipy import special, optimize, stats

rng = np.random.default_rng(31)
LAMS = (2.0, 2.3, 2.6, 3.0, 3.5, 4.2)


def code(Z, depth):
    m, n = Z.shape
    half = 2.0 ** (depth - 1)
    yn2 = (Z * Z).sum(1); rms = np.sqrt(yn2 / n)
    best = np.full(m, np.inf); out = np.zeros_like(Z)
    for lam in LAMS:
        cell = np.clip(np.floor(Z / (lam * rms[:, None]) * half) + half, 0, 2 * half - 1)
        val = (cell + 0.5) / half - 1.0
        dot = (val * Z).sum(1); cn = (val * val).sum(1)
        obj = np.log(np.maximum(cn * yn2 - dot * dot, 1e-300) / np.maximum(dot * dot, 1e-300))
        upd = obj < best; best[upd] = obj[upd]; out[upd] = val[upd]
    return out


print("== D2: is the sign of the error a function of z? butterfly n = 16, y = e0, Delta = e1, depth 1 ==")
n = 16; m = 20000
Z = np.zeros((m, n)); Z[:, 0] = 1; W = np.zeros((m, n)); W[:, 1] = 1
l = 0; idx = np.arange(n)
while (1 << l) < n:
    lo = idx[(idx >> l) & 1 == 0]; hi = lo | (1 << l)
    th = rng.uniform(0, 2 * np.pi, (m, lo.size)); c, s = np.cos(th), np.sin(th)
    for X in (Z, W):
        a, b = X[:, lo].copy(), X[:, hi].copy()
        X[:, lo] = c * a - s * b; X[:, hi] = s * a + c * b
    l += 1
O = code(Z, 1); kappa = 1.0 / (O * Z).sum(1)
e = kappa[:, None] * O - Z
err = (e * W).sum(1)
odd = idx % 2 == 1
so = np.linalg.norm(Z[:, odd], axis=1); se = np.linalg.norm(Z[:, ~odd], axis=1)
pred = (kappa * (O[:, odd] * Z[:, odd]).sum(1) - so ** 2) / (so * se)
print(f" | |err| - |formula| | max {np.abs(np.abs(err) - np.abs(pred)).max():.1e}; share of draws with err = +formula {np.mean(np.sign(err) == np.sign(pred)):.3f}, = -formula {np.mean(np.sign(err) == -np.sign(pred)):.3f}")

print()
print("== F2: R4 (Theorem 3.21) constants, one-sided, delta = eps/(N L), eps = 1e-9, L = 4 ==")


def sphere_tail(tau, d):
    return 0.5 * special.betainc((d - 1) / 2, 0.5, 1 - tau * tau)


def sphere_c(d, delta):
    return optimize.brentq(lambda c: sphere_tail(c / np.sqrt(d), d) - delta, 1.0, np.sqrt(d) * 0.999, xtol=1e-12)


def r4(ds, delta):
    ds = np.asarray(ds, dtype=float); D = ds.sum()
    lognu = special.gammaln(ds / 2 + 0.5) + special.gammaln(D / 2) - special.gammaln(ds / 2) - special.gammaln(D / 2 + 0.5)
    theta = np.sqrt(np.max(ds / (D * np.exp(2 * lognu))))
    s = theta * np.sqrt(D)  # in units of sd
    logC = special.gammaln(D / 2) - 0.5 * np.log(np.pi) - special.gammaln((D - 1) / 2)

    def bound(c):
        def f(tp):  # tp in units of sd, tp < c
            tau = tp / s
            ex = np.exp(logC + (D - 1) / 2 * np.log1p(-tau * tau)) / (D - 1) - tau * sphere_tail(tau, D)
            return s * ex / (c - tp)
        r = optimize.minimize_scalar(f, bounds=(c - 3.0, c - 1e-3), method="bounded", options={"xatol": 1e-10})
        return r.fun
    return optimize.brentq(lambda c: np.log(bound(c)) - np.log(delta), 5.0, 12.0, xtol=1e-6), theta


for N, name in ((1e9, "N = 1e9"), (2.0 ** 20, "N = 2^20")):
    delta = 1e-9 / (N * 4)
    c768, th = r4([95, 127, 255, 287], delta)
    line = f" {name}: n = 768 four blocks {c768:.3f} (theta {th:.5f});"
    for label, ds in (("n = 2048 four blocks", [255, 335, 687, 767]), ("each halved", [127] * 2 + [167] * 2 + [343] * 2 + [383] * 2),
                      ("32 x 64", [63] * 32), ("128 x 16", [15] * 128)):
        cc, th = r4(ds, delta)
        dprime = sum(d + 2 for d in ds) - 2
        line += f" {label}: R4 {cc:.3f}, sphere quantile in {dprime} dims {sphere_c(dprime, delta):.3f};"
    print(line)

print()
print("== F3 / F5: one kappa for the block, radial term; half-width over the one-rotation half-width, c = 7.8 ==")
C = 7.8
for (n, k) in ((256, 2), (256, 4), (768, 2), (768, 3)):
    b = n // k
    out = []
    for depth in (1, 2, 4):
        Z = rng.standard_normal((400, n))
        O = code(Z, depth)
        yn2 = (Z * Z).sum(1); dot = (O * Z).sum(1); kappa = yn2 / dot
        A = np.linalg.norm(kappa[:, None] * O - Z, axis=1)
        B2 = 0.0; M = 0.0; own2 = 0.0
        for i in range(k):
            zi = Z[:, i * b:(i + 1) * b]; oi = O[:, i * b:(i + 1) * b]
            ci = (oi * zi).sum(1) / (zi * zi).sum(1)
            eps = kappa * ci - 1
            cosf = (oi * zi).sum(1) / (np.linalg.norm(oi, axis=1) * np.linalg.norm(zi, axis=1))
            Ap = kappa * np.linalg.norm(oi, axis=1) * np.sqrt(1 - cosf ** 2)
            B2 = B2 + eps ** 2 * (zi * zi).sum(1)
            M = np.maximum(M, Ap / np.sqrt(b - 1))
            own2 = np.maximum(own2, np.linalg.norm(zi, axis=1) * np.sqrt(1 - cosf ** 2) / cosf / np.sqrt(b - 1))
        one = C * A / np.sqrt(n - 1)
        out.append((np.mean((np.sqrt(B2) + C * M) / one), np.mean(np.sqrt(B2) / one), np.mean(C * own2 / one)))
    print(f" {n} = {k} x {b}: F3 width ratio at depths 1/2/4: " + " / ".join(f"{o[0]:.2f}" for o in out)
          + "; radial term alone: " + " / ".join(f"{o[1]:.2f}" for o in out)
          + "; F2 worst-query width (max over sub-blocks): " + " / ".join(f"{o[2]:.2f}" for o in out))

print()
print("== G5a: constants at N = 1e9 (delta = 2.5e-19), N = 2^20, N = 2^16 ==")
for N in (1e9, 2.0 ** 20, 2.0 ** 16):
    delta = 1e-9 / (N * 4)
    zg = stats.norm.isf(delta)
    line = f" N = {N:.3g}: sub-Gaussian {np.sqrt(2 * np.log(1 / delta)):.3f}; Gaussian {zg:.3f}; Gaussian at delta/3.178 {stats.norm.isf(delta / 3.178):.3f}; sphere d = 255/767/2046 " \
           + "/".join(f"{sphere_c(d, delta):.3f}" for d in (255, 767, 2046)) + "; uniforms equal weights (Lugannani-Rice) m = 64/256/768/2048 "
    vals = []
    for mm in (64, 256, 768, 2048):
        sd = np.sqrt(mm / 3)
        K = lambda lam: mm * (np.log(np.sinh(lam) / lam))
        K1 = lambda lam: mm * (1 / np.tanh(lam) - 1 / lam)
        K2 = lambda lam: mm * (1 / lam ** 2 - 1 / np.sinh(lam) ** 2)

        def tail(x):
            lam = optimize.brentq(lambda l_: K1(l_) - x, 1e-9, 500.0, xtol=1e-14)
            r = np.sqrt(2 * (lam * x - K(lam))); u = lam * np.sqrt(K2(lam))
            return stats.norm.sf(r) + stats.norm.pdf(r) * (1 / u - 1 / r)
        cq = optimize.brentq(lambda c: np.log(tail(c * sd)) - np.log(delta), 3.0, min(12.0, np.sqrt(3 * mm) * 0.999), xtol=1e-9)
        vals.append(cq)
    print(line + "/".join(f"{v:.3f}" for v in vals))
