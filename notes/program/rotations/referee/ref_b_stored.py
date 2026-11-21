"""Do the B4 lower bounds survive the built index's rounding of the stored scalars? tan(phi) is stored as the
smallest of 15 table values at or above it (the table runs between the 0.2% and 99.8% quantiles of the data's
tan(phi)), or as an escape rounded up by at most 2^(1/20). Model: typical data are Gaussian blocks of n positions at
depth 1; the structured item's stored tan is max(tan, table minimum) * 2^(1/20) (the worst case for the attack)."""
import numpy as np

rng = np.random.default_rng(99)


def hadamard(n):
    k = np.arange(n)
    x = k[:, None] & k[None, :]
    pc = np.zeros((n, n), dtype=np.int64)
    while x.any():
        pc += x & 1
        x >>= 1
    return (1.0 - 2.0 * (pc % 2)) / np.sqrt(n)


def tan_gauss(n, m=20000):
    z = rng.standard_normal((m, n))
    cos = np.abs(z).sum(1) / (np.sqrt(n) * np.linalg.norm(z, axis=1))
    return np.sqrt(1 - cos ** 2) / cos


def xt(t, total):
    Ht = hadamard(t)
    chi = 1.0 - 2.0 * (np.arange(t) & 1)
    U = []; N = []; T = []
    for _ in range(total // 100000):
        eps = rng.choice((-1.0, 1.0), (100000, t))
        zeta = np.round((eps @ Ht) * np.sqrt(t)) / t
        xi = ((eps * chi) @ Ht) / np.sqrt(t)
        o = np.where(zeta >= 0, 0.5, -0.5)
        kappa = (zeta * zeta).sum(1) / (o * zeta).sum(1)
        e = kappa[:, None] * o - zeta
        A = np.linalg.norm(e, axis=1)  # |y| = 1, so A = tan(phi)
        U.append((e * xi).sum(1)); N.append((np.where(zeta != 0, e, 0.0) * xi).sum(1)); T.append(A)
    return np.concatenate(U), np.concatenate(N), np.concatenate(T)


cs = {256: 8.282, 512: 8.589, 1024: 8.748}
for (n, t, k) in [(256, 64, 2), (256, 16, 3), (256, 32, 3), (256, 32, 4), (512, 64, 2), (512, 32, 3), (1024, 64, 2)]:
    tg = tan_gauss(n)
    tmin = np.quantile(tg, 0.002)
    V = n // t
    lp = ((1 + np.log2(V) - V) + (1 - V)) * (k // 2) + (1 + np.log2(t) - t) * ((k - 1) // 2)
    up, no, tan = xt(t, 4_000_000)
    c = cs[n]
    line = f"n={n} t={t} k={k}: typical tan at depth 1: 0.2% quantile {tmin:.3f}, median {np.median(tg):.3f}; structured item's tan: 10/50/90% {np.quantile(tan,0.1):.3f}/{np.median(tan):.3f}/{np.quantile(tan,0.9):.3f};"
    for label, stored in (("exact tan", tan), ("escape rounding only", tan * 2 ** 0.05), ("floor at table minimum and escape rounding", np.maximum(tan, tmin) * 2 ** 0.05)):
        h = c * stored / np.sqrt(n - 1)
        pu = np.mean(np.abs(up) > h); pn = np.mean(np.abs(no) > h)
        mix = (t / n) * pu + (1 - t / n) * pn
        line += f" {label}: 2^{lp + (np.log2(mix) if mix > 0 else -np.inf):.1f};"
    fail = np.abs(no) > c * tan / np.sqrt(n - 1)
    line += f" tan among failing draws: median {np.median(tan[fail]) if fail.any() else float('nan'):.3f}"
    print(line)
