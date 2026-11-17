# Runs rust/tools/fit.py unchanged (its main(), same argv), only recording what its k-means calls return: the first
# call is the clustering (C, K0 x n, normalised spherical k-means centres before empty clusters are dropped), the
# second the grouping. Saved to OUTDIR/kmeans_C.npy (first call) next to the fit's own files.
# usage: python -I fitwrap.py /path/to/fit.py VECTORS.npy OUTDIR [fit.py options]
import importlib.util
import os
import sys

fitpy = sys.argv[1]
sys.argv = [fitpy] + sys.argv[2:]
spec = importlib.util.spec_from_file_location("fit_mod", fitpy)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
out = sys.argv[2]
_km = mod.kmeans
calls = []


def kmeans(*a, **kw):
    C = _km(*a, **kw)
    if not calls:
        import numpy as np
        os.makedirs(out, exist_ok=True)
        np.save(os.path.join(out, "kmeans_C.npy"), C)
    calls.append(C.shape)
    return C


mod.kmeans = kmeans
mod.main()
print("kmeans calls:", calls, flush=True)
