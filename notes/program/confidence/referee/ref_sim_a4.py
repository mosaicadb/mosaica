"""Referee simulation of A1 / N1(b) / A4 / pivot with real codebooks. n = d+1 ambient dims.
Sampling: a full Haar matrix by QR (sign-fixed) for the 'qr' runs, else a random orthonormal 2-frame."""
import numpy as np, sys
from scipy.special import betainc, betaincinv

def T(d, x):
    x = np.asarray(x, float); ax = np.clip(np.abs(x), 0, 1)
    t = 0.5 * betainc((d - 1) / 2, 0.5, 1 - ax * ax)
    return np.where(x >= 0, t, 1 - t)
def qd(d, delta): return float(np.sqrt(1 - betaincinv((d - 1) / 2, 0.5, 2 * delta)))

rng = np.random.default_rng(20261121)

def frames(n, batch, use_qr):
    if use_qr:
        G = rng.standard_normal((batch, n, n))
        Q, R = np.linalg.qr(G)
        Q = Q * np.sign(np.einsum('bii->bi', R))[:, None, :]
        return Q[:, :, 0], Q[:, :, 1]          # Rot y , Rot v  for y=e1, v=e2
    u = rng.standard_normal((batch, n)); u /= np.linalg.norm(u, axis=1, keepdims=True)
    v = rng.standard_normal((batch, n)); v -= (v * u).sum(1, keepdims=True) * u
    v /= np.linalg.norm(v, axis=1, keepdims=True)
    return u, v

def make_code(kind, n):
    if kind == 'sign':   # hypercube: code is sign(z)/sqrt(n)
        return lambda z: np.sign(z) / np.sqrt(n)
    if kind == 'grid2':  # 2-bit scalar quantiser on each coordinate, fixed grid, then normalised
        lev = np.array([-1.5, -0.5, 0.5, 1.5]) / np.sqrt(n)
        def f(z):
            idx = np.abs(z[:, :, None] - lev[None, None, :]).argmin(2)
            c = lev[idx]; return c / np.linalg.norm(c, axis=1, keepdims=True)
        return f
    if kind == 'tern':   # ternary {-1,0,1} with a dead zone; all-zero mapped to e1
        def f(z):
            c = np.sign(z) * (np.abs(z) > 0.6 / np.sqrt(n))
            nz = np.linalg.norm(c, axis=1, keepdims=True); c = np.where(nz > 0, c / np.maximum(nz, 1e-12), 0); c[nz[:, 0] == 0, 0] = 1
            return c
        return f
    M = int(kind[1:])
    C = rng.standard_normal((M, n)); C /= np.linalg.norm(C, axis=1, keepdims=True)
    return lambda z: C[(z @ C.T).argmax(1)]

def run(n, kind, delta, s_list, trials, use_qr=False, batch=20000):
    d = n - 1
    q1, q2 = qd(d, delta), qd(d, delta / 2); qn = qd(d + 1, delta)
    code = make_code(kind, n)
    print(f"\nn={n} d={d} code={kind} delta={delta} q_d(delta)={q1:.4f} q_d(delta/2)={q2:.4f} qr={use_qr} trials={trials} se~{np.sqrt(delta/trials):.5f}")
    print("   s      plain1  plain2  A4_1    A4_1pred A4_2    piv2    pivUp   pivUpCap naive_sin  A4up>plainup  meank   Pr[cos<=0]")
    for s in s_list:
        acc = np.zeros(10); ksum = 0; neg = 0
        for _ in range(trials // batch):
            z, v = frames(n, batch, use_qr)
            Dl = s * z + np.sqrt(1 - s * s) * v
            x0 = code(z)
            cosf = (z * x0).sum(1); E0 = (Dl * x0).sum(1)
            ok = cosf > 1e-12; neg += np.count_nonzero(~ok)
            cs = np.where(ok, cosf, 1.0); sinf = np.sqrt(np.clip(1 - cs ** 2, 0, 1)); tanf = sinf / cs
            Eh = E0 / cs
            res = []
            for q in (q1, q2):
                # plain, with the A1 fallback (0, 1) when q tan >= 1 or cos<=0
                fb = (~ok) | (q * tanf >= 1)
                est = np.where(fb, 0.0, Eh); hw = np.where(fb, 1.0, q * tanf)
                res.append((est, hw))
            (ep1, hp1), (ep2, hp2) = res
            def a4(q):
                k = q * q * tanf ** 2
                est = np.where(ok, Eh / (1 + k), 0.0); hw = np.where(ok, q * tanf / np.sqrt(1 + k), 1.0)
                return est, hw, k
            ea1, ha1, k1 = a4(q1); ea2, ha2, k2 = a4(q2)
            ksum += k1[ok].sum()
            # pivot interval
            def piv(q):
                k = q * q * tanf ** 2
                disc = k * (1 + k - Eh ** 2)
                c = Eh / (1 + k); h = np.sqrt(np.maximum(disc, 0)) / (1 + k)
                return c, h, disc, k
            c2, h2, disc2, _ = piv(q2); c1, h1, disc1, _ = piv(q1)
            acc[0] += np.count_nonzero(s - ep1 > hp1)                 # plain one-sided
            acc[1] += np.count_nonzero(np.abs(s - ep2) > hp2)         # plain two-sided
            acc[2] += np.count_nonzero(s - ea1 > ha1)                 # A4 one-sided
            # exact prediction for A4 one-sided given phi
            a = -k1 / (1 + k1); b = tanf / (1 + k1)
            den = np.sqrt(1 - s * s) * b
            pred = np.where(ok & (den > 0), T(d, (ha1 + s * a) / np.maximum(den, 1e-300)), 0.0)
            acc[3] += pred.sum()
            acc[4] += np.count_nonzero(np.abs(s - ea2) > ha2)         # A4 two-sided
            acc[5] += np.count_nonzero(ok & ((disc2 < 0) | (np.abs(s - c2) > h2)))   # pivot two-sided (cos<=0: whole [-1,1])
            up = c1 + h1
            acc[6] += np.count_nonzero(ok & ((disc1 < 0) & (s > c1) | (disc1 >= 0) & (s > up)))  # pivot upper end, uncapped
            upc = np.where(Eh >= 1, 1.0, np.where(disc1 >= 0, np.minimum(1.0, up), 1.0))       # R8 / Thm 3.11 repaired
            acc[7] += np.count_nonzero(ok & (s > upc))
            acc[8] += np.count_nonzero(ok & (s - cs * E0 > q1 * sinf))  # naive shrunk, half-width q sin phi
            acc[9] += np.count_nonzero(ea1 + ha1 > ep1 + hp1 + 1e-12)  # how often A4's upper end is above the plain one
        t = (trials // batch) * batch
        r = acc / t
        print(f"  {s:+.3f}  {r[0]:.4f}  {r[1]:.4f}  {r[2]:.4f}  {r[3]:.4f}   {r[4]:.4f}  {r[5]:.4f}  {r[6]:.4f}  {r[7]:.4f}   {r[8]:.4f}     {r[9]:.3f}        {ksum/max(t-neg,1):.3f}   {neg/t:.3f}")

S = [0.0, 0.3, 0.6, 0.9, 0.99, 0.999, -0.3, -0.9, -0.999]
which = sys.argv[1]
if which == '1':
    run(8, 'sign', 0.01, S, 400000, use_qr=True)
    run(8, 'M16', 0.01, S, 400000)
    run(8, 'M1', 0.01, S, 400000)
if which == '2':
    run(13, 'M512', 0.01, S, 400000)
    run(13, 'grid2', 0.01, S, 400000)
    run(5, 'tern', 0.02, S, 400000, use_qr=True)
    run(4, 'M2', 0.02, S, 400000)
if which == '3':   # N2: averaged requirement, Delta uniform and independent of y
    for n, kind, delta in ((8, 'sign', 0.01), (13, 'M512', 0.01), (8, 'M16', 0.01), (5, 'tern', 0.02)):
        d = n - 1; qn = qd(d + 1, delta); q = qd(d, delta); code = make_code(kind, n)
        f = np.zeros(4); t = 0
        for _ in range(20):
            z = rng.standard_normal((20000, n)); z /= np.linalg.norm(z, axis=1, keepdims=True)
            Dl = rng.standard_normal((20000, n)); Dl /= np.linalg.norm(Dl, axis=1, keepdims=True)
            x0 = code(z); cosf = (z * x0).sum(1); sinf = np.sqrt(np.clip(1 - cosf ** 2, 0, 1)); E0 = (Dl * x0).sum(1); s = (Dl * z).sum(1)
            f[0] += np.count_nonzero(s - cosf * E0 > qn * sinf)             # naive shrunk, q_{d+1} sin phi: exactly delta on average
            f[1] += np.count_nonzero(s - cosf * E0 > q * sinf * cosf)       # with sin cos
            f[2] += np.count_nonzero(s - E0 / cosf > q * sinf / cosf)       # plain
            f[3] += (qn * sinf).sum() / (q * sinf / cosf).sum() * 20000
            t += 20000
        print(f"N2 averaged: n={n} code={kind} delta={delta}: naive(q_(d+1) sin)={f[0]/t:.4f}  naive(q sin cos)={f[1]/t:.4f}  plain={f[2]/t:.4f}  width ratio naive/plain~{f[3]/t:.3f}")
