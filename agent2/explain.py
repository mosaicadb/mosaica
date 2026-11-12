# Which terms of the deterministic bound blow up for text queries. Pairs = (query, true match at answer size 2000).
import numpy as np, json
from harness import *
I = load(); X, Z, Mean, grp, V, lam, assign, Qi = (I[k] for k in ("X", "Z", "Mean", "grp", "V", "lam", "assign", "Qi"))
P = json.load(open("text_q.json")); T = np.load("text_q.npy"); qi = np.load("item_q_idx.npy"); Ttr = np.load("text_train.npy")
Qt = text_profile(I, Ttr); thr = np.load("thr_all.npy"); rng = np.random.default_rng(3)
sets = {"item": (X[qi], thr[:100]), "text": (T, thr[100:])}
# score density around the threshold
for nm, (Q, th) in sets.items():
    S = X @ Q.T
    print(nm, "items within +-w of the threshold per true match (answer sizes 20/200/2000/10000):")
    for w in (0.005, 0.01, 0.02, 0.04, 0.08):
        print(f"   w={w}: " + " ".join(f"{(np.abs(S - th[None, :, ti]) < w).sum() / (S >= th[None, :, ti]).sum():8.2f}" for ti in range(4)))
    print("   score std over items:", round(float(S.std(0).mean()), 4), " (top-200 threshold - median):", round(float((th[:, 1] - np.median(S, axis=0)).mean()), 4))
LV = [0, 32, 96, 192, 384]
for nm, (Q, th) in sets.items():
    acc = {L: np.zeros(12) for L in LV}; cnt = 0
    for j in range(0, len(Q), 3):
        s = X @ Q[j]; mi = np.where(s >= th[j, 2])[0]; mi = rng.choice(mi, 60, replace=False)
        for g in np.unique(grp[assign[mi]]):
            ii = mi[grp[assign[mi]] == g]; z = np.asarray(Z[np.sort(ii)]); ii = np.sort(ii)
            D = (Q[j][None] - Mean[assign[ii]]) @ V[g]; sig = np.sqrt(lam[g]).astype(np.float32)
            for L in LV:
                bits = alloc(lam[g], Qi[g], L * 8)
                if L == 0: e = z
                else:
                    code = quant(z / sig, bits) * sig; gam = (z * code).sum(1) / ((code * code).sum(1) + 1e-20); e = z - gam[:, None] * code
                di = ((lam[g] * GT[bits] + 1e-10) / (Qi[g] + 1e-10)) ** 0.25; dt = ((lam[g] * GT[bits] + 1e-10) / (Qt[g] + 1e-10)) ** 0.25
                plain = np.linalg.norm(D, axis=1) * np.linalg.norm(e, axis=1)
                wi = np.linalg.norm(D * di, axis=1) * np.linalg.norm(e / di, axis=1); wt = np.linalg.norm(D * dt, axis=1) * np.linalg.norm(e / dt, axis=1)
                l1 = (np.abs(D) * np.abs(e)).sum(1); unc = bits == 0
                rmsb = (np.abs(D) * np.sqrt(lam[g] * GT[bits])).sum(1)
                acc[L] += [np.linalg.norm(D, axis=1).sum(), np.linalg.norm(e, axis=1).sum(), plain.sum(), wi.sum(), wt.sum(), np.minimum(plain, wi).sum(), l1.sum(),
                           (np.abs(D) * np.abs(e))[:, unc].sum(), np.abs((D * e).sum(1)).sum(), (D[:, unc] ** 2).sum(), (D ** 2).sum(), rmsb.sum()]
            cnt += len(ii)
    print(f"\n{nm}: {cnt} (query, true match) pairs; bits and D from the item-fitted profile")
    print("  bytes  |D|    |e|    plain  w_item w_text min(pl,wi) pairwise-l1 (uncoded share)  rms-l1  actual|D.e|  D-energy in uncoded dims")
    for L in LV:
        a = acc[L] / cnt
        print(f"  {L:4d}  {a[0]:.3f}  {a[1]:.3f}  {a[2]:.4f} {a[3]:.4f} {a[4]:.4f} {a[5]:.4f}     {a[6]:.4f} ({a[7]/a[6]:.2f})            {a[11]:.4f}  {a[8]:.5f}     {acc[L][9]/acc[L][10]:.2f}")
