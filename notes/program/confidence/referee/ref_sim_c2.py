import sys; sys.argv = ['x']
import numpy as np
import ref_sim_c as R
R.rng = np.random.default_rng(99)
n, N, delta, batch = 13, 20, 0.01, 2000
q = R.qd(n - 1, delta); s = 0.3
C64 = R.unit(R.rng.standard_normal((64, n)))
for kind, code, trials in (('sign', R.signcode, 400000), ('M64', lambda z: R.nearest(z, C64), 400000)):
    Kr = []; Km = []
    for _ in range(trials // batch):
        Q = R.haar(batch, n); e1 = np.zeros((batch, n)); e1[:, 0] = 1
        y = s * e1[:, None, :] + np.sqrt(1 - s * s) * R.perp_unit(batch, N, e1)
        z = np.einsum('bij,bkj->bki', Q, y)
        Kr.append(R.fail_plain(z, Q[:, :, 0], s, q, code).sum(1))
        Dp = R.unit(R.rng.standard_normal((batch, n)))
        z = s * Dp[:, None, :] + np.sqrt(1 - s * s) * R.perp_unit(batch, N, Dp)
        Km.append(R.fail_plain(z, Dp, s, q, code).sum(1))
    Kr = np.concatenate(Kr).astype(float); Km = np.concatenate(Km).astype(float)
    a = R.stats(Kr, N); b = R.stats(Km, N)
    print(kind, "real", [round(float(x), 5) for x in a], "model", [round(float(x), 5) for x in b], "se(Pr[K>=1])", round(float(np.sqrt(a[2]*(1-a[2])/trials)), 5))
