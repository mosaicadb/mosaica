# Exact quantile c(w) on a grid of variance shares (floating point; for measurement, not a certificate).
# c(w) = x with P[sum_j sqrt(w_j d_j) U_j >= x] = exp(lnd), by the inversion integral at a fixed real part lam.
import numpy as np, math, itertools, time, sys
from law import lnmgf

def quantiles(W, ds, lnds, lam=9.0, T=40.0, npts=801):
    """W: (m, J) shares (rows sum to anything; scales a_j = sqrt(w_j d_j)). Returns (m, len(lnds)) quantiles."""
    W = np.asarray(W, float); m, J = W.shape
    t = np.linspace(-T, T, npts); z = lam + 1j * t
    Kz = np.zeros((m, npts), complex)
    for j in range(J):
        a = np.sqrt(W[:, j] * ds[j])
        nz = a > 0
        if nz.any():
            Kz[nz] += lnmgf(ds[j], (a[nz, None] * z[None, :]))
    K0 = Kz[:, npts // 2].real
    dt = t[1] - t[0]
    def lt(x):
        g = np.exp(Kz - K0[:, None] - 1j * t[None, :] * x[:, None]) / z[None, :]
        val = (g.sum(1) - 0.5 * (g[:, 0] + g[:, -1])).real * dt / (2 * math.pi)
        return np.log(np.maximum(val, 1e-300)) + K0 - lam * x
    out = np.zeros((m, len(lnds)))
    sig = np.sqrt(W.sum(1))
    for i, lnd in enumerate(lnds):
        lo = np.full(m, 3.0) * sig; hi = np.full(m, 1.02 * math.sqrt(-2 * lnd)) * sig
        for _ in range(48):
            mid = (lo + hi) / 2
            above = lt(mid) > lnd
            lo = np.where(above, mid, lo); hi = np.where(above, hi, mid)
        out[:, i] = (lo + hi) / 2
    return out

def simplex_grid(J, K):
    """All i in Z_{>=0}^J with sum K, as shares i / K."""
    pts = [p + (K - sum(p),) for p in itertools.product(range(K + 1), repeat=J - 1) if sum(p) <= K]
    return np.array(pts, float) / K

if __name__ == "__main__":
    from law import cw, c1
    ds = [95, 127, 255, 287]
    L1 = math.log(2 * 1e9 * 4 / 1e-9)
    W = np.array([[0.25] * 4, [95 / 764, 127 / 764, 255 / 764, 287 / 764], [0.336, 0.398, 0.26, 0.006], [1, 0, 0, 0], [0, 0, 0, 1], [0.9, 0, 0, 0.1]])
    t0 = time.time()
    q = quantiles(W, ds, [-L1])
    print(np.round(q[:, 0], 5), f"{time.time()-t0:.2f}s")
    print([round(cw(w, ds, -L1), 5) for w in W[:3]], round(c1(95, -L1), 5), round(c1(287, -L1), 5))
