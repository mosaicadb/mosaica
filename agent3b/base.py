# Replica of the FINDINGS baseline ("local" deterministic code, nested levels) on the 200k subset, with the same
# clusters and queries as stats.py, so constructions can be compared on equal terms.
# Saves the per-pair byte cost (cost_<t>.npy, uint16) and the level-0 annulus decision (ann_<t>.npy, bool = touched).
import numpy as np, time, json, sys
rng = np.random.default_rng(1)
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
DD = sys.argv[1] if len(sys.argv) > 1 else "../data"
FILES = sys.argv[2:] if len(sys.argv) > 2 else ["e00", "e05", "e10", "e15"]
TAG = DD.strip("./").replace("/", "_")
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = unit(np.concatenate([np.load(f"{DD}/{f}.npy") for f in FILES]))
N, d = X.shape
K = N // 1000; NQ = 200; G = max(1, N // 15000)
TS = [0.95, 0.9, 0.8, 0.7]
qi = np.sort(rng.choice(N, NQ, replace=False)); Q = X[qi]
true = X @ Q.T
def kmeans(S, K, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for it in range(iters):
        a = np.concatenate([(S[i:i+50_000] @ C.T).argmax(1) for i in range(0, len(S), 50_000)])
        M = np.zeros_like(C); np.add.at(M, a, S)
        cnt = np.bincount(a, minlength=len(C)); empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C
Chat = kmeans(X, K)
assign = np.concatenate([(X[i:i+50_000] @ Chat.T).argmax(1) for i in range(0, N, 50_000)])
cnt = np.bincount(assign, minlength=K)
Mean = np.zeros((K, d)); np.add.at(Mean, assign, X); Mean = (Mean / np.maximum(cnt, 1)[:, None]).astype(np.float32)
Mh = Mean / np.maximum(np.linalg.norm(Mean, axis=1, keepdims=True), 1e-9)
grp = (Mh @ kmeans(Mh, G, 20).T).argmax(1) if G > 1 else np.zeros(K, int)
pq = X[rng.choice(N, 4000, replace=False)]
pk = np.argsort(-(pq @ Chat.T), axis=1)[:, :4]
np.savez(f"clusters_{TAG}.npz", assign=assign, Mean=Mean, grp=grp, qi=qi)
log("clusters done")

STEP = {2: 0.9957, 3: 0.5860, 4: 0.3352, 5: 0.1881, 6: 0.1041, 7: 0.0569, 8: 0.0308}
GT = np.array([1, .3634, .1188, .03744, .01154, .00349, .00104, .0003043, .0000876])
def quant(Z, bits):
    out = np.zeros_like(Z)
    for b in np.unique(bits):
        if b == 0: continue
        cols = np.where(bits == b)[0]; z = Z[:, cols]
        if b == 1: out[:, cols] = np.where(z >= 0, 0.7979, -0.7979)
        else:
            s = STEP[int(b)]; L = 2 ** int(b)
            out[:, cols] = (np.clip(np.floor(z / s), -L // 2, L // 2 - 1) + 0.5) * s
    return out
def alloc(lam, Qe, B):
    b = np.zeros(d, int); f = lambda bb: np.sqrt(Qe * lam * GT[bb]); val = f(b)
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - f(nb); gain[b >= 8] = -1
        i = int(gain.argmax()); b[i] += 1; val = f(b)
    return b
LV = [0, 4, 8, 16, 32, 48, 96, 192, 384, 576]                    # code bytes per nested level
LV = [b for b in LV if b * 8 <= 8 * d]
SCAL = 4
cost = np.zeros((len(TS), N, NQ), np.uint16)
viol = 0
und = np.zeros((len(LV), len(TS)))
for g in range(G):
    ks = np.where(grp == g)[0]
    kpos = -np.ones(K, int); kpos[ks] = np.arange(len(ks))
    items = np.where(grp[assign] == g)[0]
    Rs = X[items] - Mean[assign[items]]
    lam, V = np.linalg.eigh((Rs.T @ Rs).astype(np.float64) / max(len(Rs), 1))
    lam = np.maximum(lam[::-1], 1e-12).copy(); V = V[:, ::-1].astype(np.float32).copy(); sig = np.sqrt(lam).astype(np.float32)
    rows, cols = np.where(grp[pk] == g)
    Qe = (((pq[rows] - Mean[pk[rows, cols]]) @ V) ** 2).mean(0).astype(np.float64) if len(rows) > 50 else lam.copy()
    QV = Q @ V; MV = Mean[ks] @ V
    Dl = QV[None] - MV[:, None]; dn = np.sqrt((Dl ** 2).sum(2))
    a = assign[items]; ap = kpos[a]
    z = Rs @ V; mva = MV[ap]; T = true[items]; base = (Q @ Mean[ks].T).T[ap]      # m x NQ
    undecided = np.ones((len(TS), len(items), NQ), bool)
    cg = np.full((len(TS), len(items), NQ), SCAL, np.uint16)
    for li, Bb in enumerate(LV):
        bits = alloc(lam, Qe, Bb * 8)
        Dv = (((lam * GT[bits] + 1e-10) / (Qe + 1e-10)) ** 0.25).astype(np.float32)
        dw = np.sqrt(((Dl * Dv) ** 2).sum(2))
        if Bb == 0:
            evec = z; gam = np.zeros(len(z), np.float32); sc = np.zeros_like(T)
        else:
            code = quant(z / sig, bits) * sig
            cn2 = (code * code).sum(1) + 1e-20; gam = (z * code).sum(1) / cn2
            evec = z - gam[:, None] * code; sc = code @ QV.T
        en = np.linalg.norm(evec, axis=1); ew = np.linalg.norm(evec / Dv, axis=1); me = (mva * evec).sum(1)
        est = base + gam[:, None] * sc + me[:, None]
        h = np.minimum(dn[ap] * en[:, None], dw[ap] * ew[:, None]) * 1.0001 + 2e-5
        for ti, t in enumerate(TS):
            win = est - h >= t; out = est + h < t
            viol += int((win & (T < t - 2e-5)).sum() + (out & (T >= t)).sum())
            newly = undecided[ti] & (win | out)
            cg[ti][newly] = SCAL + Bb
            undecided[ti] &= ~(win | out)
            und[li, ti] += undecided[ti].sum()
    for ti in range(len(TS)):
        cg[ti][undecided[ti]] = SCAL + LV[-1] + 2 * d
        cost[ti][items] = cg[ti]
    log("group", g, "items", len(items), "violations", viol)
# level-0 annulus (tight cap bound from one scalar a = x.mhat): touched = not decided by it
ax = np.einsum("ij,ij->i", X, Mh[assign]); cq = Q @ Mh.T                 # NQ x K
est0 = ax[:, None] * cq.T[assign]; h0 = np.sqrt(np.maximum(1 - ax ** 2, 0))[:, None] * np.sqrt(np.maximum(1 - cq.T[assign] ** 2, 0)) * 1.0001 + 2e-5
out = {"N": N, "n": d, "NQ": NQ, "TS": TS, "LV": LV, "violations": viol, "undecided_share": (und / (N * NQ)).tolist()}
edges = np.linspace(-1, 1, 2001); bins = np.clip(np.digitize(true, edges) - 1, 0, 1999)
for ti, t in enumerate(TS):
    c = cost[ti]; M = true >= t
    touched = (est0 + h0 >= t) & (est0 - h0 < t)
    v0 = int((M & (est0 + h0 < t)).sum() + (~M & (est0 - h0 >= t)).sum())
    np.save(f"cost_{TAG}_{t}.npy", c); np.save(f"ann_{TAG}_{t}.npy", touched)
    chist = np.bincount(bins.ravel(), weights=c.ravel().astype(np.float64), minlength=2000)
    np.save(f"costhist_{TAG}_{t}.npy", chist)
    out[str(t)] = {"matches": float(M.mean()), "baseline_bytes": float(c.mean()), "baseline_pct": float(c.mean() / (2 * d) * 100),
                   "first_pass_bytes": SCAL, "annulus_touched": float(touched.mean()), "annulus_violations": v0,
                   "sorted_annulus_bytes": float((c * touched).mean()), "sorted_annulus_pct": float((c * touched).mean() / (2 * d) * 100),
                   "ideal_matches_only_pct": float(M.mean() * 100)}
    log(t, out[str(t)])
json.dump(out, open(f"base_{TAG}.json", "w"), indent=1)
