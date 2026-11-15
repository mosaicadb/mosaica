# Nested samples: the shard of each c1m row (as assemble.py drew them), a 500k half of c1m stratified by shard, and
# 200 item queries drawn from the 500k rows (so they are corpus items at 500k, 1M and 5M).
import sys, numpy as np
D = sys.argv[1]
p = np.random.default_rng(7).permutation(1_000_000)          # assemble.py's permutation
rest = np.sort(p[1000:])
shard = (rest // 50_000).astype(np.int16)
np.save(f"{D}/c1m_shard.npy", shard)
X = np.load(f"{D}/c1m_x.npy", mmap_mode="r")
assert len(X) == len(rest)
rng = np.random.default_rng(500)
half = np.sort(np.concatenate([rng.choice(np.where(shard == s)[0], (shard == s).sum() // 2, replace=False) for s in range(20)]))
np.save(f"{D}/c500k_rows.npy", half)                          # rows of c1m_x
np.save(f"{D}/c500k_x.npy", np.asarray(X[half]))
q = np.sort(rng.choice(half, 200, replace=False))
np.save(f"{D}/scale_item_rows.npy", q)                        # rows of c1m_x (also the first rows of c5m_x)
np.save(f"{D}/scale_item_q.npy", np.asarray(X[q], np.float32))
print(len(half), np.bincount(shard[half]), q[:5])
