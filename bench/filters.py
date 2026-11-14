# Visible sets for the filtered runs (random, fixed seed) and their float brute-force ground truth.
# usage: python3 -I filters.py DATASET_DIR
#   writes DATASET_DIR/filters/vis_<frac>.npy (sorted entity ids, int32), gt_vis_<frac>.npy (k nearest visible ids)
#   and gtd_vis_<frac>.npy (their angular distances), k = 10.
import os, sys
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from annlib import FRACS, K, load, tag, unit  # noqa: E402

ddir = sys.argv[1]
out = os.path.join(ddir, "filters")
os.makedirs(out, exist_ok=True)
ds = load(ddir)
X, Q = ds["train"], unit(ds["test"])
N = len(X)
rng = np.random.default_rng(7)
for f in FRACS:
    vis = np.sort(rng.choice(N, max(1, int(N * f)), replace=False)).astype(np.int32)
    np.save(os.path.join(out, tag(f) + ".npy"), vis)
    Xv = X[vis]
    ids = np.zeros((len(Q), K), np.int32)
    dist = np.zeros((len(Q), K), np.float32)
    for i in range(0, len(Q), 500):
        S = (Q[i:i + 500] @ Xv.T).astype(np.float64)
        top = np.argpartition(-S, K, axis=1)[:, :K]
        s = np.take_along_axis(S, top, 1)
        o = np.argsort(-s, axis=1)
        ids[i:i + 500] = vis[np.take_along_axis(top, o, 1)]
        dist[i:i + 500] = 1 - np.take_along_axis(s, o, 1)
    np.save(os.path.join(out, "gt_" + tag(f) + ".npy"), ids)
    np.save(os.path.join(out, "gtd_" + tag(f) + ".npy"), dist)
    print(tag(f), len(vis), "visible")
