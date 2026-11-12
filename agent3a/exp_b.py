# E2: (a) guarantee uniform over a k-dimensional query class, (b) how low-dimensional real query offsets are,
#     (c) an adaptive adversary with L leaked bits, (d) full recovery of an item's error vector from n_b estimates.
# One rotated block of a one-group (global residual basis) version of the split scheme, 200k subset.
import numpy as np, time, math
rng = np.random.default_rng(1)
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
DATA = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/data/"
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = unit(np.concatenate([np.load(DATA + f"e{i:02d}.npy") for i in (0, 5, 10, 15)]))
N, d = X.shape; K = 200
def kmeans(S, K, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for it in range(iters):
        a = np.concatenate([(S[i:i+50_000] @ C.T).argmax(1) for i in range(0, len(S), 50_000)])
        M = np.zeros_like(C); np.add.at(M, a, S)
        cnt = np.bincount(a, minlength=K); empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C
Chat = kmeans(X, K); assign = (X @ Chat.T).argmax(1)
Msum = np.zeros((K, d)); np.add.at(Msum, assign, X)
Mean = (Msum / np.maximum(np.bincount(assign, minlength=K), 1)[:, None]).astype(np.float32)
Rs = X - Mean[assign]
lam, V = np.linalg.eigh((Rs.T @ Rs).astype(np.float64) / N); lam = np.maximum(lam[::-1], 1e-12).copy(); V = V[:, ::-1].astype(np.float32).copy()
# query offsets: 2000 corpus items against their 4 nearest clusters
pq = X[rng.choice(N, 2000, replace=False)]; pk = np.argsort(-(pq @ Chat.T), axis=1)[:, :4]
Doff = ((pq[:, None, :] - Mean[pk]).reshape(-1, d) @ V)                      # 8000 offsets in the residual basis
Qe = (Doff ** 2).mean(0).astype(np.float64)
Dt = ((lam / Qe) ** 0.25).astype(np.float32)
a_i = np.sqrt(lam * Qe); order = np.argsort(-a_i)
sub = rng.choice(N, 40_000, replace=False); Z = (Rs[sub] @ V) / Dt           # whitened residuals
Dw = Doff * Dt                                                               # whitened offsets
for (lo, hi, rate) in [(96, 224, 1.0), (224, 480, 1.0), (480, 768, 0.25)]:
    idx = order[lo:hi]; nb = len(idx); m = nb - 1
    R = np.linalg.qr(rng.standard_normal((nb, nb)))[0].astype(np.float32)
    y = Z[:, idx] @ R.T; l2 = np.linalg.norm(y, axis=1)
    kk = int(rate * nb); cy = np.zeros_like(y); cy[:, :kk] = np.sign(y[:, :kk])
    num = (cy * y).sum(1); E = (l2 * l2 / num)[:, None] * cy - y                # error vector of each item, rotated frame
    A = np.linalg.norm(E, axis=1); U = E / A[:, None]
    assert np.abs((U * y).sum(1) / l2).max() < 1e-3                          # E is orthogonal to y
    Dr = Dw[:, idx] @ R.T                                                    # real offsets in the rotated frame
    print(f"\n=== block dims {lo}:{hi} (n_b={nb}), rate {rate} bit/dim, {len(Z)} items ===")
    # per-query check on real offsets
    r = (U[:5000] @ Dr[:2000].T) / np.linalg.norm(Dr[:2000], axis=1)[None] * math.sqrt(m)
    print(f"per-query: 5000 items x 2000 real offsets, max |error| = {np.abs(r).max():.2f} predicted sd (rms {r.std():.3f})")
    # (b) spectrum of real offsets inside the block
    Cq = (Dr.T @ Dr).astype(np.float64) / len(Dr); ev, Wq = np.linalg.eigh(Cq); ev = ev[::-1]; Wq = Wq[:, ::-1].astype(np.float32)
    pr = ev.sum() ** 2 / (ev ** 2).sum(); f = np.cumsum(ev) / ev.sum()
    print(f"(b) real offsets in block: participation ratio {pr:.0f} of {nb}; energy in top k PCs:",
          " ".join(f"k={k}:{f[k-1]:.2f}" for k in [1, 4, 16, 64, 128] if k <= nb))
    # (a) uniform over a k-dim class: sup over unit Delta in W of |<E,Delta>| = A * ||P_W U||
    print("(a) k | random W: max_i ||P_W u_i||*sqrt(m)  [mean] (excess over sqrt(k), in sd) | W = top-k PCs of real offsets: same | "
          "uniform/per-query(c=8) = (sqrt k + c)/c | uniform/deterministic = (sqrt k + c)/sqrt m")
    for k in [1, 4, 16, 64, 128]:
        if k >= nb: continue
        Wr = np.linalg.qr(rng.standard_normal((nb, k)))[0].astype(np.float32)
        out = []
        for W in (Wr, Wq[:, :k]):
            s = np.linalg.norm(U @ W, axis=1) * math.sqrt(m)
            out.append(f"{s.max():6.2f} [{s.mean():5.2f}] ({s.max() - math.sqrt(k):5.2f})")
        print(f"   {k:4d} | {out[0]} | {out[1]} | {(math.sqrt(k) + 8) / 8:5.2f} | {(math.sqrt(k) + 8) / math.sqrt(m):5.2f}")
    # class certificate with stored scalars: ||P_W D|| ||P_W E|| + ||P_perp D|| ||P_perp E||, against A ||D|| and c A ||D|| / sqrt m
    Ds = Dr[rng.choice(len(Dr), 300, replace=False)]; Es = E[:3000]; As = A[:3000]; dn = np.linalg.norm(Ds, axis=1)
    viol = 0; line = []
    for k in [4, 16, 64]:
        if k >= nb: continue
        W = Wq[:, :k]; Dp = np.linalg.norm(Ds @ W, axis=1); Dq = np.sqrt(np.maximum(dn ** 2 - Dp ** 2, 0))
        Ep = np.linalg.norm(Es @ W, axis=1); Eq = np.sqrt(np.maximum(As ** 2 - Ep ** 2, 0))
        hb = Ep[:, None] * Dp[None] + Eq[:, None] * Dq[None]; act = np.abs(Es @ Ds.T)
        viol += int((act > hb * 1.0001 + 1e-9).sum())
        line.append(f"k={k}: {(hb / (As[:, None] * dn[None])).mean():.3f}")
    print(f"    deterministic two-scalar class certificate / plain Cauchy-Schwarz, real offsets: {'  '.join(line)}  (violations {viol});"
          f"  per-query randomised c=8 / plain: {8 / math.sqrt(m):.3f}")
    # (c) adversary with L leaked bits about one item's error vector
    print("(c) L leaked bits -> error of the adversary's query in predicted sd (mean over 200 items) | ceiling proved: sqrt(2 ln2 L) | sqrt(m) =", f"{math.sqrt(m):.1f}")
    for L in [16, 64, 256, 1024, 4096]:
        res = []
        for i in range(200):
            Gm = rng.standard_normal((L, nb)).astype(np.float32); s = np.sign(Gm @ U[i]); D = s @ Gm
            res.append(float(U[i] @ D) / float(np.linalg.norm(D)) * math.sqrt(m))
        print(f"   L={L:5d}: {np.mean(res):6.2f} (min {np.min(res):5.2f})  | {math.sqrt(2 * math.log(2) * L):6.2f}")
    # (d) n_b real-valued estimates of one item reveal its error vector exactly
    i = 7; P = rng.standard_normal((nb, nb)).astype(np.float64); v = (l2[i] ** 2 / num[i]) * cy[i].astype(np.float64)
    vhat = np.linalg.solve(P, P @ v); yh = y[i].astype(np.float64) / l2[i]; Eh = vhat - yh * (yh @ vhat) * 1.0
    Eh = vhat - y[i].astype(np.float64)
    print(f"(d) {nb} estimates of item {i} solved for its error vector; attack query error = {abs(E[i].astype(np.float64) @ Eh) / (A[i] * np.linalg.norm(Eh)) * math.sqrt(m):.2f} predicted sd"
          f" (deterministic bound = {math.sqrt(m):.2f} sd, c = 8 or 10)")
