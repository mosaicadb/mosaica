"""A4, synthetic only: what the derived level width misses when a cluster has directions of its own.

Part A. The two-term formula rho_true^2 = (1 - beta) rho_g^2 + beta c^2 / r against a direct construction.
Part B. A constructed group: K_g clusters, each with a common covariance (diagonal, spectrum 1/i) holding 1 - beta0 of
        its variance and r own random directions holding beta0. The group basis is the eigenbasis of the pooled
        covariance; the profile is the pooled spectrum; a level is a weighted reverse water-filling at B bits with the
        forward test channel per coordinate (rhat_i = gamma_i (r_i + w_i), so e = A r - Gamma w and an uncoded
        coordinate has e_i = r_i). Every second moment is then exact, no row is sampled. For three kinds of query it
        prints the actual spread over the derived, A3, form 1, form 6 and the cap of form 3.
Part C. Form 2 as a process: rows pass when a + psi * rhohat * h >= t, the rows that pass reveal their error, and
        rhohat is the pooled ratio with the derived value as prior. Prior 2.3 times too narrow.

usage: run-limited.sh 6 1 2 python3 a4_model.py
"""
import math
import numpy as np

rng = np.random.default_rng(4)


def orth(n, r):
    q, _ = np.linalg.qr(rng.normal(size=(n, r)))
    return q


def waterfill(lam, pi, bits):
    w = lam * pi
    lo, hi = 1e-30, w.max()
    for _ in range(200):
        th = math.sqrt(lo * hi)
        b = 0.5 * np.log2(np.maximum(w / th, 1.0)).sum()
        lo, hi = (th, hi) if b > bits else (lo, th)
    return np.minimum(w, th) / pi


print("A. rho_true^2 = (1 - beta) rho_g^2 + beta c^2 / r  (n 512, weighted coordinates, a water-filled with 30% coded)")
n = 512
a = np.where(np.arange(n) < 150, 1.0, np.exp(-(np.arange(n) - 150) / 80.0)); a /= a.sum()
rho_g2 = (a * a).sum()
print(f"   derived rho_g {math.sqrt(rho_g2):.4f}  (1/sqrt n {1 / math.sqrt(n):.4f})")
for beta, r, c2 in ((0.3, 16, 0.6), (0.3, 16, 0.2), (0.5, 8, 0.5), (0.1, 4, 0.9), (0.3, 64, 0.6)):
    got = []
    for _ in range(40):
        V = orth(n, r)
        S = (1 - beta) * np.diag(a) + beta * (V @ V.T) / r          # trace 1
        p = rng.normal(size=n) * np.sqrt(a); p -= V @ (V.T @ p); p /= np.linalg.norm(p)
        s = V @ rng.normal(size=r); s /= np.linalg.norm(s)
        d = math.sqrt(1 - c2) * p + math.sqrt(c2) * s
        got.append(d @ S @ d)
    f = (1 - beta) * rho_g2 + beta * c2 / r
    full = (1 - beta) * ((1 - c2) * rho_g2 + c2 / n) + beta * c2 / r
    print(f"   beta {beta} r {r:2d} c^2 {c2}: actual/derived spread {math.sqrt(np.mean(got) / rho_g2):.2f}   two-term formula {math.sqrt(f / rho_g2):.2f}   with the cross terms {math.sqrt(full / rho_g2):.2f}")

print("\nB. a constructed group (beta0 0.3, r 8 own directions a cluster; exact second moments; 200 queries a kind)")
print("   columns: actual/derived = sqrt(mean rho_true^2)/rho_g | A3/derived | form 1 over actual (sd) and slope b | form 6 over actual | cap of form 3 over actual, over derived | a rank-16 sketch of the weighted error covariance with the mean of the rest, and with the 17th eigenvalue for the rest, over actual")


def group(n, Kg, bits, beta0=0.3, r=8, nq=200):
    common = 1.0 / np.arange(1, n + 1); common /= common.sum()
    own = [orth(n, r) for _ in range(Kg)]
    cov = lambda V: (1 - beta0) * np.diag(common) + beta0 * (V @ V.T) / r
    pooled = sum(cov(V) for V in own) / Kg
    lam, B = np.linalg.eigh(pooled); lam = lam[::-1].copy(); B = B[:, ::-1]
    pi = lam.copy()                                                   # queries distributed as the pooled rows
    nu = waterfill(lam, pi, bits)
    coded = nu < lam * (1 - 1e-9)
    D2 = np.sqrt(nu / pi); av = np.sqrt(nu * pi)
    rho_g2 = (av * av).sum() / av.sum() ** 2
    A = nu / lam; G = 1 - A
    W = np.where(coded, nu * lam / np.maximum(lam - nu, 1e-300), 0.0)
    Sc = B.T @ cov(own[0]) @ B                                        # cluster 0 in the group's coordinates
    SE = (A[:, None] * Sc) * A[None, :] + np.diag(G * W * G)          # error covariance
    Sh = (G[:, None] * (Sc + np.diag(W))) * G[None, :]                # covariance of the coded estimate rhat
    Csh = Sc * G[None, :]                                             # Cov(r, rhat)
    Sw = SE / np.sqrt(D2)[:, None] / np.sqrt(D2)[None, :]             # D^-1 SE D^-1, D = sqrt(D2)
    ev, evec = np.linalg.eigh(Sw); cap2 = ev[-1] / ev.sum()
    top, tv = ev[-16:], evec[:, -16:]; rest_mean, rest_max = ev[:-16].mean(), ev[-17]
    own_unc = float(((B.T @ own[0])[~coded] ** 2).sum() / r)          # share of the own directions' energy left uncoded
    Lc = np.linalg.cholesky(Sc + 1e-12 * np.eye(n))
    out = {}
    for kind in ("as the cluster's rows", "on the profile", "isotropic"):
        t2, d3, f1, f6, bb, sk, sks = [], [], [], [], [], [], []
        for _ in range(nq):
            if kind == "as the cluster's rows":
                d = Lc @ rng.normal(size=n)
            elif kind == "on the profile":
                d = rng.normal(size=n) * np.sqrt(pi)
            else:
                d = rng.normal(size=n) / np.sqrt(D2)
            w2 = (D2 * d * d).sum()
            true = d @ SE @ d
            t2.append(true / (w2 * np.trace(Sw)))
            d3.append((d * d * nu).sum() / (w2 * av.sum()))
            va = d @ Sh @ d
            f1.append((d @ Sc @ d - va) / true)
            bb.append((d @ Csh @ d) / va)
            du = np.where(coded, 0.0, d)
            f6.append((du @ Sc @ du + (np.where(coded, d, 0.0) ** 2 * nu).sum()) / true)
            dw = np.sqrt(D2) * d; pr = tv.T @ dw; restn = dw @ dw - pr @ pr
            sk.append(((top * pr * pr).sum() + rest_mean * restn) / true); sks.append(((top * pr * pr).sum() + rest_max * restn) / true)
        out[kind] = (math.sqrt(np.mean(t2) / rho_g2), math.sqrt(np.mean(d3) / rho_g2), math.sqrt(np.median(f1)), float(np.median(bb)),
                     math.sqrt(np.median(f6)), math.sqrt(cap2 / np.mean(t2)), math.sqrt(cap2 / rho_g2), math.sqrt(np.median(sk)), math.sqrt(np.median(sks)),
                     math.sqrt(np.quantile(f1, 0.1)), math.sqrt(np.quantile(f6, 0.1)), math.sqrt(np.quantile(sk, 0.1)))
    return int(coded.sum()), own_unc, math.sqrt(rho_g2), out


for n, bits in ((256, 64), (256, 128), (512, 64)):
    for Kg in (1, 2, 4, 8, 16, 32):
        nc, ou, rg, out = group(n, Kg, bits)
        print(f"   n {n} bits {bits} clusters in the group {Kg:2d}: coded coordinates {nc:3d}, own directions' energy uncoded {ou:.2f}, derived rho {rg:.4f}")
        for kind, v in out.items():
            print(f"        query {kind:22s}: {v[0]:.2f} | {v[1]:.2f} | {v[2]:.2f} (10th pct {v[9]:.2f}), b {v[3]:.2f} | {v[4]:.2f} (10th pct {v[10]:.2f}) | {v[5]:.2f}, {v[6]:.2f} | sketch {v[7]:.2f} (10th pct {v[11]:.2f}), safe sketch {v[8]:.2f}")

print("\nC. form 2 as a process (one level then the vector; clusters of 2,000 rows; t at 3.5 cluster spreads; level explains 0.8;")
print("   true rho 2.3 times the prior; psi 4.5). Share of the rows above t that pass, rows passed per cluster, rhohat/true after 1, 3, 10, 40 clusters")


def process(m0, b=1.0, clusters=400, rows=2000, psi=4.5, seed_rows=28, ucb=0.0, fixed=None):
    r2 = np.random.default_rng(9)
    theta, g = 0.8, 3.5
    va = theta / (2 * b - 1)                         # Var(a) so that Var(s) = 1 with slope b
    tau2 = 1 - (2 * b - 1) * va - (b - 1) ** 2 * va  # conditional error variance given a (unit spread of scores)
    tau2 = max(tau2, 1e-6) + 0.0
    tau2 = 1 - b * b * va                            # Var(s | a) = 1 - b^2 Var(a)
    t = g
    rho_true = math.sqrt(tau2 + (b - 1) ** 2 * va)   # rms of the error over all rows, in units of h (E u^2 = 1)
    rho0 = rho_true / 2.3
    num, den = m0 * rho0 ** 2, float(m0)
    hit = tot = passed = 0
    traj = {}
    for c in range(clusters):
        x = r2.normal(0, math.sqrt(va), rows)
        u = np.exp(r2.normal(-0.045, 0.3, rows))     # the row's scale, E u^2 = 1
        eps = (b - 1) * x + r2.normal(0, math.sqrt(tau2), rows) * u
        s = x + eps
        rh = fixed * rho_true if fixed else math.sqrt(num / den) * (1 + ucb / math.sqrt(max(den, 1.0)))
        if c == 0 and seed_rows:
            go = np.zeros(rows, bool); go[np.argsort(-x)[:seed_rows]] = True
        else:
            go = x + psi * rh * u >= t
        num += (eps[go] ** 2).sum(); den += (u[go] ** 2).sum()
        if c > 0:
            hit += int((go & (s >= t)).sum()); tot += int((s >= t).sum()); passed += int(go.sum())
        if c + 1 in (1, 3, 10, 40):
            traj[c + 1] = math.sqrt(num / den) / rho_true
    return hit / max(tot, 1), passed / (clusters - 1), traj, tot


for b in (1.0, 1.09, 1.3):
    print(f"   slope b {b}:")
    for name, kw in (("width at the true rms (oracle)", dict(m0=1, fixed=1.0)), ("derived prior, never updated", dict(m0=1, fixed=1 / 2.3)),
                     ("form 2, prior weight 4 rows", dict(m0=4)), ("form 2, prior weight 16 rows", dict(m0=16)), ("form 2, prior weight 64 rows", dict(m0=64)),
                     ("form 2, weight 16, no seed from the first cluster", dict(m0=16, seed_rows=0)),
                     ("form 2, weight 16, upper margin 1/sqrt(rows)", dict(m0=16, ucb=1.0))):
        h, p, tr, tot = process(b=b, **kw)
        print(f"      {name:48s}: pass share {h:.3f} of {tot}, rows passed {p:7.1f}, rhohat/true " + ", ".join(f"{k}: {v:.2f}" for k, v in tr.items()))
