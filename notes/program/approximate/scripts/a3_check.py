"""A3, a rough check without the index's codes: the per-query spread factor against the profile's.

rho_l(q, c)^2 / rho_{g,l}^2 = E_omega[a] / E_a[a], a_c = sqrt(v_c pi_c), omega_c = D_c^2 Delta_c^2 (CLAIMS.md A3.1).
The level's error variances v_c are NOT the build's here: they are a weighted reverse water-filling at the level's bit
budget (v_c pi_c = theta where lam_c pi_c > theta, else v_c = lam_c), which stands in for `det_alloc_tree`. So this
checks the sign and rough size of the correction, not the build's number. Delta = V_g^T (q - alpha m_c), alpha = q.m/|m|^2.

usage: python3 a3_check.py DATASET
"""
import sys, math
import numpy as np

SUITE = "/home/joe/code/vexact-data/suite"
ds = sys.argv[1]
fit, prep = f"{SUITE}/fit/{ds}", f"{SUITE}/prep/{ds}"
V = np.load(fit + "/V.npy", mmap_mode="r"); lam = np.load(fit + "/lam.npy"); prof = np.load(fit + "/profile.npy")
grp = np.load(fit + "/grp.npy"); mq8 = np.load(fit + "/mq8.npy"); msc = np.load(fit + "/msc.npy"); cstart = np.load(fit + "/cstart.npy")
Xs = np.load(fit + "/Xs.npy", mmap_mode="r")
Q = np.load(prep + "/queries.npy"); Q = Q / np.linalg.norm(Q, axis=1, keepdims=True)
G, n = lam.shape; K = len(grp)
Qp = np.zeros((Q.shape[0], n)); Qp[:, :Q.shape[1]] = Q
rng = np.random.default_rng(3)
rows = rng.choice(Xs.shape[0], 100, replace=False)
P = np.asarray(Xs[np.sort(rows)], dtype=np.float64); P /= np.linalg.norm(P, axis=1, keepdims=True)
M = mq8.astype(np.float64) * msc[:, None]


def waterfill(l, p, bits):
    w = l * p
    lo, hi = 1e-30, w.max()
    for _ in range(200):
        th = math.sqrt(lo * hi)
        b = 0.5 * np.log2(np.maximum(w / th, 1.0)).sum()
        lo, hi = (th, hi) if b > bits else (lo, th)
    return np.minimum(w, th) / p            # v_c


for nbytes in (16, 32):
    rho_g = []; a_all = []; D2_all = []
    for g in range(G):
        v = waterfill(lam[g], prof[g] + 1e-10, 8 * nbytes)
        a = np.sqrt(v * (prof[g] + 1e-10))
        rho_g.append(math.sqrt((a * a).sum()) / a.sum()); a_all.append(a); D2_all.append(np.sqrt(v / (prof[g] + 1e-10)))
    print(f"{ds}  level {nbytes} B: rho_g under water-filling, min / median / max over groups: {min(rho_g):.4f} / {np.median(rho_g):.4f} / {max(rho_g):.4f};  1/sqrt(n) {1 / math.sqrt(n):.4f}")
    for name, QQ in (("test queries", Qp), ("corpus rows as queries", P)):
        ratios = []
        for i in range(QQ.shape[0]):
            q = QQ[i]
            mu = M @ q
            for c in np.argsort(-mu)[:10]:
                g = grp[c]
                alpha = mu[c] / (M[c] @ M[c])
                d = (q - alpha * M[c]) @ np.asarray(V[g], dtype=np.float64)
                om = D2_all[g] * d * d
                a = a_all[g]
                ratios.append(math.sqrt((om * a).sum() / om.sum() / ((a * a).sum() / a.sum())))
        r = np.array(ratios)
        print(f"   {name}: rho(q,c)/rho_g over (query, its 10 best clusters by centre score): 5 / 50 / 95 pct {np.quantile(r, .05):.3f} / {np.median(r):.3f} / {np.quantile(r, .95):.3f}; rms {math.sqrt((r * r).mean()):.3f}")
