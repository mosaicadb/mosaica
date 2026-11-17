# Extra queries from a suite HDF5 file (ANN-Benchmarks / VIBE): 1,000 test rows other than the suite's 100 (query_ids),
# normalised in f64 as prep.py does, with the file's own `neighbors` (first 10) as truth. Also prints the file's keys
# and attributes (VIBE's metadata on the query distribution).
# usage: python -I hdf5_extra.py FILE.hdf5 PREPDIR OUTDIR
import sys, os
import numpy as np
import h5py

p, prep, out = sys.argv[1:4]
os.makedirs(out, exist_ok=True)
with h5py.File(p, "r") as f:
    print("keys", {k: f[k].shape for k in f.keys()}, "attrs", dict(f.attrs), flush=True)
    test = f["test"][:]
    nb = f["neighbors"][:, :10]
used = set(np.load(os.path.join(prep, "query_ids.npy")).tolist())
rest = np.array([i for i in range(len(test)) if i not in used])
pick = np.sort(np.random.default_rng(11).choice(rest, min(1000, len(rest)), replace=False))
Q = test[pick].astype(np.float64)
Q /= np.linalg.norm(Q, axis=1, keepdims=True)
np.save(os.path.join(out, "queries.npy"), Q)
np.save(os.path.join(out, "gt.npy"), nb[pick].astype(np.int64))
# check the file's neighbours against prep's truth on the suite's 100 queries
qi = np.load(os.path.join(prep, "query_ids.npy"))
o = np.load(os.path.join(prep, "order.npy"), mmap_mode="r")[:, :10]
print("file neighbours vs prep truth on the suite's queries: mean overlap@10",
      np.mean([len(set(a) & set(b)) / 10 for a, b in zip(nb[qi], o)]), flush=True)
