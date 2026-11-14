# Spectrum of the query-offset covariance (offset = query minus its component along a cluster centre), for Theorem 4's constant.
import numpy as np, sys
W = sys.argv[1]; rng = np.random.default_rng(1)
C = np.load(f"{W}/index.npz")["Chat"]; X = np.load(f"{W}/X.npy", mmap_mode="r")
Qt = np.load(f"{W}/text_q.npy").astype(np.float32); Qt /= np.linalg.norm(Qt, axis=1, keepdims=True)
Qi = np.asarray(X[np.sort(rng.choice(len(X), 369, replace=False))])
for name, Q in (("text", Qt), ("item", Qi)):
    S = np.zeros((C.shape[1], C.shape[1])); cnt = 0
    for q in Q:
        c = C[rng.choice(len(C), 100, replace=False)]; D = q[None] - (c @ q)[:, None] * c
        S += D.T @ D; cnt += len(D)
    S /= cnt; l = np.linalg.eigvalsh(S)[::-1]; tr = l.sum()
    print(f"{name}: mean |offset|^2 {tr:.3f}; top eigenvalue share {l[0]/tr:.3f}; effective rank tr/lmax {tr/l[0]:.1f}; "
          f"after removing the top 1/4/16 directions {l[1:].sum()/l[1]:.1f} / {l[4:].sum()/l[4]:.1f} / {l[16:].sum()/l[16]:.1f}; participation ratio {tr**2/(l**2).sum():.1f}")
