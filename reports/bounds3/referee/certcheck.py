# Is the certificate's per-corner number an upper bound of the exact tail? cert2's own tables against my FFT tail.
import sys, math, numpy as np
sys.path.insert(0, "/home/joe/code/vexact-const/reports/bounds3")
from cert import tilted_pmf, REL, ABS, lattice_lntail
from cert2 import Tables
from mytail import lntails_fft
ds = [95, 127, 255, 287]; J = 4
rng = np.random.default_rng(5)
eta, M, k0, Kmax = 0.001, 65536, 400, 4096
worst_gap = 1e9; worst_trunc = -1e9; n = 0
for X in (7.957, 8.704, 8.774):
    lam = X; mx = int(round(X / eta))
    om = np.zeros(M); mm = np.arange(mx, M // 2); om[mm] = np.exp(-lam * (mm - mx) * eta)
    kap = np.conj(np.fft.rfft(om)); absk = np.abs(kap)
    wt = np.full(k0 + 1, 2.0); wt[0] = 1.0; kw = kap[:k0 + 1] * wt / M
    T = Tables(ds, Kmax, eta, lam, M, k0, absk, [1.0] * J)
    base = -lam * mx * eta + math.log1p(REL)
    for trial in range(120):
        K = int(rng.choice([64, 256, 1024, 4096])); top = K + J - 1
        if trial % 3 == 0:   # near the worst shares
            w = np.array([0.1254, 0.167, 0.333, 0.3746]) + rng.normal(0, 0.02, 4)
        elif trial % 3 == 1: # faces and corners
            w = rng.dirichlet([0.3] * 4)
        else:
            w = rng.dirichlet([2.0] * 4)
        w = np.abs(w); w /= w.sum()
        i = np.ceil(w[:3] * K - 1e-12).astype(int); i4 = top - i.sum()
        if i4 < 1: continue
        idx = list(i) + [i4]
        Ls = 0; Ps = kw.copy(); Rs = np.inf
        for j in range(J):
            Lj, Pj, Rj = T.get(j, K, np.array([idx[j]]))
            Ls += Lj[0]; Ps = Ps * Pj[0]; Rs = min(Rs, Rj[0])
        S = Ps.real.sum() + Rs + ABS
        lt_cert = math.log(S) + Ls + base
        g = np.array(idx) / K
        full = lattice_lntail(ds, g, X, eta=eta, M=M)             # all frequencies, same lattice
        a = [math.sqrt(x * d) for x, d in zip(g, ds)]
        xs, lt = lntails_fft(a, ds, 8.3, eta=0.001, M=1 << 17)
        exact_corner = lt[int(round(X / 0.001))]
        a0 = [math.sqrt(x * d) for x, d in zip(w, ds)]
        xs, lt0 = lntails_fft(a0, ds, 8.3, eta=0.001, M=1 << 17)
        exact_w = lt0[int(round(X / 0.001))]
        n += 1
        worst_gap = min(worst_gap, lt_cert - exact_corner, exact_corner - exact_w, full - exact_corner)
        worst_trunc = max(worst_trunc, full - lt_cert)
        if lt_cert < exact_corner or exact_corner < exact_w - 1e-9 or lt_cert < full:
            print("VIOLATION", X, K, w, idx, lt_cert, full, exact_corner, exact_w)
print(n, "cases; smallest of (cert - exact at corner, exact at corner - exact at w, full lattice - exact at corner):", worst_gap, "; largest (full lattice - cert):", worst_trunc)
