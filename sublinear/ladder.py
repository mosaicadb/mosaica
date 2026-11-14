# Step 3. The per-member read (the block ladder of vc/scripts/margin.py: exact local PCA coordinates in dyadic blocks, the
# rest bounded block by block by the product of the two block norms) at fixed answer sizes, on each nested subsample with
# its own clustering and bases, and on visible subsets of the 1M clustering. Records, per query, set and k, the histogram of
# the level at which an item is decided. Every interval at every level is checked against the exact score.
import numpy as np, json, os, sys
from common import *
j = int(sys.argv[1])
Q, qtype, qkind, qi = queries()
QS = list(range(0, 369, 3)) + list(range(369, 469)); Qs = Q[QS]; nq = len(QS)
rank = np.load(os.path.join(OUT, "rank.npy")); nq_before = np.cumsum(np.isin(np.arange(N), qi))
Z0 = {r["q"]: r for r in json.load(open(os.path.join(OUT, "zero_all.json")))["recs"]}
d = np.load(os.path.join(OUT, f"cl{j}.npz")); lab = d["lab"]; Mean = d["Mean"]; grp = d["grp"]; xm = d["xm"]; K = len(Mean); G = int(grp.max()) + 1
GF = os.environ.get("GFIX") == "1" and j >= 3            # variant: 12 local bases at every N (prep_g.py)
if GF: grp = np.load(os.path.join(OUT, f"grpg{j}.npy")); G = int(grp.max()) + 1
Vb = np.load(os.path.join(OUT, f"Vg{j}.npy" if GF else f"V{j}.npy")); mn2 = (Mean ** 2).sum(1)
E = [0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, n]; NB = len(E) - 1; SLACK = 2e-4
KK = [10, 100, 1000, 10000]
Mu = np.stack([Mean[k] @ Vb[grp[k]] for k in range(K)]); Mu2 = np.add.reduceat(Mu * Mu, E[:-1], axis=1)
Qg = np.einsum("gij,qi->gqj", Vb, Qs)                                           # G x nq x n
QM = Qs @ Mean.T                                                                # nq x K
# sets and thresholds
top = j == len(SIZES) - 1 and os.environ.get("GFIX") != "1"
sets = {"sub": lambda gid: rank[gid] < SIZES[j]}
if top:
    for v in [977, 3906, 15625, 31250, 62500, 125000, 250000, 500000]:
        sets[f"rand{v}"] = (lambda v: lambda gid: rank[gid] < v)(v)
        sets[f"range{v}"] = (lambda v: lambda gid: (rank[gid] < N) & ((gid - nq_before[gid]) < v))(v)
def thr(q, name):
    r = Z0[q]["sub"][str(SIZES[j])] if name == "sub" else Z0[q]["vis"][name]
    return [(k, r[str(k)]["t"]) for k in KK if str(k) in r]
H = {name: np.zeros((nq, len(KK), NB + 2), np.int64) for name in sets}
viol = 0; checks = 0
for i0, X in chunks():
    l = lab[i0:i0 + len(X)]; gid = i0 + np.arange(len(X)); m = (l >= 0) & (rank[gid] < N)
    if not m.any(): continue
    Xs = X[m]; ls = l[m]; gid = gid[m]; g = grp[ls]; M_ = len(ls); del X
    S = Xs @ Qs.T                                                               # exact scores, M x nq
    R = Xs - Mean[ls]; b = xm[gid] - mn2[ls]; del Xs
    Z = np.empty_like(R); C = np.empty((nq, M_, NB), np.float32)
    for gg in np.unique(g):
        mg = np.flatnonzero(g == gg); Zg = R[mg] @ Vb[gg]; Z[mg] = Zg
        for bi in range(NB): C[:, mg, bi] = (Zg[:, E[bi]:E[bi+1]] @ Qg[gg][:, E[bi]:E[bi+1]].T).T
    del R
    Rn = np.sqrt(np.add.reduceat(Z * Z, E[:-1], axis=1)); Mrb = np.add.reduceat(Z * Mu[ls], E[:-1], axis=1); del Z
    insets = {name: np.flatnonzero(f(gid)) for name, f in sets.items()}
    for qq in range(nq):
        qg = Qg[grp, qq]                                                        # K x n
        qg2 = np.add.reduceat(qg * qg, E[:-1], axis=1); qmu = np.add.reduceat(qg * Mu, E[:-1], axis=1); qm = QM[qq]
        U = np.full((M_, NB + 1), np.inf, np.float32); L = np.full((M_, NB + 1), -np.inf, np.float32)
        for al in (qmu.sum(1) / Mu2.sum(1), np.zeros(K, np.float32)):
            D = C[qq] - al[ls, None] * Mrb; base = qm[ls] + al[ls] * b
            dn = np.sqrt(np.maximum(0, qg2 - 2 * al[:, None] * qmu + al[:, None] ** 2 * Mu2))
            Ec = np.concatenate([base[:, None], base[:, None] + np.cumsum(D, 1)], 1)
            T = np.concatenate([np.cumsum((dn[ls] * Rn)[:, ::-1], 1)[:, ::-1], np.zeros((M_, 1), np.float32)], 1) + SLACK
            U = np.minimum(U, Ec + T); L = np.maximum(L, Ec - T)
        s = S[:, qq]
        viol += int(((s[:, None] > U) | (s[:, None] < L)).sum()); checks += U.size
        U = np.minimum.accumulate(U, 1); L = np.maximum.accumulate(L, 1)
        for name, sel in insets.items():
            if len(sel) == 0: continue
            Us, Ls = (U, L) if len(sel) == M_ else (U[sel], L[sel])
            for k, t in thr(QS[qq], name):
                lev = ((Ls < t) & (Us >= t)).sum(1)
                H[name][qq, KK.index(k)] += np.bincount(lev, minlength=NB + 2)
    log(f"clustering {j} rows to {i0 + CH}: violations {viol} of {checks}")
json.dump({"j": j, "N": SIZES[j], "QS": QS, "E": E, "KK": KK, "viol": viol, "checks": checks, "H": {k: v.tolist() for k, v in H.items()}},
          open(os.path.join(OUT, f"ladder_g_{j}.json" if os.environ.get("GFIX") == "1" else f"ladder_{j}.json"), "w"))
log("done; violations", viol, "of", checks)
