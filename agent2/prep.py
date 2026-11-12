# Shared index for all agent2 experiments: 200k subset, K=200 clusters, G=12 local bases (same construction as exp2.py).
import numpy as np, time, sys, os
rng = np.random.default_rng(0); t0 = time.time()
def log(*a): print(f"[{time.time()-t0:5.0f}s]", *a, flush=True)
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = np.load("X.npy") if os.path.exists("X.npy") else np.concatenate([unit(np.load(f"../data/e{i:02d}.npy")) for i in (0, 5, 10, 15)]); N, n = X.shape
K, G = 200, 12
def kmeans(S, K, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for it in range(iters):
        a = np.concatenate([(S[i:i+50_000] @ C.T).argmax(1) for i in range(0, len(S), 50_000)])
        M = np.zeros_like(C); np.add.at(M, a, S)
        cnt = np.bincount(a, minlength=K); empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C
Chat = kmeans(X, K); assign = np.concatenate([(X[i:i+50_000] @ Chat.T).argmax(1) for i in range(0, N, 50_000)]).astype(np.int32)
Msum = np.zeros((K, n)); np.add.at(Msum, assign, X); Mean = (Msum / np.maximum(np.bincount(assign, minlength=K), 1)[:, None]).astype(np.float32)
Mh = Mean / np.linalg.norm(Mean, axis=1, keepdims=True); grp = (Mh @ kmeans(Mh, G, 20).T).argmax(1)
log("clusters done; group sizes", np.bincount(grp[assign], minlength=G).tolist())
pq_i = rng.choice(N, 5000, replace=False); pq = X[pq_i]; pk = np.argsort(-(pq @ Chat.T), axis=1)[:, :4]
V = np.zeros((G, n, n), np.float32); lam = np.zeros((G, n)); Qi = np.zeros((G, n))
Z = np.lib.format.open_memmap("Z.npy", mode="w+", dtype=np.float32, shape=(N, n))
for g in range(G):
    m = grp[assign] == g; R = X[m] - Mean[assign[m]]
    l, v = np.linalg.eigh((R.T @ R).astype(np.float64) / max(len(R), 1))
    lam[g] = np.maximum(l[::-1], 1e-12); V[g] = v[:, ::-1].astype(np.float32)
    Z[np.where(m)[0]] = R @ V[g]
    pm = grp[pk] == g; rows, cols = np.where(pm)
    Qi[g] = (((pq[rows] - Mean[pk[rows, cols]]) @ V[g]) ** 2).mean(0) if pm.sum() > 50 else lam[g]
    log("group", g, "items", int(m.sum()), "pseudo-query pairs", int(pm.sum()))
Z.flush()
np.savez("index.npz", assign=assign, Mean=Mean, grp=grp, V=V, lam=lam, Qi=Qi, Chat=Chat)
qi = np.sort(np.random.default_rng(5).choice(N, 100, replace=False)); np.save("item_q_idx.npy", qi)
log("done")
