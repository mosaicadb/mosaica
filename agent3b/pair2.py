# Construction C2 on the metric: pruning clusters of several sizes (independent of the coding clusters of base.py),
# centroid-anchor support table + sorted annulus range. Bytes = baseline per-pair cost summed over touched pairs only,
# plus table rows and range searches. Centroid reads (K * 2n bytes) are reported separately.
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
qi = np.load(f"clusters_{TAG}.npz")["qi"]; Q = X[qi]; NQ = len(qi); TS = [0.95, 0.9, 0.8, 0.7]
true = X @ Q.T
COST = {t: np.load(f"cost_{TAG}_{t}.npy") for t in TS}
rng = np.random.default_rng(11)
def kmeans(S, K, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for it in range(iters):
        a = np.concatenate([(S[i:i+50_000] @ C.T).argmax(1) for i in range(0, len(S), 50_000)])
        M = np.zeros_like(C); np.add.at(M, a, S)
        cnt = np.bincount(a, minlength=len(C)); empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C
res = {"baseline_bytes": {str(t): float(COST[t].mean()) for t in TS}, "n": n, "N": N}
for K in [int(a) for a in (N // 4000, N // 1000, N // 250)]:
    Chat = kmeans(X, K)
    assign = np.concatenate([(X[i:i+50_000] @ Chat.T).argmax(1) for i in range(0, N, 50_000)])
    keep = np.unique(assign); remap = -np.ones(K, int); remap[keep] = np.arange(len(keep)); assign = remap[assign]; K = len(keep)
    cnt = np.bincount(assign, minlength=K)
    Mean = np.zeros((K, n)); np.add.at(Mean, assign, X); Mean = (Mean / cnt[:, None]).astype(np.float32)
    Mh = Mean / np.linalg.norm(Mean, axis=1, keepdims=True)
    order = np.argsort(assign, kind="stable"); starts = np.searchsorted(assign[order], np.arange(K))
    ax = np.einsum("ij,ij->i", X, Mh[assign]); sx = np.sqrt(np.maximum(1 - ax ** 2, 0))
    S = X @ Mean.T
    hi = np.maximum.reduceat(S[order], starts, axis=0).T; lo = np.minimum.reduceat(S[order], starts, axis=0).T; del S
    cq = Q @ Mh.T; sq = np.sqrt(np.maximum(1 - cq ** 2, 0)); sim = Q @ Mean.T
    for J in (1, 8):
        W = np.zeros((NQ, K), np.float32)
        for q in range(NQ):
            if J == 1: W[q, int(np.argmin(((Q[q][None] - Mean) ** 2).sum(1)))] = 1.0
            else:
                top = np.argsort(-sim[q])[:J]; w, _ = nnls(Mean[top].T.astype(np.float64), Q[q].astype(np.float64)); W[q, top] = w
        RHO = Q - W @ Mean; rn = np.linalg.norm(RHO, axis=1)
        rc = RHO @ Mh.T; rp = np.sqrt(np.maximum(rn[:, None] ** 2 - rc ** 2, 0)); WH = W @ hi; WL = W @ lo
        over = ((W > 0).sum(1).mean() * K * 2 + K * 2 * np.ceil(np.log2(cnt.mean())) * 2) / N     # table rows + range searches, bytes/item
        out = {"K": K, "J": J, "overhead_bytes_per_item": float(over), "centroid_bytes_per_item": K * 2 * n / N}
        for t in TS:
            acc = np.zeros(6); viol = 0
            for q in range(NQ):
                g_ub = WH[q][assign] + rc[q][assign] * ax + rp[q][assign] * sx + 2e-5
                g_lb = WL[q][assign] + rc[q][assign] * ax - rp[q][assign] * sx - 2e-5
                a_ub = cq[q][assign] * ax + sq[q][assign] * sx + 2e-5; a_lb = cq[q][assign] * ax - sq[q][assign] * sx - 2e-5
                ub = np.minimum(g_ub, a_ub); lb = np.maximum(g_lb, a_lb)
                M = true[:, q] >= t
                viol += int((M & (ub < t)).sum() + (~M & (lb >= t)).sum())
                tch = (ub >= t) & (lb < t); tann = (a_ub >= t) & (a_lb < t); c = COST[t][:, q]
                acc += [tch.sum(), c[tch].sum(), tann.sum(), c[tann].sum(), (lb >= t).sum(), (a_lb >= t).sum()]
            acc /= N * NQ
            out[str(t)] = {"touched": acc[0], "bytes": acc[1] + over, "pct": (acc[1] + over) / (2 * n) * 100,
                           "annulus_touched": acc[2], "annulus_bytes": acc[3] + K * 2 * np.ceil(np.log2(cnt.mean())) * 2 / N,
                           "annulus_pct": (acc[3] + K * 2 * np.ceil(np.log2(cnt.mean())) * 2 / N) / (2 * n) * 100,
                           "certified_in": acc[4], "annulus_certified_in": acc[5], "violations": viol}
        res[f"K{K}_J{J}"] = out
        log(K, J, "touched", [round(100 * out[str(t)]["touched"], 2) for t in TS], "pct", [round(out[str(t)]["pct"], 3) for t in TS],
            "| annulus touched", [round(100 * out[str(t)]["annulus_touched"], 2) for t in TS], "pct", [round(out[str(t)]["annulus_pct"], 3) for t in TS],
            "| in", [round(100 * out[str(t)]["certified_in"], 2) for t in TS], "viol", [out[str(t)]["violations"] for t in TS], "over", round(over, 4))
json.dump(res, open(f"pair2_{TAG}.json", "w"), indent=1)
log("baseline pct", [round(COST[t].mean() / (2 * n) * 100, 3) for t in TS])
