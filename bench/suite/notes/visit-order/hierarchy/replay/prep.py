# Builds and caches what the replays share: the score matrix, the sub-cluster splits (4, 16, 64 per cluster of the
# real fit) and the refits at K = 5,000 and 20,000 (faiss k-means; a different partition from the real fit).
# usage: python3 -I prep.py DS
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import DS, subsplit, refit, log
import numpy as np

D = DS(sys.argv[1])
log(f"{D.name}: N {D.N} d {D.d} K {D.K} G {D.grp.max() + 1}")
for s in (4, 16, 64):
    z = subsplit(D, s)
    n = np.bincount(z["sub"])
    log(f"sub x{s}: {len(n)} leaves, sizes {n.min()}..{n.max()} median {int(np.median(n))}")
for K, it, smp in ((5000, 10, 400_000), (20000, 6, 200_000)):
    z = refit(D, K, it, smp)
    n = np.bincount(z["a"])
    log(f"refit K {K}: {len(n)} clusters, sizes {n.min()}..{n.max()} median {int(np.median(n))}")
