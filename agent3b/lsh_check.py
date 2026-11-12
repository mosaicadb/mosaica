# Expectation check for the hyperplane-LSH miss formula: 20 disjoint groups of 8 tables (k = 12), t = 0.95.
import numpy as np
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = unit(np.concatenate([np.load(f"../data/{f}.npy") for f in ["e00", "e05", "e10", "e15"]]))
qi = np.load("clusters_data.npz")["qi"]; Q = X[qi]; true = X @ Q.T; N, n = X.shape
k = 12; LT = 160; rng = np.random.default_rng(123); H = rng.standard_normal((n, k * LT)).astype(np.float32)
pw = (1 << np.arange(k)).astype(np.uint16)
def keys(Y):
    out = np.empty((len(Y), LT), np.uint16)
    for i in range(0, len(Y), 10000): out[i:i+10000] = ((Y[i:i+10000] @ H > 0).reshape(-1, LT, k) * pw).sum(2)
    return out
KX = keys(X); KQ = keys(Q); t = 0.95; obs = np.zeros(20); pred = 0.0
for q in range(len(qi)):
    c = np.clip(true[:, q], -1, 1); M = c >= t; eq = KX[M] == KQ[q][None]
    obs += (eq.reshape(M.sum(), 20, 8).sum(2) == 0).sum(0)
    pred += ((1 - (1 - np.arccos(c[M]) / np.pi) ** k) ** 8).sum()
print("predicted misses per group of 8 tables", round(pred), "observed mean", round(obs.mean()), "sd", round(obs.std()), "min", obs.min(), "max", obs.max())
