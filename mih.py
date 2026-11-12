import numpy as np, math
rng = np.random.default_rng(1)
X16 = np.concatenate([np.load(f"data/e{i:02d}.npy") for i in range(20)])
N, d = X16.shape
Rot, _ = np.linalg.qr(rng.standard_normal((d, d)).astype(np.float32))
si = rng.choice(N, 100_000, replace=False)
S = X16[si].astype(np.float32); S /= np.linalg.norm(S, axis=1, keepdims=True); gmean = S.mean(0)
NQ = 40
qi = rng.choice(N, NQ, replace=False)
Q = X16[qi].astype(np.float32); Q /= np.linalg.norm(Q, axis=1, keepdims=True)
codes = np.empty((N, d // 16), np.uint16); true = np.empty((N, NQ), np.float32)
w = (1 << np.arange(16)).astype(np.uint16)
def enc(x):
    bits = ((x - gmean) @ Rot) >= 0
    return (bits.reshape(len(x), d // 16, 16) * w).sum(2).astype(np.uint16)
for i in range(0, N, 50_000):
    x = X16[i:i+50_000].astype(np.float32); x /= np.linalg.norm(x, axis=1, keepdims=True)
    codes[i:i+50_000] = enc(x); true[i:i+50_000] = x @ Q.T
qcodes = enc(Q)
pc = np.array([bin(v).count("1") for v in range(65536)], np.uint8)
m = d // 16
for t in (0.98, 0.95, 0.9, 0.8, 0.7):
    rows = []
    for j in range(NQ):
        cd = pc[codes ^ qcodes[j]]            # N x 48 chunk distances
        H = cd.sum(1, dtype=np.int32)
        A = true[:, j] >= t
        if A.sum() < 2: continue
        r = int(H[A].max()); per = r // m
        cand = (cd <= per).any(1)
        assert cand[A].all()
        rows.append((A.sum(), r, H[~A].mean(), (H <= r).mean(), per, cand.mean(),
                     m * sum(math.comb(16, k) for k in range(per + 1))))
    R = np.array(rows, float)
    print(f"t={t}: queries {len(R)}  median answers {np.median(R[:,0]):.0f}  oracle Hamming radius median {np.median(R[:,1]):.0f}/768 "
          f"(background mean {np.median(R[:,2]):.0f})  frac within radius {np.median(R[:,3]):.4f}  "
          f"pigeonhole per-chunk radius {np.median(R[:,4]):.0f}  candidate frac median {np.median(R[:,5]):.4f} mean {R[:,5].mean():.4f}  "
          f"posting lists unioned {np.median(R[:,6]):.0f}")
