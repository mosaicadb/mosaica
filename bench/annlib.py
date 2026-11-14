# Shared pieces of the ANN-Benchmarks runs: datasets, visible sets, recall by the ANN-Benchmarks rule.
import os
import numpy as np

K = 10
FRACS = [0.5, 0.1, 0.01, 0.001]


def unit(a):
    a = np.asarray(a, np.float32)
    n = np.linalg.norm(a, axis=1, keepdims=True)
    return a / np.where(n > 0, n, 1)


def load(ddir):
    """Train (unit rows), test (as stored), and the published neighbours and distances."""
    return dict(dir=ddir, train=unit(np.load(os.path.join(ddir, "train.npy"))), test=np.load(os.path.join(ddir, "test.npy")),
                neighbors=np.load(os.path.join(ddir, "neighbors.npy")), distances=np.load(os.path.join(ddir, "distances.npy")))


def tag(frac):
    return f"vis_{frac:g}"


def angular(q, x):
    """ANN-Benchmarks angular distance, 1 - cos, in float64 (nan for a zero query)."""
    q = np.asarray(q, np.float64)
    with np.errstate(invalid="ignore", divide="ignore"):
        return 1 - (x.astype(np.float64) @ q) / (np.linalg.norm(x.astype(np.float64), axis=1) * np.linalg.norm(q))


def kth_distance(ds, ftag):
    """The k-th true distance of every query: published for the whole set, brute force (filters.py) for a visible set."""
    if ftag == "all":
        return ds["distances"][:, K - 1].astype(np.float64)
    return np.load(os.path.join(ds["dir"], "filters", f"gtd_{ftag}.npy"))[:, K - 1].astype(np.float64)


def recall(ds, ids, ftag):
    """Mean over the queries of `ids` (the first len(ids) test queries) of the fraction of the k returned items within the k-th true distance + 1e-3."""
    X, Q = ds["train"], ds["test"]
    thr = kth_distance(ds, ftag) + 1e-3
    out = np.zeros(len(ids))
    for i in range(len(ids)):
        r = ids[i][ids[i] >= 0][:K]
        if len(r) == 0:
            continue
        d = angular(Q[i], X[r])
        out[i] = np.sum(d <= thr[i]) / K
    return float(out.mean())
