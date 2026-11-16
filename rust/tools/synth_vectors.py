# The synthetic set of REPORT_BUILD.md: a mixture of 300 Gaussian clusters in 768 dimensions with a decaying spectrum,
# rotated at random. Writes OUT/x.npy (60,000 rows), OUT/heldout.npy (200 held-out items from the same mixture) and
# OUT/q.npy (50 points of the mixture without the rotation: they point elsewhere than the corpus, a stand-in for
# out-of-distribution queries).
# usage: python3 -I synth_vectors.py OUT
import os, sys
import numpy as np

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
rng = np.random.default_rng(1)
N, n, C = 60000, 768, 300
spec = 1.0 / np.arange(1, n + 1) ** 0.6
cent = rng.standard_normal((C, n)) * spec
x = cent[rng.integers(0, C, N)] + 0.9 * rng.standard_normal((N, n)) * spec
Q = np.linalg.qr(rng.standard_normal((n, n)))[0]
np.save(os.path.join(out, "x.npy"), (x @ Q).astype(np.float32))
q = cent[rng.integers(0, C, 50)] + 0.9 * rng.standard_normal((50, n)) * spec
np.save(os.path.join(out, "q.npy"), q.astype(np.float32))
r2 = np.random.default_rng(7)
h = cent[r2.integers(0, C, 200)] + 0.9 * r2.standard_normal((200, n)) * spec
np.save(os.path.join(out, "heldout.npy"), (h @ Q).astype(np.float32))
