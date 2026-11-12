# Modelled single-thread time of a native implementation (a MODEL, not a measurement), and level-entry counts.
# t = groups * T_BASIS + clusters * T_CLUS + coded bytes * T_BYTE + full reads * T_FULL
import numpy as np, sys
from kcommon import *
NAME = sys.argv[1]; C = Ctx(NAME, need_rand=False); O = np.load(workdir(NAME) + "post.npy", allow_pickle=True)[0]; n = C.n
Sd = np.load(workdir(NAME) + "seed.npz"); CFG = [tuple(x) for x in Sd["CFG"]]; FN = ["all", "rand50", "rand10", "rand1", "rand0.1", "clusters10", "idrange10"]
T_BASIS = n * n / 4e9; T_CLUS = (10 * 3 * n + n) / 2e9; T_FULL = n / 2e9 + 2e-7       # 4 and 2 Gflop/s effective, plus a cache miss per full read
for cl in np.unique(C.qcls):
    m = C.qcls == cl
    for i, (k, f) in enumerate(CFG):
        o = O[("R", i, "seed")]; g = o["groups"][m].mean(); cls_ = o["clus"][m].mean() / (n + 4); coded = (o["items"] + o["seed"] + o["s3"])[m].mean() - o["full"][m].mean() * 2 * n
        base = g * T_BASIS + cls_ * T_CLUS + o["full"][m].mean() * T_FULL
        print(f"{NAME} {cl} k={k} {FN[f]}: groups {g:.1f} clusters {cls_:.0f} coded MB {coded / 1e6:.2f} full {o['full'][m].mean():.0f} | model ms at 1 / 4 ns per coded byte: "
              f"{1e3 * (base + coded * 1e-9):.1f} / {1e3 * (base + coded * 4e-9):.1f} | entering each level (mean): " + " ".join(f"{x:.0f}" for x in o["lev"][:, m].mean(1)))
