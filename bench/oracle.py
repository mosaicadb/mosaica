# An exactness oracle that shares no code with the index: float64 brute force in numpy over the stored f16 vectors,
# with the query normalised and rounded to f16 as the index rounds it. Checks the ground truth the Rust side wrote
# (truth_<filter>_s.npy) and every exact and flat row of ours.csv: a query counts as wrong when its list of returned
# scores differs from the oracle's top-k scores by more than 1e-9 (ties may return different items).
# usage: python3 -I oracle.py DATASET_DIR RESULTS_DIR [--nq N]
import csv, os, sys
import numpy as np

K = 10
ddir, rdir = sys.argv[1], sys.argv[2]
nq = int(sys.argv[sys.argv.index("--nq") + 1]) if "--nq" in sys.argv else None
X = np.load(os.path.join(ddir, "index", "Xs.npy"), mmap_mode="r")
perm = np.load(os.path.join(ddir, "index", "perm.npy"))  # stored row -> entity
row_of = np.empty(perm.max() + 1, np.int64)
row_of[perm] = np.arange(len(perm))
n = X.shape[1]
Q = np.load(os.path.join(ddir, "test.npy")).astype(np.float64)
Q = Q[:nq] if nq else Q
qn = np.linalg.norm(Q, axis=1, keepdims=True)
Q = np.where(qn > 0, Q / np.where(qn > 0, qn, 1), 0).astype(np.float32).astype(np.float16).astype(np.float64)
Q = np.pad(Q, ((0, 0), (0, n - Q.shape[1])))


def oracle(rows):
    """Top-k exact scores (descending) of every query over the given stored rows."""
    out = np.empty((len(Q), K))
    best = np.full((len(Q), K), -np.inf)
    for a in range(0, len(rows), 200_000):
        Xc = np.asarray(X[rows[a:a + 200_000]], np.float64)
        for b in range(0, len(Q), 1000):
            S = Q[b:b + 1000] @ Xc.T
            kk = min(K, S.shape[1])
            top = -np.partition(-S, kk - 1, axis=1)[:, :kk]
            m = np.concatenate([best[b:b + 1000], top], axis=1)
            best[b:b + 1000] = -np.sort(-m, axis=1)[:, :K]
    out[:] = best
    return out


def scores_of(ids):
    """Exact scores of returned entity ids (-1 = none), descending."""
    s = np.full(ids.shape, -np.inf)
    for i in range(len(ids)):
        v = ids[i][ids[i] >= 0]
        s[i, :len(v)] = np.sort(np.asarray(X[np.sort(row_of[v])], np.float64) @ Q[i])[::-1]
    return s


def differ(a, b):
    a, b = np.where(np.isfinite(a), a, -9.0), np.where(np.isfinite(b), b, -9.0)
    return int((np.abs(a - b) > 1e-9).any(axis=1).sum())


tags = ["all"] + sorted(f[:-4] for f in os.listdir(os.path.join(ddir, "filters")) if f.startswith("vis_") and f.endswith(".npy"))
orc = {}
for tag in tags:
    rows = np.arange(len(perm)) if tag == "all" else np.sort(row_of[np.load(os.path.join(ddir, "filters", tag + ".npy"))])
    orc[tag] = oracle(rows)
    tp = os.path.join(rdir, f"truth_{tag}_s.npy")
    if os.path.exists(tp):
        t = np.load(tp)[:len(Q)]
        print(f"truth {tag}: {differ(t, orc[tag])} of {len(Q)} queries differ from the oracle", flush=True)
for r in csv.DictReader(open(os.path.join(rdir, "ours.csv"))):
    if r["method"] not in ("exact", "flat") or r["filter"] not in orc:
        continue
    ids = np.load(os.path.join(rdir, r["ids"]))
    m = min(len(ids), len(Q))
    print(f'{r["method"]} {r["filter"]}: {differ(scores_of(ids[:m]), orc[r["filter"]][:m])} of {m} queries differ from the oracle', flush=True)
