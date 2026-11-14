# 20 fetched row groups -> 1M rows; 1,000 random rows held out as item queries; the rest is the 1M index's input
# (999,000 rows); a random 50k of those is the 50k index's input, with 200 of the held-out rows as its item queries.
import sys, numpy as np
D = sys.argv[1]
X = np.concatenate([np.load(f"{D}/raw/e{i:02d}.npy") for i in range(20)])
print(X.shape, X.dtype, np.isfinite(X.astype(np.float32)).all())
rng = np.random.default_rng(7)
p = rng.permutation(len(X))
hold, rest = np.sort(p[:1000]), np.sort(p[1000:])
np.save(f"{D}/c1m_x.npy", X[rest]); np.save(f"{D}/c1m_item_q.npy", X[hold].astype(np.float32))
s50 = np.sort(rng.choice(rest, 50_000, replace=False))
np.save(f"{D}/c50k_x.npy", X[s50]); np.save(f"{D}/c50k_item_q.npy", X[hold[:200]].astype(np.float32))
print(len(rest), len(s50))
