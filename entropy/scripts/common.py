import numpy as np, time, sys
BASE = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
def load(name):
    if name == "bioclip768": fs = [BASE + f"data/e{i:02d}.npy" for i in (0, 5, 10, 15)]
    elif name == "bioclip1024": fs = [BASE + f"agent3b/data1024/f{i:02d}.npy" for i in range(4)]
    else: fs = [BASE + f"entropy/data/{name}.npy"]
    X = np.concatenate([np.load(f).astype(np.float32) for f in fs])
    nr = np.linalg.norm(X, axis=1); X = X[nr > 0]; X /= np.linalg.norm(X, axis=1, keepdims=True)
    return X
def assign(X, M, chunk=20000):
    out = np.empty(len(X), np.int32); m2 = (M * M).sum(1)
    for s in range(0, len(X), chunk): out[s:s + chunk] = (m2[None] - 2 * X[s:s + chunk] @ M.T).argmin(1)
    return out
def kmeans(X, K, iters=15, seed=0):
    rng = np.random.default_rng(seed); M = X[rng.choice(len(X), K, replace=False)].copy()
    for it in range(iters):
        a = assign(X, M)
        for c in range(K):
            sel = a == c
            M[c] = X[sel].mean(0) if sel.any() else X[rng.integers(len(X))]
    return M, assign(X, M)
T0 = time.time()
def log(*a):
    print(f"[{time.time() - T0:7.1f}s]", *a, flush=True)
