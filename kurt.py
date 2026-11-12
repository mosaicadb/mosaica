import numpy as np
rng = np.random.default_rng(2)
X = np.concatenate([np.load(f"data/e{i:02d}.npy") for i in (3, 11)]).astype(np.float32)
X /= np.linalg.norm(X, axis=1, keepdims=True)
# one local neighbourhood: k-means with 20 centres on this slice pair, take residuals within the biggest 4 clusters
C = X[rng.choice(len(X), 20, replace=False)].copy()
for _ in range(10):
    a = (X @ C.T).argmax(1)
    for k in range(20):
        if (a == k).any(): C[k] = X[a == k].mean(0)
    C /= np.linalg.norm(C, axis=1, keepdims=True)
for k in np.argsort(-np.bincount(a, minlength=20))[:3]:
    R = X[a == k]; R = R - R.mean(0)
    lam, V = np.linalg.eigh(R.T @ R / len(R)); V = V[:, ::-1]; lam = lam[::-1]
    Z = R @ V; Zs = Z / Z.std(0)
    kur = (Zs ** 4).mean(0) - 3
    # tail vectors of different items: how orthogonal are they, vs what isotropic noise predicts
    T = Z[:, 256:]; Tn = T / np.linalg.norm(T, axis=1, keepdims=True)
    idx = rng.choice(len(T), 2000, replace=False); Gm = Tn[idx] @ Tn[idx].T; off = Gm[np.triu_indices(2000, 1)]
    tl = lam[256:]; pred = np.sqrt((tl ** 2).sum()) / tl.sum()
    print(f"cluster of {len(R)}: excess kurtosis  dims 0-32 {np.median(kur[:32]):.2f}  32-256 {np.median(kur[32:256]):.2f}  256-768 {np.median(kur[256:]):.2f} | "
          f"tail energy share {tl.sum()/lam.sum():.2f} | cosine between two items' tails: std {off.std():.4f} (isotropic-noise prediction {pred:.4f}), max {np.abs(off).max():.3f}")
