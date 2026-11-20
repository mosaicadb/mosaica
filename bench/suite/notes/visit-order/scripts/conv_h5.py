"""An HDF5 dataset (train, test) as Xs.npy (f32 corpus) and queries.npy (the first 100 test vectors), normalised
when cosine; with N, a uniform sample of N corpus rows (seed 5). python3 -I conv_h5.py IN.hdf5 OUT_DIR cosine|ip [N]"""
import os
import sys

import h5py
import numpy as np

src, out, metric = sys.argv[1], sys.argv[2], sys.argv[3]
sample = int(sys.argv[4]) if len(sys.argv) > 4 else None
os.makedirs(out, exist_ok=True)
with h5py.File(src, "r") as f:
    print({k: f[k].shape for k in f.keys()})
    x = f["train"]
    if sample:
        x = x[np.sort(np.random.default_rng(5).choice(x.shape[0], sample, replace=False))]
    x = np.asarray(x, dtype=np.float64)
    q = np.asarray(f["test"][:100], dtype=np.float64)
if metric == "cosine":
    x /= np.maximum(np.linalg.norm(x, axis=1, keepdims=True), 1e-30)
    q /= np.maximum(np.linalg.norm(q, axis=1, keepdims=True), 1e-30)
np.save(os.path.join(out, "Xs.npy"), x.astype(np.float32))
np.save(os.path.join(out, "queries.npy"), q.astype(np.float32))
print(out, x.shape, q.shape, float(np.linalg.norm(x, axis=1).mean()), float(np.linalg.norm(q, axis=1).mean()))
