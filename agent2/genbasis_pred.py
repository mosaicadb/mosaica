# Predicted per-query matched l1 bound  sum_i |Delta_i| sqrt(var(e_i))  in three coding bases, on held-out queries:
#   pca  : local PCA basis (existing), bits from a diagonal profile
#   gen-X: basis that diagonalises both the residual covariance and the query-offset covariance C_Q of X (X = text, item, mix)
import numpy as np, json
from harness import *
I = load(); X, Mean, grp, V, lam, assign, Qi, Chat = (I[k] for k in ("X", "Mean", "grp", "V", "lam", "assign", "Qi", "Chat"))
T = np.load("text_q.npy"); qi = np.load("item_q_idx.npy"); Ttr = np.load("text_train.npy"); thr = np.load("thr_all.npy"); rng = np.random.default_rng(3)
G, n = lam.shape; Qt = text_profile(I, Ttr)
ptr = rng.choice(len(X), 5000, replace=False); ptr = np.setdiff1d(ptr, qi); pk = np.argsort(-(X[ptr] @ Chat.T), axis=1)[:, :4]
tk = np.argsort(-(Ttr @ Mean.T), axis=1)[:, :50]
LV = [0, 32, 96, 192, 384]
def cov_pairs(Qs, kk_all, g):
    rows, cols = np.where(grp[kk_all] == g)
    if len(rows) < 200: return None
    D = ((Qs[rows] - Mean[kk_all[rows, cols]]) @ V[g]).astype(np.float64); return D.T @ D / len(D)
bases = {}
for g in range(G):
    Ct = cov_pairs(Ttr, tk, g); Ci = cov_pairs(X[ptr], pk, g)
    if Ct is None or Ci is None: continue
    out = {}
    lf = np.maximum(lam[g], 1e-9); sq = np.sqrt(lf)
    for nm, C in (("text", Ct), ("item", Ci), ("mix", 0.5 * Ct / np.trace(Ct * lf[None] ** 0) * 1 + 0.5 * Ci * (np.trace(Ct) / np.trace(Ci)))):
        M = sq[:, None] * C * sq[None]; gam, O = np.linalg.eigh(M); gam = np.maximum(gam[::-1], 1e-14); O = O[:, ::-1]
        out[nm] = (gam, O)          # r' = O^T diag(1/sq) z ;  Delta' = O^T diag(sq) Delta_local
    bases[g] = (out, sq, Ct, Ci)
res = {}
for qn, Q, th in (("item", X[qi], thr[:100]), ("text", T, thr[100:])):
    acc = {}; cnt = 0
    for j in range(0, len(Q), 2):
        s = X @ Q[j]; mi = np.where(s >= th[j, 2])[0]; mi = rng.choice(mi, 40, replace=False)
        ks, kc = np.unique(assign[mi], return_counts=True)
        for k, c in zip(ks, kc):
            g = grp[k]
            if g not in bases: continue
            out, sq, Ct, Ci = bases[g]; D = ((Q[j] - Mean[k]) @ V[g]).astype(np.float64)
            for L in LV:
                for an, prof in (("pca/item-bits", Qi[g]), ("pca/text-bits", Qt[g])):
                    b = alloc(lam[g], prof, L * 8); acc[(an, L)] = acc.get((an, L), 0) + c * (np.abs(D) * np.sqrt(lam[g] * GT[b])).sum()
                for nm, (gam, O) in out.items():
                    b = alloc(np.ones(n), gam, L * 8); Dp = O.T @ (sq * D)
                    acc[("gen-" + nm, L)] = acc.get(("gen-" + nm, L), 0) + c * (np.abs(Dp) * np.sqrt(GT[b])).sum()
            cnt += c
    print(f"\n{qn} queries, {cnt} (query, true match) pairs: predicted matched l1 half-width at {LV} bytes")
    for an in ("pca/item-bits", "pca/text-bits", "gen-item", "gen-text", "gen-mix"):
        print(f"  {an:14s}", " ".join(f"{acc[(an, L)] / cnt:.4f}" for L in LV))
g = max(bases, key=lambda g: (grp[assign] == g).sum()); out, sq, Ct, Ci = bases[g]
print("\nlargest group", g, ": sum sqrt(gamma) text", np.sqrt(out['text'][0]).sum(), "vs PCA-diagonal sum sqrt(Q_ii lam_i)", np.sqrt(np.diag(Ct) * lam[g]).sum(),
      "| item", np.sqrt(out['item'][0]).sum(), "vs", np.sqrt(np.diag(Ci) * lam[g]).sum())
