# Split (randomised) certificate of ../exp8.py on item and real text queries. Variants differ in the energy profile used for
# the coordinate scaling D0 = (lam/Q)^(1/4) and the bit allocation: item-fitted (existing) or mixture-fitted.
import numpy as np, json, sys
from harness import *
rng = np.random.default_rng(0)
I = load(); X, Z, Mean, grp, V, lam, assign, Qi = (I[k] for k in ("X", "Z", "Mean", "grp", "V", "lam", "assign", "Qi"))
P = json.load(open("text_q.json")); T = np.load("text_q.npy"); qi = np.load("item_q_idx.npy"); Ttr = np.load("text_train.npy")
Q = np.concatenate([X[qi], T]); cls = np.array(["item"] * len(qi) + ["text"] * len(T)); thr = thresholds(X, Q)
Qt = text_profile(I, Ttr); Qmix = ((np.sqrt(Qi) + np.sqrt(Qt)) / 2) ** 2
BUD = [32, 96, 192, 384]; C = 8.0; NMIN = 24; N, n = X.shape; NQ = len(Q); G = len(V); NT = thr.shape[1]
def alloc_mse(gam, B):
    b = np.zeros(len(gam), int); val = gam * GT[b]
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - gam * GT[nb]; gain[b >= 8] = -1
        i = int(gain.argmax()); b[i] += 1; val[i] = gam[i] * GT[b[i]]
    return b
variants = {"split-item": Qi, "split-mix": Qmix}
U = {v: np.zeros((len(BUD), NT, NQ)) for v in variants}; H = {v: np.zeros((len(BUD), NT, NQ)) for v in variants}
Hparts = {v: np.zeros((len(BUD), 3, 2)) for v in variants}     # mean of (small-block det, c*sd, uncoded det) over true matches at size 2000, by class
A_ = np.zeros((NT, NQ)); viol = {v: 0 for v in variants}; sigmax = {v: np.zeros(2) for v in variants}; nblocks = {v: np.zeros(len(BUD)) for v in variants}
big = {v: np.zeros((2, 3)) for v in variants}                  # pairs with |err|/sd above 4, 5, 6
qm = Q @ Mean.T; isT = cls == "text"
for g in range(G):
    ks = np.where(grp == g)[0]; kpos = -np.ones(len(grp), int); kpos[ks] = np.arange(len(ks))
    QV = Q @ V[g]; MV = Mean[ks] @ V[g]; Dl2 = (QV[None] - MV[:, None]) ** 2
    spl = {}
    for v, Qp in variants.items():
        Dt = (((lam[g] + 1e-10) / (Qp[g] + 1e-10)) ** 0.25).astype(np.float32); QVt = QV * Dt; MVt = MV * Dt; Dl2t = Dl2 * Dt ** 2; lv = []
        for B in BUD:
            bits = alloc_mse(lam[g] * Qp[g], B * 8); blocks = []
            for d in np.unique(bits):
                idx = np.where(bits == d)[0]
                if d == 0: continue
                R = np.linalg.qr(rng.standard_normal((len(idx), len(idx))))[0].astype(np.float32) if len(idx) > 1 else np.ones((1, 1), np.float32)
                blocks.append((int(d), idx, R, np.sqrt(Dl2t[:, :, idx].sum(2)), QVt[:, idx] @ R.T, MVt[:, idx] @ R.T))
            i0 = np.where(bits == 0)[0]; lv.append((blocks, i0, np.sqrt(Dl2t[:, :, i0].sum(2))))
        spl[v] = (Dt, lv)
    items = np.where(grp[assign] == g)[0]
    for c0 in range(0, len(items), 4000):
        ii = items[c0:c0+4000]; x = X[ii]; a = assign[ii]; ap = kpos[a]; z = np.asarray(Z[ii]); mva = MV[ap]
        true = x @ Q.T; base2 = qm[:, a].T + (mva * z).sum(1)[:, None]
        Am = true[:, None, :] >= thr.T[None]; A_ += Am.sum(0) / len(variants)
        for v, (Dt, lv) in spl.items():
            zt = z / Dt; und = np.ones((len(ii), NT, NQ), bool)
            for li, (blocks, i0, dn0) in enumerate(lv):
                est = base2.copy(); hdet = np.zeros_like(true); var = np.zeros_like(true); rerr = np.zeros_like(true)
                for d, idx, R, dnb, QR_, MR_ in blocks:
                    nb = len(idx); y = zt[:, idx] @ R.T; l2 = np.linalg.norm(y, axis=1) + 1e-20
                    cy = quant(y / l2[:, None] * np.sqrt(nb), np.full(nb, d)); num = (cy * y).sum(1); cn = np.linalg.norm(cy, axis=1)
                    cosf = np.clip(num / (cn * l2), 1e-6, 1.0); Ab = l2 * np.sqrt(1 - cosf ** 2) / cosf; mr = MR_[ap]
                    eb = (l2 * l2 / num)[:, None] * (cy @ QR_.T - (cy * mr).sum(1)[:, None]); est += eb
                    if nb >= NMIN:
                        var += (Ab[:, None] * dnb[ap]) ** 2 / (nb - 1); rerr += eb - (y @ QR_.T - (y * mr).sum(1)[:, None])
                    else: hdet += Ab[:, None] * dnb[ap]
                if c0 == 0: nblocks[v][li] = max(nblocks[v][li], len(blocks))
                h0 = dn0[ap] * np.linalg.norm(zt[:, i0], axis=1)[:, None]; sd = np.sqrt(var); ratio = np.abs(rerr) / np.maximum(sd, 1e-12)
                for ci, m in enumerate((~isT, isT)):
                    sigmax[v][ci] = max(sigmax[v][ci], float(ratio[:, m].max())); big[v][ci] += [(ratio[:, m] > s).sum() for s in (4, 5, 6)]
                    Hparts[v][li, :, ci] += [(hdet * Am[:, 2])[:, m].sum(), (C * sd * Am[:, 2])[:, m].sum(), (h0 * Am[:, 2])[:, m].sum()]
                h = (hdet + C * sd + h0) * 1.0005 + 5e-5; ub = est + h; lb = est - h
                for ti in range(NT):
                    t = thr[None, :, ti]; A = Am[:, ti]; sv = ub >= t; win = lb >= t
                    viol[v] += int((A & ~sv).sum() + (win & ~(true >= t - 5e-5)).sum())
                    und[:, ti] &= sv & ~win; U[v][li, ti] += und[:, ti].sum(0); H[v][li, ti] += (h * A).sum(0)
    log("group", g, "violations", viol, "max sigma (item, text)", {v: np.round(s, 2).tolist() for v, s in sigmax.items()})
print("pairs per level", N * NQ, "violations", viol, "max |error|/predicted sd (item, text)", {v: s.tolist() for v, s in sigmax.items()})
print("pairs above 4/5/6 sd, rows item,text:", {v: b.tolist() for v, b in big.items()})
for v in variants:
    print("\n==", v, "c =", C, " blocks per level", nblocks[v].tolist())
    for nm, m in (("item", ~isT), ("text", isT)):
        A = A_[:, m].sum(1); ci = int(nm == "text")
        for li, B in enumerate(BUD):
            hp = Hparts[v][li, :, ci] / A[2]
            print(f"  {nm} {B:3d} B undecided/match: " + " ".join(f"{x:8.2f}" for x in U[v][li][:, m].sum(1) / A) + "   half-width: " + " ".join(f"{x:.4f}" for x in H[v][li][:, m].sum(1) / A)
                  + f"   parts at size 2000 (small-block det, c*sd, uncoded det): {hp[0]:.4f} {hp[1]:.4f} {hp[2]:.4f}")
        for extra in (0, 2):
            cost = (SCAL + BUD[0] + extra * nblocks[v][0]) * N + np.zeros_like(U[v][0])
            for li in range(1, len(BUD)): cost += (BUD[li] - BUD[li - 1] + extra * nblocks[v][li]) * U[v][li - 1]
            cost += FULL * U[v][-1]
            print(f"  {nm} read % (levels 32/96/192/384 then full; {extra} B of scalars per block): " + " ".join(f"{100 * x:6.2f}" for x in (cost / (N * FULL))[:, m].mean(1)))
np.savez("res3.npz", cls=cls, A=A_, **{f"U_{v}": U[v] for v in variants}, **{f"H_{v}": H[v] for v in variants})
