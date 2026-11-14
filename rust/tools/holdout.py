# Split vector files into a corpus and held-out item queries: concatenates the inputs, draws HOLD rows at random as
# queries (written as f32), and writes the remaining rows, in input order, as the corpus (f16).
# usage: python3 -I holdout.py OUTDIR HOLD [--limit N] [--seed S] FILE.npy...
import argparse, os
import numpy as np

ap = argparse.ArgumentParser()
ap.add_argument("out")
ap.add_argument("hold", type=int)
ap.add_argument("files", nargs="+")
ap.add_argument("--limit", type=int, default=0, help="use only the first N rows of the concatenation")
ap.add_argument("--seed", type=int, default=7)
a = ap.parse_args()
X = np.concatenate([np.load(f) for f in a.files])
if a.limit:
    X = X[:a.limit]
rng = np.random.default_rng(a.seed)
q = np.sort(rng.choice(len(X), a.hold, replace=False))
keep = np.ones(len(X), bool)
keep[q] = False
os.makedirs(a.out, exist_ok=True)
np.save(os.path.join(a.out, "corpus.npy"), X[keep].astype(np.float16))
np.save(os.path.join(a.out, "item_q.npy"), X[q].astype(np.float32))
print(f"corpus {keep.sum()} rows, {a.hold} held-out queries, from {len(X)} rows of {len(a.files)} files")
