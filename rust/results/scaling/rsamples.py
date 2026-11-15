# The 5M population (c5m_x.npy): the shard of every row, nested uniform samples R1M (50,000 per shard) within R3M
# (150,000 per shard) over all 250,000 population rows of each shard, and 200 item queries drawn from R1M.
import sys, numpy as np
D = sys.argv[1]
sh = np.load(f"{D}/c5m_shard.npy").astype(np.int64)
p = np.random.default_rng(7).permutation(1_000_000); hold = np.sort(p[:1000])     # assemble.py's draw
c1 = np.load(f"{D}/c1m_shard.npy").astype(np.int64)
sh[:len(c1)] = c1; sh[len(c1):len(c1) + 1000] = hold // 50_000
assert (sh >= 0).all() and (np.bincount(sh) == 250_000).all()
np.save(f"{D}/pop_shard.npy", sh.astype(np.int16))
rng = np.random.default_rng(3000)
perm = [rng.permutation(np.where(sh == s)[0]) for s in range(20)]
r1 = np.sort(np.concatenate([x[:50_000] for x in perm])); r3 = np.sort(np.concatenate([x[:150_000] for x in perm]))
assert np.isin(r1, r3).all()
np.save(f"{D}/r1m_rows.npy", r1); np.save(f"{D}/r3m_rows.npy", r3)
X = np.load(f"{D}/c5m_x.npy", mmap_mode="r")
q = np.sort(rng.choice(len(r1), 200, replace=False))                               # positions in R1M (entities)
np.save(f"{D}/r_item_pos.npy", q); np.save(f"{D}/r_item_q.npy", np.asarray(X[r1[q]], np.float32))
print(len(r1), len(r3), "R1M rows in the original 1M row groups:", int((r1 < 1_000_000).sum()))
