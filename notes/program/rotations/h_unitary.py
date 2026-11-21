"""H4: Lemma 3.1 for Haar measure on U(n/2) with a complex rescaling, checked by Monte Carlo on a sparse pair,
and the size of the deterministic term left by the present real rescaling.
"""
import numpy as np
from lib import built_code, value

rng = np.random.default_rng(13)
C = 7.8


def haar_unitary(m, rng):
    g = (rng.standard_normal((m, m)) + 1j * rng.standard_normal((m, m))) / np.sqrt(2)
    q, r = np.linalg.qr(g)
    d = np.diag(r)
    return q * (d / np.abs(d))


def run(m, depth, draws, y, Dl, label):
    n = 2 * m
    rs = []; rr = []; share = []
    for _ in range(draws):
        U = haar_unitary(m, rng)
        z = U @ y; w = U @ Dl
        zr = np.r_[z.real, z.imag][None, :]
        cell = built_code(zr, [depth])
        v = value(cell, depth, depth)[0]
        o = v[:m] + 1j * v[m:]
        y2 = np.vdot(y, y).real
        herm = np.sum(o * np.conj(z))            # <o, z> = sum o conj(z)
        kc = y2 / herm                           # complex scale
        e = kc * o - z
        A = np.linalg.norm(e)
        P = Dl - (np.sum(Dl * np.conj(y)) / y2) * y   # off the complex span of y
        err = np.sum(w * np.conj(e)).real
        rs.append(err * np.sqrt(n - 2) / (A * np.linalg.norm(P)))
        kr = y2 / herm.real                      # the present real scale
        er = kr * o - z
        Ar = np.linalg.norm(er)
        Pr = Dl - (np.sum(Dl * np.conj(y)).real / y2) * y
        errr = np.sum(w * np.conj(er)).real
        rr.append(errr * np.sqrt(n - 1) / (Ar * np.linalg.norm(Pr)))
        beta = np.sum(er * np.conj(1j * z)).real / y2
        share.append(abs(beta) * np.sqrt(y2) * np.sqrt(n - 2) / (C * Ar))
    rs = np.array(rs); rr = np.array(rr)
    kurt = lambda x: (x ** 4).mean() / (x ** 2).mean() ** 2
    print(f"{label}, n = {n}, depth {depth}, {draws} draws")
    print(f"   complex scale: mean square {np.mean(rs**2):.3f} (law 1), kurtosis {kurt(rs):.3f} (law {3 - 6/n:.3f}), "
          f"Pr[|.| > 3] {np.mean(np.abs(rs) > 3):.2e}, max {np.abs(rs).max():.2f}")
    print(f"   real scale, read against the real law: mean square {np.mean(rr**2):.3f}, max {np.abs(rr).max():.2f}; "
          f"allowance |beta| |y| |Delta| as a share of c*sd: mean {np.mean(share):.3f}, 90% {np.quantile(share, 0.9):.3f}")


if __name__ == "__main__":
    m = 32
    y = np.zeros(m, complex); y[0] = 1; y[1] = 1
    Dl = np.zeros(m, complex); Dl[0] = 1; Dl[1] = -1
    run(m, 1, 20000, y, Dl, "y = e0 + e1, Delta = e0 - e1")
    Dl2 = np.zeros(m, complex); Dl2[0] = 1j; Dl2[1] = 0.3
    run(m, 2, 20000, y, Dl2, "y = e0 + e1, Delta = i e0 + 0.3 e1 (has a component along Iy)")
    g = np.random.default_rng(1)
    y = g.standard_normal(m) + 1j * g.standard_normal(m); Dl = g.standard_normal(m) + 1j * g.standard_normal(m)
    run(m, 2, 20000, y, Dl, "fixed Gaussian pair")
