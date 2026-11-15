# Nested row subsets of c5m_x.npy: the 1M rows (and the 1,000 held-out rows), then the same share of each shard's
# 200,000 extra rows, in a fixed random order per shard: 2M, 3M, 4M (5M is every row).
import sys, numpy as np
D = sys.argv[1]
shard = np.load(f"{D}/c5m_shard.npy")
base = np.where(shard < 0)[0]
perm = {s: np.random.default_rng(2000 + s).permutation(np.where(shard == s)[0]) for s in range(20)}
for m in [2, 3, 4]:
    k = (m * 1_000_000 - len(base)) // 20
    rows = np.sort(np.concatenate([base] + [perm[s][:k] for s in range(20)]))
    np.save(f"{D}/c{m}m_rows.npy", rows); print(m, len(rows), k)
