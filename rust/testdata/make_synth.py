# Makes testdata/synth4k, the fitted index the end-to-end tests build: 4,000 unit vectors in 64 dimensions drawn
# around 40 directions with a spread that varies by direction, fitted by tools/fit.py at 125 items per cluster.
# usage: python3 -I make_synth.py   (needs numpy and scipy; writes beside this file; deterministic)
import json, os, subprocess, sys, tempfile
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "synth4k")
N, D, C = 4000, 64, 40

rng = np.random.default_rng(20261115)
centres = rng.standard_normal((C, D))
centres /= np.linalg.norm(centres, axis=1, keepdims=True)
spread = rng.uniform(0.15, 0.6, C)
which = rng.integers(0, C, N)
X = centres[which] + spread[which, None] * rng.standard_normal((N, D)) / np.sqrt(D)
X /= np.linalg.norm(X, axis=1, keepdims=True)

with tempfile.TemporaryDirectory() as tmp:
    src = os.path.join(tmp, "synth4k.npy")
    np.save(src, X.astype(np.float32))
    subprocess.run([sys.executable, "-I", os.path.join(HERE, "..", "tools", "fit.py"), src, OUT, "--per-cluster", "125",
                    "--cert-time", "120"], check=True)
meta_path = os.path.join(OUT, "meta.json")
meta = json.load(open(meta_path))
meta["source"] = "testdata/make_synth.py"
json.dump(meta, open(meta_path, "w"), indent=1)
