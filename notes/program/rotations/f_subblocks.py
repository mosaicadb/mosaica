"""F: a block of n positions split into k sub-blocks of b positions, each with its own Haar rotation.

A rotated sub-block is a uniform direction times its norm, so the simulation draws Gaussian sub-blocks (flat: equal
expected energy; skew: energy falling by a factor 4 from the first to the last sub-block). The code is the built
one, with one scale per block as now.

Per item:  A      the error norm, as with one rotation of the whole block (the code is the same);
           A'_i   the tangential error norm of sub-block i under one shared kappa;
           eps_i  the radial mismatch kappa*c_i - 1;  B^2 = sum eps_i^2 |y_i|^2;  A^2 = B^2 + sum A'_i^2.
Widths, in units of c*A*|Delta|/sqrt(n-1) (one rotation of the whole block):
  own scalars   sqrt(sum (|y_i| tan phi_i)^2 |Delta_i|^2/(b-1)) with a query spread evenly (the form that needs k sets of scalars)
  shared, worst (B + c * max_i A'_i/sqrt(b-1)) / (c A/sqrt(n-1))      certified for any query
  radial share  B / (c A/sqrt(n-1))                                     the deterministic term alone
"""
import numpy as np
from lib import built_code, value

rng = np.random.default_rng(31)
C = 7.8


def analyse(Z, cell, depth, b):
    m, n = Z.shape
    k = n // b
    val = value(cell, depth, depth)
    yn2 = (Z * Z).sum(1)
    dot = (val * Z).sum(1)
    kappa = yn2 / dot
    e = kappa[:, None] * val - Z
    A = np.sqrt((e * e).sum(1))
    Zs = Z.reshape(m, k, b); vs = val.reshape(m, k, b)
    y2 = (Zs * Zs).sum(2)
    dots = (vs * Zs).sum(2)
    cn = (vs * vs).sum(2)
    ci = dots / y2
    eps = kappa[:, None] * ci - 1.0
    Ap2 = kappa[:, None] ** 2 * np.maximum(cn - dots ** 2 / y2, 0.0)
    B = np.sqrt((eps ** 2 * y2).sum(1))
    assert np.allclose(A ** 2, B ** 2 + Ap2.sum(1), rtol=1e-8)
    tan_i2 = np.maximum(cn * y2 - dots ** 2, 0) / dots ** 2  # own-kappa tan^2 per sub-block
    base = C * A / np.sqrt(n - 1)
    own = np.sqrt((y2 * tan_i2).sum(1) / (k * (b - 1))) * C / base
    shared = (B + C * np.sqrt(Ap2.max(1) / (b - 1))) / base
    radial = B / base
    maxpart = C * np.sqrt(Ap2.max(1) / (b - 1)) / base
    return own.mean(), shared.mean(), radial.mean(), maxpart.mean(), (B / A).mean() * np.sqrt(b)


def equalised(Z, depth, b, tries=41):
    """Per sub-block, pick a scale near the block's so that its gain c_i is closest to the block's mean gain (one rise).
    Returns the cells. Not possible at depth 1, where the code is the sign and does not depend on the scale."""
    from lib import cells
    m, n = Z.shape
    k = n // b
    base_cell = built_code(Z, [depth])
    val = value(base_cell, depth, depth)
    target = ((val * Z).sum(1) / (Z * Z).sum(1))  # 1/kappa of the block
    # recover the block's scale by re-running the search (cheap: six candidates)
    from lib import LAMS
    yn2 = (Z * Z).sum(1); rms = np.sqrt(yn2 / n)
    lam = np.zeros(m)
    for L in LAMS:
        c = cells(Z / (L * rms[:, None]), depth)
        hit = (c == base_cell).all(1) & (lam == 0)
        lam[hit] = L
    lam[lam == 0] = 2.6
    out =base_cell.copy().reshape(m, k, b)
    Zs = Z.reshape(m, k, b)
    best = np.full((m, k), np.inf)
    for f in np.linspace(0.85, 1.15, tries):
        c = cells(Zs / (f * lam * rms)[:, None, None], depth)
        v = value(c, depth, depth)
        ci = (v * Zs).sum(2) / (Zs * Zs).sum(2)
        # the decoder reads grid values with one unit for the block, so the gain is ci as it stands
        miss = np.abs(ci / target[:, None] - 1.0)
        upd = miss < best
        best[upd] = miss[upd]
        out[upd] = c[upd]
    return out.reshape(m, n)


if __name__ == "__main__":
    m = 400
    print(f"c = {C}; {m} items per row; widths as multiples of the one-rotation half-width")
    for n, bs in ((256, (128, 64, 32)), (768, (384, 256, 128, 64)), (688, (344, 172, 86))):
        for profile in ("flat", "skew"):
            for depth in (1, 2, 4):
                Zg = rng.standard_normal((m, n))
                cell = built_code(Zg, [depth]) if profile == "flat" else None
                for b in bs:
                    k = n // b
                    Z = Zg
                    if profile == "skew":
                        w = 4.0 ** (-np.arange(k) / max(k - 1, 1) / 2)  # energy ratio 4 from first to last
                        Z = (Zg.reshape(m, k, b) * w[None, :, None]).reshape(m, n)
                        cell = built_code(Z, [depth])
                    own, shared, radial, maxpart, bn = analyse(Z, cell, depth, b)
                    line = (f"n = {n} {profile} depth {depth} b = {b:3d} k = {k:2d}: own scalars {own:.3f}; shared worst {shared:.3f} "
                            f"(radial {radial:.3f} + max {maxpart:.3f}); B*sqrt(b)/A = {bn:.2f}; sqrt(k)/c = {np.sqrt(k)/C:.3f}")
                    if depth > 1 and profile == "flat":
                        ce = equalised(Z, depth, b)
                        own2, shared2, radial2, maxpart2, bn2 = analyse(Z, ce, depth, b)
                        line += f"; gains equalised by the encoder: shared worst {shared2:.3f} (radial {radial2:.3f} + max {maxpart2:.3f})"
                    print(line)
        print()
