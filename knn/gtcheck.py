# Agreement of this study's ground truth on GloVe (fp16 rows, renormalised) with the published ann-benchmarks neighbours.
import numpy as np
from kcommon import *
C = Ctx("gfull", need_rand=False); Sd = np.load(workdir("gfull") + "seed.npz"); nb = np.load(KNN + "gdata/neighbors.npy")[:200]
mine = C.perm[Sd["GTI"][0][:, :100]]
for k in (1, 10, 100): print("k", k, "mean overlap with published top k:", np.mean([len(set(mine[q, :k]) & set(nb[q, :k])) / k for q in range(200)]))
