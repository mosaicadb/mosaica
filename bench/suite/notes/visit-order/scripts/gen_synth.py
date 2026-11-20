"""Synthetic mixtures for the stress study: 100 clusters of 2,000 rows, centres N(0, 0.25 I), each cluster an
anisotropic covariance (a random rotation, variances falling as 1/j, trace 0.5 d); Gaussian rows, or multivariate
Student-t(3) rows scaled to the same covariance. Queries: 100 more rows of the same mixture.
python3 -I gen_synth.py OUT_DIR d gauss|t3"""
import os
import sys

import numpy as np

out, d, kind = sys.argv[1], int(sys.argv[2]), sys.argv[3]
rng = np.random.default_rng(11 + d)
K, M = 100, 2000
centres = rng.normal(0.0, 0.5, size=(K, d))
lam = 1.0 / np.arange(1, d + 1)
lam *= 0.5 * d / lam.sum()
mats = []
for c in range(K):
    q, _ = np.linalg.qr(rng.normal(size=(d, d)))
    mats.append(q * np.sqrt(lam))  # rows x = centre + (q sqrt(lam)) z
draw = np.random.default_rng(99 + d)


def sample(n_per, clusters):
    xs = []
    for c in clusters:
        z = draw.normal(size=(n_per, d))
        if kind == "t3":
            w = np.sqrt(draw.chisquare(3, size=(n_per, 1)) / 3.0)
            z = z / w / np.sqrt(3.0)
        xs.append(centres[c] + z @ mats[c].T)
    return np.concatenate(xs)


x = sample(M, range(K))
q = np.concatenate([sample(1, [c]) for c in draw.integers(0, K, size=100)])
os.makedirs(out, exist_ok=True)
np.save(os.path.join(out, "Xs.npy"), x.astype(np.float32))
np.save(os.path.join(out, "queries.npy"), q.astype(np.float32))
print(out, x.shape, q.shape, float(np.linalg.norm(x, axis=1).mean()), float(np.abs(x).max()))
