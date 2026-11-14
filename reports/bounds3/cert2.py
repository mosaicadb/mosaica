# Adaptive grid certificate for the worst-case constant over variance shares (see cert.py for the reduction).
#
# certify(ds, lnd, X): True if  P[sum_j a_j U_j >= X] <= exp(lnd)  for every a >= 0 with sum_j a_j^2 / d_j = 1,
# as far as floating point allows (margins REL, ABS below; the discretisations err on the safe side).
#
# Levels K = K0, 2 K0, ..., Kmax (h = 1 / K). At level K the simplex is covered by the cells
#   C(i) = { w : (i_j - 1) h < w_j <= i_j h for j < J,  w_J = 1 - sum_{j<J} w_j >= 0 },  i in Z_{>=0}^{J-1},
# and every w in C(i) is dominated coordinate by coordinate by the corner g(i) = (i_1 h, .., i_{J-1} h, (K + J - 1 - sum i) h).
# The tail is non-decreasing in every scale, so a cell passes if the lattice upper bound of the tail at g(i) is at
# most delta. Cells that fail are split into 2^(J-1) at the next level; the next level evaluates every cell in the
# bounding box of the failing ones. The certificate holds if no cell fails at some level.
import numpy as np, math, sys, time
from cert import tilted_pmf, REL, ABS

class Tables:
    """Lazily built per-block rows, keyed by the share's numerator over Kmax."""
    def __init__(self, ds, Kmax, eta, lam, M, k0, absk, scale):
        self.ds, self.Kmax, self.eta, self.lam, self.M, self.k0, self.absk, self.scale = ds, Kmax, eta, lam, M, k0, absk, scale
        self.key = [dict() for _ in ds]
        self.lnM = [[] for _ in ds]; self.Phi = [[] for _ in ds]; self.R = [[] for _ in ds]
        self.arr = [None for _ in ds]
    def rows(self, j, K, idx):
        """Row numbers for block j, shares idx / K (builds missing rows)."""
        mult = self.Kmax // K
        out = np.empty(len(idx), int)
        new = False
        for n, i in enumerate(idx):
            f = int(i) * mult
            r = self.key[j].get(f)
            if r is None:
                a = math.sqrt(f / self.Kmax * self.ds[j] * self.scale[j])
                k, q = tilted_pmf(self.ds[j], a, self.eta, self.lam)
                m = q.sum()
                arr = np.zeros(self.M); arr[k % self.M] = q / m
                F = np.fft.rfft(arr); k0, M = self.k0, self.M
                r = len(self.lnM[j]); self.key[j][f] = r
                self.lnM[j].append(math.log(m)); self.Phi[j].append(F[:k0 + 1])
                self.R[j].append((2 * (np.abs(F[k0 + 1:M // 2]) * self.absk[k0 + 1:M // 2]).sum() + abs(F[M // 2]) * self.absk[M // 2]) / M)
                new = True
            out[n] = r
        if new or self.arr[j] is None:
            self.arr[j] = (np.array(self.lnM[j]), np.array(self.Phi[j]), np.array(self.R[j]))
        return out
    def get(self, j, K, idx):
        r = self.rows(j, K, idx)
        lnM, Phi, R = self.arr[j]
        return lnM[r], Phi[r], R[r]

def certify(ds, lnd, X, K0=64, Kmax=4096, eta=0.001, lam=None, M=65536, k0=400, verbose=True, scale=None):
    """scale s_j (default 1): the statement certified is, for every a >= 0 with sum_j a_j^2 / (d_j s_j) = 1,
    P[sum_j a_j U_j >= X] <= exp(lnd); grid shares are g_j = a_j^2 / (d_j s_j)."""
    J = len(ds); assert 2 <= J <= 4
    scale = [1.0] * J if scale is None else list(scale)
    lam = lam or X
    mx = int(round(X / eta)); assert abs(mx * eta - X) < 1e-9, "X must be a multiple of eta"
    assert math.sqrt(sum(d * sj for d, sj in zip(ds, scale)) * (1 + (J - 1) / K0)) / eta + J < M // 2, "lattice window too small"
    om = np.zeros(M); mm = np.arange(mx, M // 2); om[mm] = np.exp(-lam * (mm - mx) * eta)
    kap = np.conj(np.fft.rfft(om)); absk = np.abs(kap)
    wt = np.full(k0 + 1, 2.0); wt[0] = 1.0
    kw = kap[:k0 + 1] * wt / M
    T = Tables(ds, Kmax, eta, lam, M, k0, absk, scale)
    base = -lam * mx * eta + math.log1p(REL)
    t0 = time.time()
    K = K0
    box = [(0, K + J - 1)] * (J - 1)          # index ranges of the free coordinates
    worst = -np.inf
    while True:
        top = K + J - 1
        fails = []; nev = 0; lev_max = (-np.inf, None)
        if J == 4:
            (lo1, hi1), (lo2, hi2), (lo3, hi3) = box
            for s in range(lo1 + lo2, hi1 + hi2 + 1):
                i1 = np.arange(max(lo1, s - hi2), min(hi1, s - lo2) + 1)
                i3 = np.arange(lo3, min(hi3, top - s - 1) + 1)
                if len(i1) == 0 or len(i3) == 0: continue
                i2 = s - i1; i4 = top - s - i3
                L1_, P1, R1 = T.get(0, K, i1); L2_, P2, R2 = T.get(1, K, i2)
                L3_, P3, R3 = T.get(2, K, i3); L4_, P4, R4 = T.get(3, K, i4)
                A = P1 * P2 * kw[None, :]; B = P3 * P4
                S = A.real @ B.real.T - A.imag @ B.imag.T
                S = S + np.minimum(np.minimum(R1, R2)[:, None], np.minimum(R3, R4)[None, :]) + ABS
                lt = np.log(np.maximum(S, 1e-300)) + (L1_ + L2_)[:, None] + (L3_ + L4_)[None, :] + base
                nev += lt.size
                m = lt.max()
                if m > lev_max[0]:
                    ij = np.unravel_index(np.argmax(lt), lt.shape)
                    lev_max = (float(m), (int(i1[ij[0]]), int(i2[ij[0]]), int(i3[ij[1]]), int(i4[ij[1]])))
                f = np.argwhere(lt > lnd)
                if len(f):
                    fails.append(np.c_[i1[f[:, 0]], i2[f[:, 0]], i3[f[:, 1]]])
        else:
            # J = 2 or 3: scattered evaluation
            import itertools
            rng = [range(lo, hi + 1) for lo, hi in box]
            tup = np.array([p for p in itertools.product(*rng) if top - sum(p) >= 1], int).reshape(-1, J - 1)
            last = top - tup.sum(1)
            for c0 in range(0, len(tup), 4000):
                tt = tup[c0:c0 + 4000]; ll = last[c0:c0 + 4000]
                Ls = 0; Ps = kw[None, :]; Rs = None
                for j in range(J):
                    idx = tt[:, j] if j < J - 1 else ll
                    Lj, Pj, Rj = T.get(j, K, idx)
                    Ls = Ls + Lj; Ps = Ps * Pj; Rs = Rj if Rs is None else np.minimum(Rs, Rj)
                S = Ps.real.sum(1) + Rs + ABS
                lt = np.log(np.maximum(S, 1e-300)) + Ls + base
                nev += len(lt)
                a = int(np.argmax(lt))
                if lt[a] > lev_max[0]: lev_max = (float(lt[a]), tuple(int(x) for x in tt[a]) + (int(ll[a]),))
                if (lt > lnd).any(): fails.append(tt[lt > lnd])
        nf = sum(len(f) for f in fails)
        worst = max(worst, lev_max[0]) if nf == 0 else worst
        if verbose:
            print(f"   K {K}: evaluated {nev}, failing {nf}, max ln tail {lev_max[0]:.5f} at shares {np.round(np.array(lev_max[1]) / K, 5)} [{time.time()-t0:.0f}s]", flush=True)
        if nf == 0:
            return True, lev_max[0], K
        if K >= Kmax:
            return False, lev_max[0], K
        F = np.vstack(fails)
        box = [(max(0, 2 * int(F[:, j].min()) - 1), 2 * int(F[:, j].max())) for j in range(J - 1)]
        K *= 2

if __name__ == "__main__":
    ds = [int(x) for x in sys.argv[1].split(",")]
    lnd = float(sys.argv[2]); X = float(sys.argv[3])
    Kmax = int(sys.argv[4]) if len(sys.argv) > 4 else 4096
    eta = float(sys.argv[5]) if len(sys.argv) > 5 else 0.001
    M = int(sys.argv[6]) if len(sys.argv) > 6 else 65536
    k0 = int(sys.argv[7]) if len(sys.argv) > 7 else 400
    ok, mx, K = certify(ds, lnd, X, Kmax=Kmax, eta=eta, M=M, k0=k0)
    print(f"ds {ds} ln delta {lnd:.6f} X {X}: certified {ok} at K {K}; max ln tail at the last level {mx:.6f}")
