"""Is "the build must not encode against the rounded matrix" necessary for the exact law? A deliberately coarse
rounding (grid g on the entries) so that an effect can be seen: the one-bit code is taken from R~ y, and kappa,
tan(phi) and the error are the exact ones for that code (so only the code's dependence on R beyond R y is tested).
y = (e1 + c e2)/|.|, Delta = (e2 - c e1)/|.|. Only the first two columns of R are needed."""
import numpy as np
from scipy import special

rng = np.random.default_rng(5)


def law_tail(s, d):
    return 0.5 * special.betainc((d - 1) / 2, 0.5, 1 - s * s / d)


for n in (16, 64):
    d = n - 1
    for g in (0.0, 0.25 / np.sqrt(n / 16), 2.0 ** -6, 2.0 ** -10):
        for c in (0.5,):
            tot = 4_000_000
            s1 = 0.0; s2 = 0.0; cnt = {2.0: [0, 0], 3.0: [0, 0]}
            changed = 0
            for _ in range(tot // 200000):
                m = 200000
                u = rng.standard_normal((m, n)); u /= np.linalg.norm(u, axis=1)[:, None]
                v = rng.standard_normal((m, n)); v -= (v * u).sum(1)[:, None] * u; v /= np.linalg.norm(v, axis=1)[:, None]
                nrm = np.sqrt(1 + c * c)
                z = (u + c * v) / nrm
                w = (v - c * u) / nrm
                if g > 0:
                    zt = (np.round(u / g) * g + c * np.round(v / g) * g) / nrm
                else:
                    zt = z
                o = np.where(zt >= 0, 0.5, -0.5)
                o0 = np.where(z >= 0, 0.5, -0.5)
                changed += int((o != o0).any(1).sum())
                dot = (o * z).sum(1)
                ok = dot > 0
                kappa = 1.0 / np.where(ok, dot, 1.0)
                e = kappa[:, None] * o - z
                A = np.linalg.norm(e, axis=1)
                U = (e * w).sum(1) / A * np.sqrt(d)
                U = U[ok]
                s1 += U.sum(); s2 += (U * U).sum()
                for k in cnt:
                    cnt[k][0] += int((U > k).sum()); cnt[k][1] += int((U < -k).sum())
            se = 1 / np.sqrt(tot)
            print(f"n={n} grid {g:.4g} c={c}: code differs from the exact-matrix code in {changed/tot:.4f} of draws; "
                  f"mean of standardised error {s1/tot:+.5f} (s.e. {se:.5f}), mean square {s2/tot:.4f}; "
                  + "; ".join(f"Pr[>{k}] {cnt[k][0]/tot:.3e}, Pr[<-{k}] {cnt[k][1]/tot:.3e} (law {law_tail(k, d):.3e})" for k in cnt))
