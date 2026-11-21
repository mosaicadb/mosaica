# Shared helpers for the codes track. numpy only.
# Run every script as:
#   nice -n 10 taskset -c 4,5 env OMP_NUM_THREADS=2 OPENBLAS_NUM_THREADS=2 python3 <script>
import math
import numpy as np

LAMS = [2.0, 2.3, 2.6, 3.0, 3.5, 4.2]  # rust/src/v2/mod.rs:226
BLOCKS = [96, 128, 256, 288]           # n = 768, BLOCK_FRACS of rust/tools/fit.py
QUANTUM = 16                           # rust/src/v2/mod.rs:228


def sphere(nv, m, seed):
    """nv Gaussian vectors of dimension m. Only the direction matters to tan(phi); the rotated block's direction is
    uniform on the sphere, which is the direction of a Gaussian vector."""
    return np.random.default_rng(seed).standard_normal((nv, m))


def tan2(c, y):
    """tan^2 of the angle between rows of c and rows of y."""
    dot = (c * y).sum(1)
    cn = (c * c).sum(1)
    yn = (y * y).sum(1)
    return np.maximum(cn * yn - dot * dot, 0.0) / np.maximum(dot * dot, 1e-300)


def summ(t2):
    """(mean tan^2, mean tan, tan of the mean angle) of a sample of tan^2."""
    t = np.sqrt(t2)
    return float(t2.mean()), float(t.mean()), float(np.tan(np.arctan(t).mean()))


def bound_tan2(R):
    """Asymptotic sphere bound: sin(phi) = 2^-R, tan^2 = 1 / (4^R - 1)."""
    return 1.0 / (4.0 ** R - 1.0)


# ---------------- the built code (rust/src/v2/mod.rs, rand_encode / rand_value / rand_choose / rate_depth) ----------------

def bits_of(r, nb, quantum=QUANTUM):
    """derive.rs bits_of."""
    if r < 1.0:
        k = int(r * nb)
        return k if quantum <= 1 else min((k + quantum - 1) // quantum * quantum, nb)
    return int(r * nb)


def depths(r, nb):
    """rate_depth over a block: depth of each position at block rate r (sub-bit rates code the first positions at 1 bit)."""
    d = np.zeros(nb, dtype=np.int64)
    if r < 1.0:
        d[: bits_of(r, nb)] = 1
    else:
        d[:] = int(r)
    return d


def built_values(v, maxd):
    """Greedy nested binary expansion of v (already divided by lambda rms) to maxd bits per position, returned as the
    cell index on the grid of step 2^(1 - maxd) (rand_encode: floor(v half) + half, clamped)."""
    half = np.ldexp(1.0, maxd - 1)  # 2^(maxd-1); 0.5 at depth 0
    cell = np.clip(np.floor(v * half) + half, 0.0, np.maximum(2.0 * half - 1.0, 0.0))
    return cell.astype(np.int64)


def built_decode(cell, maxd, d):
    """Value of the first d bits of a position coded to maxd >= d bits: the midpoint of its depth-d cell on [-1, 1)
    (rand_value: sum over the first d bits of +-2^-(b+1)); 0 at depth 0."""
    sh = np.maximum(maxd - d, 0)
    c = cell >> sh
    val = (2.0 * c + 1.0) * np.ldexp(1.0, -d) - 1.0
    return np.where(d > 0, val, 0.0)


def built_encode(Y, rise_depths, lams=LAMS):
    """rand_choose over rows of Y. rise_depths: list over the block's rises of an int array (depth per position).
    Returns tan^2 per (item, rise) for the scale that minimises the sum over rises of log tan^2."""
    nv, m = Y.shape
    maxd = rise_depths[-1]
    rms = np.sqrt((Y * Y).mean(1, keepdims=True))
    best = np.full(nv, np.inf)
    out = np.zeros((nv, len(rise_depths)))
    pick = np.zeros(nv, dtype=np.int64)
    for li, lam in enumerate(lams):
        v = Y / (lam * rms)
        cell = built_values(v, maxd[None, :])
        t2 = np.stack([tan2(built_decode(cell, maxd[None, :], d[None, :]), Y) for d in rise_depths], axis=1)
        obj = np.log(np.maximum(t2, 1e-300)).sum(1)
        bt = obj < best
        best[bt] = obj[bt]
        out[bt] = t2[bt]
        pick[bt] = li
    return out, pick


# ---------------- Gaussian scalar quantisers by quadrature ----------------

def _phi(x):
    return np.exp(-0.5 * x * x) / math.sqrt(2.0 * math.pi)


_erf = np.vectorize(math.erf)


def _Phi(x):
    return 0.5 * (1.0 + _erf(np.asarray(x, dtype=float) / math.sqrt(2.0)))


def cell_moments(edges):
    """For N(0,1) and cell edges e_0 < ... < e_K (e_0 = -inf, e_K = inf allowed): probability, first moment and
    second moment of each cell."""
    e = np.asarray(edges, dtype=float)
    P = _Phi(e)
    p = np.diff(P)
    ph = np.where(np.isfinite(e), _phi(np.where(np.isfinite(e), e, 0.0)), 0.0)
    m1 = ph[:-1] - ph[1:]
    ef = np.where(np.isfinite(e), e, 0.0)
    m2 = p + ef[:-1] * ph[:-1] - ef[1:] * ph[1:]
    return p, m1, m2


def lloyd_max(nlev, iters=4000):
    """Lloyd-Max quantiser of N(0,1) with nlev levels: (levels, edges, distortion)."""
    lev = np.linspace(-2.0, 2.0, nlev) if nlev > 1 else np.zeros(1)
    for _ in range(iters):
        edges = np.concatenate(([-np.inf], 0.5 * (lev[1:] + lev[:-1]), [np.inf]))
        p, m1, _ = cell_moments(edges)
        new = m1 / np.maximum(p, 1e-300)
        if np.max(np.abs(new - lev)) < 1e-13:
            lev = new
            break
        lev = new
    edges = np.concatenate(([-np.inf], 0.5 * (lev[1:] + lev[:-1]), [np.inf]))
    p, m1, m2 = cell_moments(edges)
    D = float((m2 - 2 * lev * m1 + lev * lev * p).sum())
    return lev, edges, D


def scalar_tan2(levels, edges):
    """tan^2 in the limit of a long block for a scalar quantiser of N(0,1) with a free gain: E c^2 E y^2 / (E c y)^2 - 1."""
    p, m1, _ = cell_moments(edges)
    ec2 = float((levels * levels * p).sum())
    ecy = float((levels * m1).sum())
    return ec2 / (ecy * ecy) - 1.0


def uniform_tan2(bits, lam):
    """The built code at one rate and one scale, long-block limit: 2^bits cells on [-lam, lam], midpoints, clamped."""
    k = 1 << bits
    step = 2.0 * lam / k
    edges = np.concatenate(([-np.inf], -lam + step * np.arange(1, k), [np.inf]))
    levels = -lam + step * (np.arange(k) + 0.5)
    return scalar_tan2(levels, edges)


def golden(f, a, b, tol=1e-7):
    g = (math.sqrt(5.0) - 1.0) / 2.0
    c, d = b - g * (b - a), a + g * (b - a)
    fc, fd = f(c), f(d)
    while b - a > tol:
        if fc < fd:
            b, d, fd = d, c, fc
            c = b - g * (b - a)
            fc = f(c)
        else:
            a, c, fc = c, d, fd
            d = a + g * (b - a)
            fd = f(d)
    x = 0.5 * (a + b)
    return x, f(x)
