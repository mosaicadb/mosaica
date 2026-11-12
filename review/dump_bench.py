# dump one group's escape step order, a sample of normalised coordinates and a query for the C micro-benchmark
import numpy as np, sys
sys.path.insert(0, "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined")
from core import *
ROOT = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined/"
I = np.load(ROOT + "idx_m768/index.npz"); Xs = np.load(ROOT + "idx_m768/Xs.npy", mmap_mode="r")
grp, cstart, Mq = I["grp"], I["cstart"], I["Mq"]; g = 20; ks = np.where(grp == g)[0]
lam, V = I["lam"][g], I["V"][g]; sig = np.sqrt(lam).astype(np.float32)
se, dep_at, sdep, expc = esc_steps(lam, I["Qmix"][g], 1.5 * 8 * 576)
U = []
for c in ks[:20]:
    x = unit(Xs[int(cstart[c]):int(cstart[c + 1])]); U.append(((x - Mq[c]) @ V) / sig)
U = np.concatenate(U)[:16384].astype(np.float32)
q = I["Q"][-1]; qs = ((q - Mq[ks[0]]) @ V * sig).astype(np.float32)
se.astype(np.int32).tofile("b_se.bin"); sdep.astype(np.int32).tofile("b_sdep.bin"); U.tofile("b_u.bin"); qs.tofile("b_qs.bin"); CENT2.astype(np.float32).tofile("b_cent2.bin")
print("steps", len(se), "items", U.shape, "steps within 32/64/128/256 expected bits:", [(expc <= b).sum() for b in (32, 64, 128, 256)])
