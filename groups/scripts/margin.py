# Where the per-member read spends its coordinates, by each item's distance from the threshold.
# 200k-item sample of the 1M set, PCA bases fitted on the sample, the per-member block ladder of measure.py.
import numpy as np, json, sys, os
W = sys.argv[1]; rng = np.random.default_rng(11)
X = np.load(f"{W}/X.npy", mmap_mode="r"); N, n = X.shape
ix = np.load(f"{W}/index.npz"); a0 = ix["levels"][0]; grp = ix["grp"]; K = int(a0.max()) + 1; G = int(grp.max()) + 1
Ms = np.zeros((K, n)); cnt = np.bincount(a0, minlength=K)
for i in range(0, N, 100_000): np.add.at(Ms, a0[i:i+100_000], X[i:i+100_000])
Mean = (Ms / cnt[:, None]).astype(np.float32); mn = np.linalg.norm(Mean, axis=1)
si = np.sort(rng.choice(N, 200_000, replace=False)); Xs = np.asarray(X[si]); cs = a0[si]; gs = grp[cs]; NS = len(si)
E = [0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, n]; NB = len(E) - 1
Z = np.empty((NS, n), np.float32); V = np.empty((G, n, n), np.float32)
for g in range(G):
    m = gs == g; R = Xs[m] - Mean[cs[m]]
    l, v = np.linalg.eigh((R.T @ R).astype(np.float64) / max(m.sum(), 1)); V[g] = v[:, ::-1]; Z[m] = R @ V[g]
Rn = np.stack([np.linalg.norm(Z[:, E[i]:E[i+1]], axis=1) for i in range(NB)], 1)
Mu = np.stack([Mean[k] @ V[grp[k]] for k in range(K)]); b = (Xs * Mean[cs]).sum(1) - (Mean[cs] ** 2).sum(1)
Mrb = np.stack([(Z[:, E[i]:E[i+1]] * Mu[cs, E[i]:E[i+1]]).sum(1) for i in range(NB)], 1)
Mu2 = np.add.reduceat(Mu * Mu, E[:-1], axis=1)
Qt = np.load(f"{W}/text_q.npy").astype(np.float32); Qt /= np.linalg.norm(Qt, axis=1, keepdims=True)
Q = np.concatenate([Qt[::6], np.asarray(X[np.load(f"{W}/item_q_idx.npy")[:40]])]); kind = ["text"] * len(Qt[::6]) + ["item"] * 40
FR = [1e-4, 1e-3, 1e-2, 5e-2]; BINS = [0, 0.25, 0.5, 1, 2, 4, 8, np.inf]
out = {"text": [], "item": []}
for q, kd in zip(Q, kind):
    s = Xs @ q; sig = s.std(); med = np.median(s)
    qg = np.stack([q @ V[g] for g in range(G)])[grp]                              # K x n
    qg2 = np.add.reduceat(qg * qg, E[:-1], axis=1); qmu = np.add.reduceat(qg * Mu, E[:-1], axis=1); qm = Mean @ q
    C = np.empty((NS, NB), np.float32)
    for g in range(G):
        m = gs == g; qv = q @ V[g]
        for i in range(NB): C[m, i] = Z[m, E[i]:E[i+1]] @ qv[E[i]:E[i+1]]
    U = np.full((NS, NB + 1), np.inf, np.float32); L = np.full((NS, NB + 1), -np.inf, np.float32)
    for al in (qmu.sum(1) / Mu2.sum(1), np.zeros(K)):
        D = C - al[cs, None] * Mrb; base = qm[cs] + al[cs] * b
        dn = np.sqrt(np.maximum(0, qg2 - 2 * al[:, None] * qmu + al[:, None] ** 2 * Mu2))
        Ec = np.concatenate([base[:, None], base[:, None] + np.cumsum(D, 1)], 1)
        T = np.concatenate([np.cumsum((dn[cs] * Rn)[:, ::-1], 1)[:, ::-1], np.zeros((NS, 1))], 1) + 2e-4
        U = np.minimum(U, Ec + T); L = np.maximum(L, Ec - T)
    assert ((s[:, None] <= U) & (s[:, None] >= L)).all()
    rec = []
    for f in FR:
        t = np.partition(s, NS - round(f * NS))[NS - round(f * NS)]
        dec = (L >= t) | (U < t); j = dec.argmax(1); cost = np.array(E)[j].astype(float)
        u = np.abs(s - t) / sig; h = np.histogram(u, BINS)[0]; c = np.histogram(u, BINS, weights=cost)[0]
        dens = ((np.abs(s - t) < 0.25 * sig).sum() / NS) / 0.5                     # items per unit sigma at t
        rec.append(dict(z=(t - med) / sig, cost=cost.mean(), share=(c / cost.sum()).tolist(), frac=(h / NS).tolist(),
                        dens=dens, slack0=float((U[:, 0] - L[:, 0]).mean() / 2 / sig)))
    out[kd].append(rec)
np.set_printoptions(precision=3, suppress=True)
lab = ["<0.25", "0.25-0.5", "0.5-1", "1-2", "2-4", "4-8", ">8"]
for kd, R in out.items():
    print(f"\n== {kd} queries ({len(R)}); distances in units of the query's score sd; level-0 half-slack {np.mean([r[0]['slack0'] for r in R]):.1f} sd")
    for k, f in enumerate(FR):
        g = lambda key: np.mean([r[k][key] for r in R], axis=0)
        print(f"answer {100*f:5.2f}%: threshold {g('z'):4.2f} sd above median; density at t {g('dens'):.5f} per sd; coordinates per item {g('cost'):6.1f}")
        print("   items by distance  " + "  ".join(f"{l}:{100*x:5.1f}%" for l, x in zip(lab, g('frac'))))
        print("   share of the cost  " + "  ".join(f"{l}:{100*x:5.1f}%" for l, x in zip(lab, g('share'))))
