"""The randomised code as built (rust/src/v2/mod.rs: rand_encode, rand_choose), in numpy, and helpers.

Small jobs only. Run through ../run-limited.sh 6 1 2 python3 <script>.
"""
import numpy as np

LAMS = (2.0, 2.3, 2.6, 3.0, 3.5, 4.2)


def fwht(x):
    """Normalised Walsh-Hadamard transform along the last axis (Sylvester order, H[k,i] = (-1)^popcount(k&i)/sqrt(n))."""
    x = np.array(x, dtype=np.float64)
    n = x.shape[-1]
    lead = x.shape[:-1]
    h = 1
    while h < n:
        x = x.reshape(lead + (n // (2 * h), 2, h))
        a = x[..., 0, :] + x[..., 1, :]
        b = x[..., 0, :] - x[..., 1, :]
        x = np.stack([a, b], axis=-2).reshape(lead + (n,))
        h *= 2
    return x / np.sqrt(n)


def cells(v, depth):
    """rand_encode: the cell index of v on the grid of step 2^(1-depth) over [-1, 1), clamped."""
    half = 2.0 ** (depth - 1)
    return np.clip(np.floor(v * half) + half, 0, 2 * half - 1)


def value(cell, maxd, d):
    """Decoded value of the first d bits of a depth-maxd cell index: the midpoint of the depth-d cell."""
    cd = np.floor(cell / 2.0 ** (maxd - d))
    return (cd + 0.5) / 2.0 ** (d - 1) - 1.0


def built_code(Z, rises, lams=LAMS):
    """rand_choose for a batch Z (m, n) with uniform depth per rise. Returns the depth-max cell indices.

    The scale lam*rms is chosen per row from lams to minimise the sum over rises of ln tan^2; first minimum wins.
    """
    Z = np.atleast_2d(Z)
    m, n = Z.shape
    yn2 = (Z * Z).sum(1)
    rms = np.maximum(np.sqrt(yn2 / n), 1e-300)
    maxd = max(rises)
    best = np.full(m, np.inf)
    out = np.zeros((m, n))
    for lam in lams:
        c = cells(Z / (lam * rms[:, None]), maxd)
        obj = np.zeros(m)
        for d in rises:
            val = value(c, maxd, d)
            dot = (val * Z).sum(1)
            cn = (val * val).sum(1)
            obj += np.log(np.maximum(cn * yn2 - dot * dot, 1e-300) / np.maximum(dot * dot, 1e-300))
        upd = obj < best
        best[upd] = obj[upd]
        out[upd] = c[upd]
    return out


def estimate_error(Z, cell, maxd, d):
    """For rotated blocks Z and their code at rise depth d: kappa, the error e = kappa*o - z in rotated
    coordinates, A = |e|, tan(phi)."""
    val = value(cell, maxd, d)
    yn2 = (Z * Z).sum(1)
    dot = (val * Z).sum(1)
    kappa = yn2 / dot
    e = kappa[:, None] * val - Z
    A = np.sqrt((e * e).sum(1))
    return kappa, e, A, A / np.sqrt(yn2)


def haar(n, rng):
    """A Haar orthogonal matrix: QR of a Gaussian matrix with the sign fix."""
    q, r = np.linalg.qr(rng.standard_normal((n, n)))
    return q * np.sign(np.diag(r))
