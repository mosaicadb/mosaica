# Convert an ANN-Benchmarks HDF5 file to .npy files beside it: train (float32), test (float32), neighbors (int32),
# distances (float32).
# usage: python3 -I convert.py DATASET.hdf5
import sys, os
import numpy as np, h5py

src = sys.argv[1]
out = os.path.dirname(os.path.abspath(src))
with h5py.File(src, "r") as f:
    print({k: (f[k].shape, f[k].dtype) for k in f.keys()}, dict(f.attrs))
    for k, dt in (("train", np.float32), ("test", np.float32), ("neighbors", np.int32), ("distances", np.float32)):
        a = np.ascontiguousarray(f[k][:].astype(dt))
        np.save(os.path.join(out, k + ".npy"), a)
        if a.dtype == np.float32 and k in ("train", "test"):
            nrm = np.linalg.norm(a, axis=1)
            print(k, a.shape, "norms min %.4g max %.4g, zero rows %d" % (nrm.min(), nrm.max(), int((nrm == 0).sum())))
