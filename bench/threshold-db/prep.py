# Corpus, queries, thresholds, truth and visible sets for the threshold-filter comparison.
# usage: python3 -I prep.py RAW_DIR TEXT_DIR OUT_DIR
# RAW_DIR holds e00..e19.npy (f16 BioCLIP-2 rows from fetch.py). 50 random rows are held out as image queries; the rest,
# normalised in f64, is the corpus (id = row). Writes to OUT_DIR:
#   corpus16.npy / corpus32.npy   unit vectors rounded to f16 / f32
#   queries.npy                   150 x 768 f64 unit vectors: 50 held-out images, 50 text_q, 50 text_insample
#   thresholds.npy                150 x 7 (shares 0.01% to 75%): midpoints between the k-th and (k+1)-th f64 similarity, k = share x N
#   order.npy                     150 x 750k int32: ids by f64 similarity, best first (the exact answer is a prefix)
#   vis_50.npy, vis_10.npy, vis_1.npy   sorted int32 ids of random visible sets
import os, sys, glob, numpy as np
raw, text, out = sys.argv[1:4]
SHARES = [1e-4, 1e-3, 1e-2, 1e-1, 0.3, 0.5, 0.75]
A = np.concatenate([np.load(f) for f in sorted(glob.glob(f"{raw}/e*.npy"))])
rng = np.random.default_rng(20261114)
held = rng.choice(len(A), 50, replace=False)
keep = np.ones(len(A), bool)
keep[held] = False


def unit(M):
    M = M.astype(np.float64)
    return M / np.linalg.norm(M, axis=1, keepdims=True)


Q = np.concatenate([unit(A[held]),
                    unit(np.load(f"{text}/text_q.npy")[rng.choice(369, 50, replace=False)]),
                    unit(np.load(f"{text}/text_insample.npy")[rng.choice(361, 50, replace=False)])])
X = unit(A[keep])
del A
N = len(X)
print("corpus", X.shape, "queries", Q.shape, flush=True)
if not os.path.exists(f"{out}/corpus32.npy"):  # a rerun reproduces these; the databases and the index hold them
    np.save(f"{out}/corpus16.npy", X.astype(np.float16))
    np.save(f"{out}/corpus32.npy", X.astype(np.float32))
    np.save(f"{out}/queries.npy", Q)
assert np.array_equal(np.load(f"{out}/queries.npy"), Q)
ks = [round(s * N) for s in SHARES]
K = max(ks)
order = np.empty((len(Q), K), np.int32)
thr = np.empty((len(Q), len(ks)))
for i0 in range(0, len(Q), 10):
    S = X @ Q[i0:i0 + 10].T
    for j in range(S.shape[1]):
        s = S[:, j]
        top = np.argpartition(-s, K)[:K + 1]
        top = top[np.argsort(-s[top], kind="stable")]
        order[i0 + j] = top[:K]
        thr[i0 + j] = [(s[top[k - 1]] + s[top[k]]) / 2 for k in ks]
    print("truth", i0 + 10, flush=True)
np.save(f"{out}/order.npy", order)
np.save(f"{out}/thresholds.npy", thr)
for f in (50, 10, 1):
    if not os.path.exists(f"{out}/vis_{f}.npy"):
        np.save(f"{out}/vis_{f}.npy", np.sort(rng.choice(N, round(N * f / 100), replace=False)).astype(np.int32))
print("median threshold by share, image / text_q / text_insample:")
for a in range(3):
    print(np.median(thr[50 * a:50 * a + 50], 0).round(4))
