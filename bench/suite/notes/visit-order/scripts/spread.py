# Spread over clusters of q.m (text test queries vs corpus pseudo-queries) against the spread of f * rad.
import sys, os, numpy as np
fit, prep = sys.argv[1:3]
mq = np.load(f"{fit}/mq8.npy").astype(np.float64) * np.load(f"{fit}/msc.npy")[:, None]
cs = np.load(f"{fit}/cstart.npy"); Xs = np.load(f"{fit}/Xs.npy", mmap_mode="r")
K = len(cs) - 1
rad = np.empty(K)
for j in range(K):
    x = np.asarray(Xs[cs[j]:cs[j + 1]], np.float64); rad[j] = np.sqrt(((x - mq[j]) ** 2).sum(1).mean())
Q = np.load(f"{prep}/queries.npy")
P = np.asarray(Xs[np.random.default_rng(0).choice(len(Xs), 1000, replace=False)], np.float64)
for name, A in (("text test", Q), ("corpus pseudo", P)):
    s = A @ mq.T
    top = np.sort(s, 1)[:, ::-1]
    print(f"{name:14s}: q.m std over clusters {s.std(1).mean():.4f}; best - 20th best q.m {np.mean(top[:, 0] - top[:, 19]):.4f}")
print(f"rad std {rad.std():.4f}, range {rad.min():.3f}..{rad.max():.3f}; 0.5*rad std {0.5 * rad.std():.4f}; 0.1*rad std {0.1 * rad.std():.4f}")
