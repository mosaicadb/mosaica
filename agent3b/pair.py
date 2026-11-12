# Group-level certificates from a table of exact support values: P[j, c] = max_{x in cluster c} p_j . x for anchor
# directions p_j. For w >= 0:  max_{x in c} q.x <= sum_j w_j P[j,c] + rho.m_c-part,  rho = q - sum_j w_j p_j  (sublinearity
# of the maximum). Deterministic. Combined with the per-item annulus range (one sorted scalar per cluster).
import numpy as np, time, json, sys
from scipy.optimize import nnls
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
DD = sys.argv[1] if len(sys.argv) > 1 else "../data"
FILES = sys.argv[2:] if len(sys.argv) > 2 else ["e00", "e05", "e10", "e15"]
TAG = DD.strip("./").replace("/", "_")
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = unit(np.concatenate([np.load(f"{DD}/{f}.npy") for f in FILES]))
N, n = X.shape
cl = np.load(f"clusters_{TAG}.npz"); assign, Mean, qi = cl["assign"], cl["Mean"], cl["qi"]
K = len(Mean); Q = X[qi]; NQ = len(qi); TS = [0.95, 0.9, 0.8, 0.7]
true = X @ Q.T
Mh = Mean / np.maximum(np.linalg.norm(Mean, axis=1, keepdims=True), 1e-9)
order = np.argsort(assign, kind="stable"); starts = np.searchsorted(assign[order], np.arange(K)); cnt = np.bincount(assign, minlength=K)
ax = np.einsum("ij,ij->i", X, Mh[assign])                       # the sorted scalar: a_x = x . mhat_c
sx = np.sqrt(np.maximum(1 - ax ** 2, 0))
rng = np.random.default_rng(5)
mask = np.ones(N, bool); mask[qi] = False
def table(P):                                                   # A x K exact max and min of p_j . x over each cluster
    S = X @ P.T                                                 # N x A
    hi = np.maximum.reduceat(S[order], starts, axis=0).T; lo = np.minimum.reduceat(S[order], starts, axis=0).T
    return hi, lo
def run(name, P, J):
    hi, lo = table(P)
    A = len(P); sim = Q @ P.T
    UB = np.empty((NQ, K), np.float32); LB = np.empty((NQ, K), np.float32)      # cluster-level
    W = np.zeros((NQ, A), np.float32)
    for q in range(NQ):
        if J == 1:
            j = int(np.argmin(((Q[q][None] - P) ** 2).sum(1))); W[q, j] = 1.0
        else:
            top = np.argsort(-sim[q])[:J]
            w, _ = nnls(P[top].T.astype(np.float64), Q[q].astype(np.float64)); W[q, top] = w
    RHO = Q - W @ P                                             # NQ x n
    rn = np.linalg.norm(RHO, axis=1)
    # remainder rho . x for x in cluster c:  (rho.mhat_c) a_x + |rho_perp| sqrt(1-a_x^2)   (annulus form, per item)
    rc = RHO @ Mh.T; rp = np.sqrt(np.maximum(rn[:, None] ** 2 - rc ** 2, 0))    # NQ x K
    cq = Q @ Mh.T; sq = np.sqrt(np.maximum(1 - cq ** 2, 0))
    WH = W @ hi; WL = W @ lo                                    # NQ x K
    out = {"anchors": A, "J": J, "rho_norm_median": float(np.median(rn)), "active_median": float(np.median((W > 0).sum(1)))}
    for t in TS:
        tc = np.zeros(4); viol = 0                              # cluster-level touched, item touched (grp), item touched (grp+annulus), annulus only
        for q in range(NQ):
            a_c = assign
            g_ub = WH[q][a_c] + rc[q][a_c] * ax + rp[q][a_c] * sx + 2e-5        # group max of anchor part + per-item remainder
            g_lb = WL[q][a_c] + rc[q][a_c] * ax - rp[q][a_c] * sx - 2e-5
            a_ub = cq[q][a_c] * ax + sq[q][a_c] * sx + 2e-5; a_lb = cq[q][a_c] * ax - sq[q][a_c] * sx - 2e-5
            T = true[:, q]; M = T >= t
            ub = np.minimum(g_ub, a_ub); lb = np.maximum(g_lb, a_lb)
            viol += int((M & (ub < t)).sum() + (~M & (lb >= t)).sum())
            # cluster-level decision: whole cluster out if max over its items of g_ub < t, whole cluster in if min g_lb >= t
            cmax = np.maximum.reduceat(g_ub[order], starts); cmin = np.minimum.reduceat(g_lb[order], starts)
            tc += [cnt[(cmax >= t) & (cmin < t)].sum(), ((g_ub >= t) & (g_lb < t)).sum(), ((ub >= t) & (lb < t)).sum(), ((a_ub >= t) & (a_lb < t)).sum()]
        out[str(t)] = {"cluster_touched": tc[0] / (N * NQ), "group_item_touched": tc[1] / (N * NQ), "combined_touched": tc[2] / (N * NQ),
                       "annulus_touched": tc[3] / (N * NQ), "violations": viol}
    log(name, json.dumps(out))
    return out
res = {}
res["centroid_J1"] = run("centroid_J1", Mean, 1)
res["centroid_J8"] = run("centroid_J8", Mean, 8)
res["centroid_J32"] = run("centroid_J32", Mean, 32)
items = X[rng.choice(np.where(mask)[0], 2000, replace=False)]
res["items2000_J1"] = run("items2000_J1", items, 1)
res["items2000_J16"] = run("items2000_J16", items, 16)
res["items2000_J64"] = run("items2000_J64", items, 64)
json.dump(res, open(f"pair_{TAG}.json", "w"), indent=1)
