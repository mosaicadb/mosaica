# Certify the randomised levels' worst-case constant for an index's block layout and write it to its meta.json.
#
# Statement certified, for the index's block dimensions d_j = nb_j - 1 and a ln delta:
#   P[sum_j a_j U_j >= X] <= delta  for every a >= 0 with sum_j a_j^2 / d_j = 1,
# U_j a coordinate of a uniform unit vector in R^{d_j}, independent. The Rust build (src/v2/tail.rs) uses X as the
# constant of every level whose coded blocks are a subset of these, at every budget at least as loose, and otherwise
# falls back to the convex-order and Chernoff bounds.
#
# Method (reports/bounds3/REPORT.txt, R3; cert.py and cert2.py there):
#  (1) the tail is non-decreasing in every a_j, so shares may be rounded up: at grid step h = 1/K every share vector
#      lies in a cell whose corner dominates it; cells whose corner fails are split and retested at 2K, up to Kmax;
#  (2) a_j U_j <= eta ceil(a_j U_j / eta), so the tail is at most that of a lattice sum;
#  (3) the lattice tail is an exact finite sum, evaluated through the discrete Fourier transform of the exponentially
#      tilted probabilities; the frequencies left out are bounded by the block with the smallest remainder.
# Margins REL (relative) and ABS (absolute) cover quadrature and rounding.
#
# The fit (src/fit.rs) runs the same method in Rust (src/certify.rs) and first looks in a shipped table of certificates
# (src/certs_table.json, made by tools/certs_table.py with both); tools/fit.py looks in that table, then runs this.
# This script stays as the independent implementation: it never reads the table.
#
# usage: python3 -I certify.py INDEX [--n-guarantee N] [--lnd LN_DELTA] [--time-cap SECONDS]
#   The default budget is the one the Rust build uses: delta = EPS / (N L), N the index's item count rounded up to a
#   power of two, L the number of randomised levels.
import argparse, itertools, json, math, os, sys, time
import numpy as np
from scipy.special import betaincinv, gammaln

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


class Tables:
    """Per-block rows built on demand, keyed by the share's numerator over Kmax."""
    def __init__(self, ds, Kmax, eta, lam, M, k0, absk):
        self.ds, self.Kmax, self.eta, self.lam, self.M, self.k0, self.absk = ds, Kmax, eta, lam, M, k0, absk
        self.key = [dict() for _ in ds]
        self.lnM = [[] for _ in ds]; self.Phi = [[] for _ in ds]; self.R = [[] for _ in ds]
        self.arr = [None for _ in ds]

    def get(self, j, K, idx):
        mult = self.Kmax // K
        rows = np.empty(len(idx), int)
        new = False
        for n, i in enumerate(idx):
            f = int(i) * mult
            r = self.key[j].get(f)
            if r is None:
                k, q = tilted_pmf(self.ds[j], math.sqrt(f / self.Kmax * self.ds[j]), self.eta, self.lam)
                m = q.sum()
                arr = np.zeros(self.M); arr[k % self.M] = q / m
                F = np.fft.rfft(arr); k0, M = self.k0, self.M
                r = len(self.lnM[j]); self.key[j][f] = r
                self.lnM[j].append(math.log(m)); self.Phi[j].append(F[:k0 + 1].copy())  # a view would hold all of F
                self.R[j].append((2 * (np.abs(F[k0 + 1:M // 2]) * self.absk[k0 + 1:M // 2]).sum() + abs(F[M // 2]) * self.absk[M // 2]) / M)
                new = True
            rows[n] = r
        if new or self.arr[j] is None:
            self.arr[j] = (np.array(self.lnM[j]), np.array(self.Phi[j]), np.array(self.R[j]))
        lnM, Phi, R = self.arr[j]
        return lnM[rows], Phi[rows], R[rows]


def certify(ds, lnd, X, deadline, K0=64, Kmax=4096, eta=0.001, M=65536, k0=400):
    """True if the lattice upper bound of the tail at X is at most exp(lnd) on every cell of the share simplex.
    False when a cell still fails at Kmax or the deadline (time.time()) passes."""
    J = len(ds); assert 2 <= J <= 4
    lam = X
    mx = int(round(X / eta)); assert abs(mx * eta - X) < 1e-9, "X must be a multiple of eta"
    assert math.sqrt(sum(ds) * (1 + (J - 1) / K0)) / eta + J < M // 2, "lattice window too small"
    om = np.zeros(M); mm = np.arange(mx, M // 2); om[mm] = np.exp(-lam * (mm - mx) * eta)
    kap = np.conj(np.fft.rfft(om)); absk = np.abs(kap)
    wt = np.full(k0 + 1, 2.0); wt[0] = 1.0
    kw = kap[:k0 + 1] * wt / M
    T = Tables(ds, Kmax, eta, lam, M, k0, absk)
    base = -lam * mx * eta + math.log1p(REL)
    K = K0
    box = [(0, K + J - 1)] * (J - 1)          # index ranges of the free coordinates
    while True:
        if time.time() > deadline:
            return False
        top = K + J - 1
        fails = None                            # the failing cells' least and greatest index per free coordinate
        if J == 4:
            (lo1, hi1), (lo2, hi2), (lo3, hi3) = box
            for s in range(lo1 + lo2, hi1 + hi2 + 1):
                i1 = np.arange(max(lo1, s - hi2), min(hi1, s - lo2) + 1)
                i3 = np.arange(lo3, min(hi3, top - s - 1) + 1)
                if len(i1) == 0 or len(i3) == 0:
                    continue
                i2 = s - i1; i4 = top - s - i3
                L1_, P1, R1 = T.get(0, K, i1); L2_, P2, R2 = T.get(1, K, i2)
                L3_, P3, R3 = T.get(2, K, i3); L4_, P4, R4 = T.get(3, K, i4)
                A = P1 * P2 * kw[None, :]; B = P3 * P4
                S = A.real @ B.real.T - A.imag @ B.imag.T
                S = S + np.minimum(np.minimum(R1, R2)[:, None], np.minimum(R3, R4)[None, :]) + ABS
                lt = np.log(np.maximum(S, 1e-300)) + (L1_ + L2_)[:, None] + (L3_ + L4_)[None, :] + base
                f = np.argwhere(lt > lnd)
                if len(f):
                    if K >= Kmax:               # a cell fails on the finest grid: the answer is known
                        return False
                    fails = extent(fails, np.c_[i1[f[:, 0]], i2[f[:, 0]], i3[f[:, 1]]])
        else:
            rng = [range(lo, hi + 1) for lo, hi in box]
            tup = np.array([p for p in itertools.product(*rng) if top - sum(p) >= 1], int).reshape(-1, J - 1)
            last = top - tup.sum(1)
            for c0 in range(0, len(tup), 4000):
                tt = tup[c0:c0 + 4000]; ll = last[c0:c0 + 4000]
                Ls = 0; Ps = kw[None, :]; Rs = None
                for j in range(J):
                    Lj, Pj, Rj = T.get(j, K, tt[:, j] if j < J - 1 else ll)
                    Ls = Ls + Lj; Ps = Ps * Pj; Rs = Rj if Rs is None else np.minimum(Rs, Rj)
                lt = np.log(np.maximum(Ps.real.sum(1) + Rs + ABS, 1e-300)) + Ls + base
                if (lt > lnd).any():
                    if K >= Kmax:
                        return False
                    fails = extent(fails, tt[lt > lnd])
        if fails is None:
            return True
        box = [(max(0, 2 * int(fails[0][j]) - 1), 2 * int(fails[1][j])) for j in range(J - 1)]
        K *= 2


def extent(e, F):
    """The least and greatest index in each column of the failing cells F, and of those in e (None: none yet). Only
    these set the next grid's box, so the cells themselves are not kept (at Kmax they can number 1e8)."""
    lo, hi = F.min(0), F.max(0)
    return (lo, hi) if e is None else (np.minimum(e[0], lo), np.maximum(e[1], hi))


def sphere_constant(D, lnd):
    """c with P[U >= c / sqrt(D)] = exp(lnd) for a coordinate U of a uniform unit vector in R^D."""
    return math.sqrt(1 - betaincinv((D - 1) / 2, 0.5, 2 * math.exp(lnd))) * math.sqrt(D)


def certify_layout(ds, lnd, time_cap, log=print):
    """The smallest X on a 0.001 lattice, from the one-sphere prediction in sum_j (d_j + 2) - 2 dimensions plus 0.008,
    that certifies, or None within `time_cap` seconds."""
    deadline = time.time() + time_cap
    big = sum(ds) > 1100
    pred = sphere_constant(sum(ds) + 2 * len(ds) - 2, lnd)
    X = math.ceil((pred + 0.008) * 1000) / 1000
    t0 = time.time()
    for _ in range(6):
        ok = certify(ds, lnd, round(X, 3), deadline, M=131072 if big else 65536, k0=800 if big else 400)
        log(f"certify {ds} ln delta {lnd:.4f}: X {X:.3f} {'certified' if ok else 'not certified'} [{time.time() - t0:.0f}s]")
        if ok:
            return round(X, 3)
        if time.time() > deadline:
            return None
        X += 0.002
    return None


def block_dims(meta):
    b = meta["BOUNDS"]
    return [b[j + 1] - b[j] - 1 for j in range(len(b) - 1)]


def ln_delta(meta, n_guarantee=None):
    """ln of the per-event probability the Rust build uses: EPS / (N L), with N the item count rounded up to a power
    of two unless given. Each N needs its own certificate (one at a smaller ln delta also serves)."""
    N = n_guarantee or 2 ** math.ceil(math.log2(max(meta["N"], 1)))
    return -math.log(N * len(meta["RLV"]) / meta.get("EPS", 1e-9))


TABLE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "certs_table.json")


def table_entry(ds, lnd):
    """The shipped table's entry (src/certs_table.json, tools/certs_table.py) for exactly these blocks and this ln delta:
    a list [X] (X None where no certificate was found), or None when the table has no such entry."""
    if not os.path.exists(TABLE):
        return None
    for e in json.load(open(TABLE)):
        if e["ds"] == ds and e["lnd"] == lnd:
            return [e["X"]]
    return None


def add_certificate(out, lnd, time_cap, log=print, table=False):
    """Certify the layout of the index in `out` at `lnd` and record the result in its meta.json (CERTS). With
    `table`, the shipped table's entry for the layout and budget, when it has one, in place of certifying (a time cap
    of 0 still certifies nothing)."""
    path = os.path.join(out, "meta.json")
    meta = json.load(open(path))
    ds = block_dims(meta)
    hit = table_entry(ds, lnd) if table and time_cap > 0 else None
    if hit is not None:
        X = hit[0]
        log(f"certificate for {ds} at ln delta {lnd:.4f} from the shipped table: X {X}")
    else:
        X = certify_layout(ds, lnd, time_cap, log)
    if X is None:
        log(f"no certificate for {ds} at ln delta {lnd:.4f}; the build uses the convex-order and Chernoff bounds")
        return None
    certs = [c for c in meta.get("CERTS", []) if not (c["ds"] == ds and abs(c["lnd"] - lnd) < 1e-9)]
    certs.append(dict(ds=ds, lnd=lnd, X=X))
    meta["CERTS"] = certs
    with open(path + ".tmp", "w") as f:
        json.dump(meta, f, indent=1)
    os.replace(path + ".tmp", path)
    return X


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("index")
    ap.add_argument("--n-guarantee", type=float, default=None, help="N in the budget (default: items rounded up to a power of two)")
    ap.add_argument("--lnd", type=float, default=None, help="ln delta, in place of the budget")
    ap.add_argument("--time-cap", type=float, default=600)
    a = ap.parse_args()
    meta = json.load(open(os.path.join(a.index, "meta.json")))
    lnd = a.lnd if a.lnd is not None else ln_delta(meta, a.n_guarantee)
    X = add_certificate(a.index, lnd, a.time_cap)
    print(json.dumps(dict(ds=block_dims(meta), lnd=lnd, X=X)))


if __name__ == "__main__":
    main()
