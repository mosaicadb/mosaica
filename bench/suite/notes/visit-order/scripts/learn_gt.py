# 1,000 learn text queries (the fit's --query-sample population, not the test queries) and their exact top 10 entity
# ids over the corpus (f32 products of the f32 query with the stored f16 vectors, as the suite's truth).
# usage: python -I learn_gt.py PREPDIR OUTDIR
import sys, os, time
import numpy as np

prep, out = sys.argv[1], sys.argv[2]
X16 = np.load(os.path.join(prep, "corpus16.npy"), mmap_mode="r")
L = np.load(os.path.join(prep, "learn.npy"), mmap_mode="r")
rng = np.random.default_rng(7)
pick = np.sort(rng.choice(len(L), 1000, replace=False))
Q = np.asarray(L[pick], np.float32)
Q /= np.linalg.norm(Q, axis=1, keepdims=True)
# not test queries: check none equals a test query
T = np.load(os.path.join(prep, "queries.npy")).astype(np.float32)
assert (Q @ T.T).max() < 0.9999, "a learn query equals a test query"
k = 10
bs, bi = np.full((len(Q), 0), -np.inf, np.float32), np.zeros((len(Q), 0), np.int64)
t0 = time.time()
for i in range(0, X16.shape[0], 100_000):
    S = Q @ np.asarray(X16[i:i + 100_000], np.float32).T
    s = np.concatenate([bs, S], 1)
    ii = np.concatenate([bi, np.broadcast_to(np.arange(i, i + S.shape[1]), S.shape)], 1)
    top = np.argpartition(-s, k - 1, axis=1)[:, :k]
    bs, bi = np.take_along_axis(s, top, 1), np.take_along_axis(ii, top, 1)
o = np.argsort(-bs, 1, kind="stable")
np.save(os.path.join(out, "learn_q.npy"), Q)
np.save(os.path.join(out, "learn_gt.npy"), np.take_along_axis(bi, o, 1))
print(f"learn truth: {len(Q)} queries in {time.time() - t0:.0f} s; mean top-1 sim {np.take_along_axis(bs, o, 1)[:, 0].mean():.3f}, "
      f"10th {np.take_along_axis(bs, o, 1)[:, 9].mean():.3f}")
