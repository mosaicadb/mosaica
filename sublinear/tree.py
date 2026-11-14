# Step 4. A hierarchy over centres: the four nested levels of index.npz (1,000 / 3,997 / 15,880 / 58,750 clusters, fan-out ~4).
# A node's children are read only if the node's ball bound leaves it undecided. Counts the centres read against reading
# every leaf centre, and the items under undecided leaves. Also writes x.mu (the corpus mean) for lsf.py.
# Every cluster bound is checked against the exact extremes of its members; violations are counted.
import numpy as np, json, os, sys
import scipy.sparse as sp
from common import *
Q, qtype, qkind, qi = queries()
ix = np.load(os.path.join(V, "index.npz")); LV = ix["levels"].astype(np.int32); NL = len(LV); KL = [int(l.max()) + 1 for l in LV]
rank = np.load(os.path.join(OUT, "rank.npy")); vis = rank < N
def sums(S, a, K):
    return np.asarray(sp.csr_matrix((np.ones(len(a), np.float32), (a, np.arange(len(a)))), shape=(K, len(a))) @ S)
Ms = [np.zeros((k, n), np.float32) for k in KL]
for i, X in chunks():
    for L in range(NL): Ms[L] += sums(X, LV[L][i:i + len(X)], KL[L])
mu = (Ms[0].sum(0) / N).astype(np.float32)
Mh = [m / np.linalg.norm(m, axis=1, keepdims=True) for m in Ms]; del Ms
amin = [np.full(k, 1.0, np.float32) for k in KL]; xmu = np.empty(N, np.float32)
for i, X in chunks():
    xmu[i:i + len(X)] = X @ mu
    for L in range(NL):
        l = LV[L][i:i + len(X)]; a = np.einsum("ij,ij->i", X, Mh[L][l]); np.minimum.at(amin[L], l, a)
np.save(os.path.join(OUT, "xmu.npy"), xmu); np.save(os.path.join(OUT, "mu.npy"), mu)
th = [np.arccos(np.clip(a, -1, 1)) for a in amin]
order = [np.argsort(l, kind="stable") for l in LV]; st = [np.concatenate([[0], np.flatnonzero(np.diff(LV[L][order[L]])) + 1]) for L in range(NL)]
cnt = [np.bincount(l[vis], minlength=k) for l, k in zip(LV, KL)]
parent = [None] + [LV[L - 1][order[L][st[L]]] for L in range(1, NL)]
log("levels", KL, "max angle (deg), median per level", [float(np.degrees(np.median(t))) for t in th])
Z0 = {r["q"]: r for r in json.load(open(os.path.join(OUT, "zero_all.json")))["recs"]}
KK = [10, 100, 1000]; SLACK = 2e-4
out = []; viol = 0; checks = 0
for q in range(NQ):
    wait_lock(); s = score_row(q); rec = {"q": q, "type": qtype[q]}
    und = []; ext = []
    for L in range(NL):
        phi = np.arccos(np.clip(Mh[L] @ Q[q], -1, 1)); Ub = np.cos(np.maximum(0, phi - th[L])) + SLACK; Lb = np.cos(np.minimum(np.pi, phi + th[L])) - SLACK
        so = s[order[L]]; cmax = np.maximum.reduceat(so, st[L]); cmin = np.minimum.reduceat(so, st[L])
        viol += int(((cmax > Ub) | (cmin < Lb)).sum()); checks += 2 * KL[L]; und.append((Ub, Lb)); ext.append((cmax, cmin))
    for k in KK:
        t = Z0[q]["sub"][str(SIZES[-1])][str(k)]["t"]; visited = np.ones(KL[0], bool); r = {"read": [], "und": [], "und_items": [], "oracle": []}
        for L in range(NL):
            Ub, Lb = und[L]; u = visited & (Lb < t) & (Ub >= t)
            r["read"].append(int(visited.sum())); r["und"].append(int(u.sum())); r["und_items"].append(int(cnt[L][u].sum()))
            r["oracle"].append(int(((ext[L][1] < t) & (ext[L][0] >= t)).sum()))
            if L + 1 < NL: visited = u[parent[L + 1]]
        rec[str(k)] = r
    out.append(rec)
    if q % 50 == 0: log("query", q, "violations", viol, "of", checks)
json.dump({"KL": KL, "viol": viol, "checks": checks, "recs": out}, open(os.path.join(OUT, "tree.json"), "w"))
print(f"cluster-bound checks {checks:,}; violations {viol}")
for typ in ("text", "item"):
    R = [r for r in out if r["type"] == typ]
    for k in KK:
        rd = np.array([r[str(k)]["read"] for r in R]).mean(0); un = np.array([r[str(k)]["und"] for r in R]).mean(0)
        ui = np.array([r[str(k)]["und_items"] for r in R]).mean(0); orc = np.array([r[str(k)]["oracle"] for r in R]).mean(0)
        print(f"{typ} k={k}: centres read per level {np.round(rd).astype(int).tolist()} of {KL}; total {rd.sum():.0f} = {100*rd.sum()/KL[-1]:.1f}% of the leaf centres; "
              f"undecided nodes {np.round(100*un/np.array(KL),1).tolist()}%; items under undecided nodes {np.round(100*ui/vis.sum(),1).tolist()}%; nodes holding both matches and non-matches {np.round(orc,1).tolist()}")
