# Grid certificate for the worst-case constant over variance shares.
#
# Claim checked: for every a in R_+^J with sum_j a_j^2 / d_j = 1,  P[sum_j a_j U_j >= X] <= delta.
# Reduction (proved in the report):
#  (1) the tail is non-decreasing in each a_j, so shares w_j = a_j^2 / d_j may be rounded up to the grid
#      G = { g in (h Z_{>=0})^J : sum_j g_j = 1 + (J - 1) h }, which dominates every point of the simplex;
#  (2) a_j U_j <= Y_j := eta * ceil(a_j U_j / eta), so P[sum a_j U_j >= X] <= P[sum Y_j >= X], a lattice law;
#  (3) the lattice tail is evaluated exactly (up to floating point) through the discrete Fourier transform of the
#      exponentially tilted pmfs; the frequencies above k0 are bounded by sum |Phi_j(k)| |kappa(k)| for the block with
#      the smallest such sum (every other factor has modulus at most 1).
# Margins: REL (relative, on the tail) for quadrature and rounding, ABS on the O(1) tilted sum.
import numpy as np, math, sys, time, itertools
from scipy.special import gammaln

REL, ABS = 1e-6, 1e-11
GLX, GLW = np.polynomial.legendre.leggauss(8)

def tilted_pmf(d, a, eta, lam):
    """q(k) = P[Y = k eta] exp(lam k eta) for Y = eta ceil(a U / eta), k = kmin..kmax (arrays k, q)."""
    if a <= 0:
        return np.array([0]), np.array([1.0])
    kmax = int(math.ceil(a / eta)); kmin = int(math.ceil(-a / eta))
    k = np.arange(kmin, kmax + 1)
    lo = np.clip((k - 1) * eta / a, -1.0, 1.0); hi = np.clip(k * eta / a, -1.0, 1.0)
    mid = (lo + hi) / 2; half = (hi - lo) / 2
    s = mid[:, None] + half[:, None] * GLX[None, :]
    lc = gammaln(d / 2) - 0.5 * math.log(math.pi) - gammaln((d - 1) / 2)
    with np.errstate(divide="ignore"):
        lf = lc + (d - 3) / 2 * np.log1p(-s * s)
    p = (np.exp(lf + (lam * eta) * k[:, None]) * GLW[None, :]).sum(1) * half
    return k, p

class Block:
    """Per share index i (share i h, scale sqrt(i h d)): ln M, low-frequency spectrum, truncation weight."""
    def __init__(self, d, top, h, eta, lam, M, k0, absk):
        self.lnM = np.zeros(top + 1); self.Phi = np.zeros((top + 1, k0 + 1), complex); self.R = np.zeros(top + 1)
        self.kmax = np.zeros(top + 1, int); self.kmin = np.zeros(top + 1, int)
        for i in range(top + 1):
            a = math.sqrt(i * h * d)
            k, q = tilted_pmf(d, a, eta, lam)
            m = q.sum(); self.lnM[i] = math.log(m)
            arr = np.zeros(M); arr[k % M] = q / m
            F = np.fft.rfft(arr)                      # k = 0..M/2
            self.Phi[i] = F[:k0 + 1]
            self.R[i] = (2 * (np.abs(F[k0 + 1:M // 2]) * absk[k0 + 1:M // 2]).sum() + abs(F[M // 2]) * absk[M // 2]) / M
            self.kmax[i] = k[-1]; self.kmin[i] = k[0]

def certify(ds, lnd, X, K, eta=0.001, lam=None, M=65536, k0=400, verbose=True, blas_chunk=None):
    """True if max over the grid of the lattice tail at X is at most exp(lnd). Returns (ok, max ln tail, argmax)."""
    J = len(ds); h = 1.0 / K; top = K + J - 1
    lam = lam or X
    mx = int(round(X / eta)); assert abs(mx * eta - X) < 1e-9, "X must be a multiple of eta"
    # omega(m) = exp(-lam (m - mx) eta) on mx <= m <= M/2 - 1; kappa = conj(fft(omega))
    om = np.zeros(M); mm = np.arange(mx, M // 2); om[mm] = np.exp(-lam * (mm - mx) * eta)
    kap = np.conj(np.fft.rfft(om)); absk = np.abs(kap)
    t0 = time.time()
    B = [Block(d, top, h, eta, lam, M, k0, absk) for d in ds]
    if verbose: print(f"   tables {time.time()-t0:.0f}s", flush=True)
    # support check: the convolution must fit in [-M/2, M/2 - 1] for every grid point
    # (for a grid point, sum_j ceil(a_j / eta) <= sqrt(sum_j d_j (1 + (J - 1) h)) / eta + J by Cauchy-Schwarz)
    assert math.sqrt(sum(ds) * (1 + (J - 1) * h)) / eta + J < M // 2, "lattice window too small"
    wt = np.full(k0 + 1, 2.0); wt[0] = 1.0
    kw = kap[:k0 + 1] * wt / M
    best = (-np.inf, None)
    if J == 1:
        raise ValueError("one block: use the beta tail")
    # split the blocks into two halves P, Q
    P = list(range((J + 1) // 2)); Q = list(range((J + 1) // 2, J))
    def side(blocks, s):
        """All index tuples of the given blocks summing to s: spectra product (rows), ln M sums, min R, tuples."""
        if len(blocks) == 1:
            b = B[blocks[0]]
            return b.Phi[s:s + 1], b.lnM[s:s + 1], b.R[s:s + 1], [(s,)]
        b1, b2 = B[blocks[0]], B[blocks[1]]
        i = np.arange(0, s + 1)
        return b1.Phi[i] * b2.Phi[s - i], b1.lnM[i] + b2.lnM[s - i], np.minimum(b1.R[i], b2.R[s - i]), [(int(x), int(s - x)) for x in i]
    base = -lam * mx * eta
    for s in range(0, top + 1):
        FA, LA, RA, TA = side(P, s)
        FB, LB, RB, TB = side(Q, top - s)
        FA = FA * kw[None, :]
        S = FA.real @ FB.real.T - FA.imag @ FB.imag.T
        S = S + np.minimum(RA[:, None], RB[None, :]) + ABS
        lt = np.log(np.maximum(S, 1e-300)) + LA[:, None] + LB[None, :] + base + math.log1p(REL)
        ij = np.unravel_index(np.argmax(lt), lt.shape)
        if lt[ij] > best[0]:
            best = (float(lt[ij]), TA[ij[0]] + TB[ij[1]])
    if verbose: print(f"   grid {time.time()-t0:.0f}s; max ln tail {best[0]:.5f} vs ln delta {lnd:.5f} at shares {np.array(best[1]) * h}", flush=True)
    return best[0] <= lnd, best[0], best[1]

def lattice_lntail(ds, w, X, eta=0.001, lam=None, M=65536):
    """ln of the lattice upper bound at one share vector (direct, all frequencies), for testing."""
    lam = lam or X
    mx = int(round(X / eta))
    om = np.zeros(M); mm = np.arange(mx, M // 2); om[mm] = np.exp(-lam * (mm - mx) * eta)
    kap = np.conj(np.fft.rfft(om))
    F = np.ones(M // 2 + 1, complex); lnM = 0.0
    for d, wj in zip(ds, w):
        k, q = tilted_pmf(d, math.sqrt(wj * d), eta, lam)
        m = q.sum(); lnM += math.log(m)
        arr = np.zeros(M); arr[k % M] = q / m
        F *= np.fft.rfft(arr)
    wt = np.full(M // 2 + 1, 2.0); wt[0] = 1.0; wt[-1] = 1.0
    S = (F * kap * wt).real.sum() / M
    return math.log(S) + lnM - lam * mx * eta

if __name__ == "__main__":
    from ctable import quantiles
    ds = [95, 127, 255, 287]; L1 = math.log(2 * 1e9 * 4 / 1e-9)
    # test of the lattice bound against the inversion integral: T(x - 4 eta) >= T_lattice(x) >= T(x)
    from law import tail
    for w in ([0.1254, 0.167, 0.333, 0.3746], [0.25] * 4, [0.9, 0, 0, 0.1], [1.0, 0, 0, 0], [0.01, 0.02, 0.03, 0.94]):
        a = [math.sqrt(x * d) for x, d in zip(w, ds)]
        for X in (8.0, 8.766):
            lt = lattice_lntail(ds, w, X)
            e0 = tail(a, ds, X)[1]; e4 = tail(a, ds, X - 0.004)[1]
            print(np.round(w, 3), X, f"exact ln T(X) {e0:.6f} <= lattice {lt:.6f} <= exact ln T(X - 4 eta) {e4:.6f}", e0 <= lt <= e4)
