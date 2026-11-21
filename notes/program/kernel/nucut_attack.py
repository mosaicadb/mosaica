"""Attack 2: Proposition 1.6 as built (NuCut, nu_range in rust/src/v2/q.rs; sort_by_nu, quant_basis, cluster_info and
the bmin/bmax of build2 in mod.rs), in float64 from a reading of the code. For each (cluster, query, threshold):
no row decided against its exact score, and every row's exact score inside the bounds of its stored norm code.
usage: python3 nucut_attack.py [seed] [groups]
"""
import math
import sys
import numpy as np
import range_model as rm

seed = int(sys.argv[1]) if len(sys.argv) > 1 else 1
ngrp = int(sys.argv[2]) if len(sys.argv) > 2 else 30
rng = np.random.default_rng(seed)
DA = rm.DA


def unit(v):
    return v / np.linalg.norm(v, axis=-1, keepdims=True)


def quant_basis(V):
    n = V.shape[0]
    vsc = np.abs(V).max(axis=0) / 127.0
    v8 = np.clip(np.round(V / vsc), -127, 127)
    vq = v8 * vsc                                   # columns are the basis vectors
    X = np.linalg.inv(vq)
    for _ in range(3):
        X = X @ (2 * np.eye(n) - vq @ X)
    e_inv = np.linalg.norm(vq @ X - np.eye(n)) * (1 + 1e-6) + 1e-12
    return vq, X, e_inv


tot = dict(checks=0, rows=0, decided=0, wrong=0, wrong_unclamped=0, outside=0, outside_unclamped=0, skip=0, allin=0)
ex = []
for g in range(ngrp):
    n = int(rng.choice([4, 8, 16, 32]))
    A = rng.standard_normal((n, n))
    Qo, _ = np.linalg.qr(A)
    V = Qo @ np.diag(10 ** rng.uniform(-0.7, 0.7, n)) @ (np.eye(n) + rng.uniform(0, 0.6) * rng.standard_normal((n, n)) / math.sqrt(n))
    vq, Xinv, e_inv = quant_basis(V)
    for k in range(3):
        cen = unit(rng.standard_normal(n))
        nc = int(rng.choice([24, 100]))
        X = unit(cen + rng.choice([0.05, 0.5, 2.0]) * rng.standard_normal((nc, n)) / math.sqrt(n))
        X[0] = cen
        X[1] = X[2]
        X = rm.f16(X)
        mean = X.mean(axis=0)
        sc = np.abs(mean).max() / 127.0
        m = (np.round(mean / sc) * sc).astype(np.float32).astype(np.float64)
        mn = math.sqrt(float(m @ m))
        mhat = m / mn
        a = X @ mhat
        key = np.round(np.clip(a, -1, 1) * 32767.0)
        x2 = 1.0 + max(float(np.max(np.sum(X * X, axis=1) - 1.0)), 0.0) * (1 + 1e-9) + 1e-12
        z = (X - m) @ Xinv.T
        nu = np.sqrt(np.sum(z * z, axis=1)) * (1 + 1e-9) + 1e-12
        o = np.argsort(nu, kind="stable")
        X, a, key, nu = X[o], a[o], key[o], nu[o]
        numax = max(float(nu.max()) * (1 + 1e-12), 1e-30)
        code = np.minimum(np.ceil(nu / numax * 65535.0), 65535.0)
        nus = numax * code / 65535.0
        rmax = float(np.sqrt(np.maximum(x2 + mn * mn - 2 * mn * (key / 32767.0 - DA), 0.0)).max())
        mr = mn * (key / 32767.0) - mn * mn
        bmax, bmin = float(mr.max()) + mn * DA + 1e-12, float(mr.min()) - mn * DA - 1e-12
        mv = vq.T @ m
        mv32 = mv.astype(np.float32).astype(np.float64)
        mvn = math.sqrt(float(mv @ mv))
        clamped = np.abs(a) > 1.0 + DA
        qs = [unit(rng.standard_normal(n)) for _ in range(4)] + [mhat, -mhat, X[3].copy(), -X[4], unit(mhat + 1e-5 * rng.standard_normal(n)), m / np.abs(m).max()]
        w = rng.standard_normal(n)
        qs.append(unit(w - (w @ mhat) * mhat))
        mi = np.round(m / np.abs(m).max() * 127.0)                 # the centre's int8 vector, exact in f16
        qs.append(mi * 2.0 ** -math.ceil(math.log2(np.linalg.norm(mi))))
        for q in qs:
            q = rm.f16(q)
            qn = math.sqrt(float(q @ q))
            qv = vq.T @ q
            qm = float(q @ m)
            qq = float(qv @ qv)
            qmv = float(qv @ mv32)
            mm = float(mv32 @ mv32)
            al = []
            for alpha in (qmv / max(mm, 1e-300), qm / (mn * mn), 0.0):
                d = math.sqrt(max(qq - 2 * alpha * qmv + alpha * alpha * mm, 0.0)) * (1 + 1e-12) + 1e-12 * math.sqrt(qq) + abs(alpha) * mvn * 2.0 ** -22
                e = e_inv * (qn + abs(alpha) * mn) * rmax + 1e-9
                al.append((alpha, d, e))
            s = rm.exact_scores(q, X)
            lo = np.full(nc, -np.inf)
            hi = np.full(nc, np.inf)
            for alpha, d, e in al:
                b0, b1 = alpha * bmin, alpha * bmax
                hi = np.minimum(hi, qm + max(b0, b1) + d * nus + e)
                lo = np.maximum(lo, qm + min(b0, b1) - d * nus - e)
            out = (s > hi) | (s < lo)
            tot["outside"] += int(out.sum())
            tot["outside_unclamped"] += int((out & ~clamped).sum())
            ts = list(rng.choice(s, size=6, replace=False))
            ts += [np.nextafter(x, 2.0) for x in ts[:3]] + [np.nextafter(x, -2.0) for x in ts[:3]]
            ts += [float(s.max()), float(s.min()), 0.0, qn, -qn, float(rng.uniform(-1, 1)), float(lo.max()), float(hi.min())]
            for t in ts:
                t = float(t)
                co, cin = -math.inf, -math.inf
                for alpha, d, e in al:
                    b0, b1 = alpha * bmin, alpha * bmax
                    co = max(co, (t - qm - max(b0, b1) - e) / max(d, 1e-300))
                    cin = max(cin, (qm + min(b0, b1) - e - t) / max(d, 1e-300))
                kout = int(np.searchsorted(nus, co - 1e-9 * (1 + abs(co)), side="left"))
                dcs = np.zeros(nc, dtype=int)
                if kout > 0:
                    dcs[:kout] = -1
                else:
                    kin = int(np.searchsorted(nus, cin - 1e-9 * (1 + abs(cin)), side="right"))
                    dcs[:kin] = 1
                tot["checks"] += 1
                tot["rows"] += nc
                tot["decided"] += int(np.count_nonzero(dcs))
                tot["skip"] += kout == nc
                tot["allin"] += kout == 0 and int(dcs.sum()) == nc
                isin = s >= t
                for i in np.nonzero((np.abs(s - t) < 1e-12) & (dcs != 0))[0]:
                    isin[i] = rm.ge_exact(q, X[i], t)
                bad = ((dcs == 1) & ~isin) | ((dcs == -1) & isin)
                if bad.any():
                    tot["wrong"] += int(bad.sum())
                    tot["wrong_unclamped"] += int((bad & ~clamped).sum())
                    if len(ex) < 4:
                        i = int(np.nonzero(bad)[0][0])
                        ex.append((n, t, float(s[i]), int(dcs[i]), float(a[i]), float(lo[i]), float(hi[i])))
print("seed", seed, tot)
for e in ex:
    print("WRONG (n, t, score, decision, true key, lower, upper):", e)
