# Spread of log2(rho / pred) (in 1/16-octave steps) for the relative norm code, per deterministic level, a few groups of p768.
import numpy as np, sys
sys.path.insert(0, "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined")
from core import *
VEC = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
NAME = sys.argv[1] if len(sys.argv) > 1 else "p768"
import os
IDX = VEC + (f"combined/idx_{NAME}/" if os.path.exists(VEC + f"combined/idx_{NAME}/index.npz") else f"quant/idx_{NAME}/")
I = dict(np.load(IDX + "index.npz")); Xs = np.load(IDX + "Xs.npy", mmap_mode="r")
G = int(I["G"]); grp, cstart, Mq, mn, ahat = I["grp"], I["cstart"], I["Mq"], I["mn"], I["ahat"]
gstart = np.searchsorted(grp, np.arange(G + 1)); LVS = [4, 8, 16, 32]
for g in range(0, G, max(1, G // 4)):
    lam, V, Qm = I["lam"][g], I["V"][g], I["Qmix"][g]; sig = np.sqrt(lam); lamsum = lam.sum()
    se, dep_at, sdep, expc = esc_steps(lam, Qm, 2 * 8 * 32); s2 = np.where(sdep == 2)[0]
    ks = np.arange(gstart[g], gstart[g + 1]); res = {b: ([], []) for b in LVS}
    for c in ks[:: max(1, len(ks) // 6)]:
        x = unit(Xs[cstart[c]:cstart[c + 1]]); z = (x - Mq[c]) @ V; u = z / sig; m_ = mn[c]
        ah = ahat[cstart[c]:cstart[c + 1]] / 32767; R2 = 1 + m_ * m_ - 2 * m_ * ah
        Cst = np.ones((len(x), len(se)), np.int32); Cst[:, s2] = np.floor(np.abs(u) / W).astype(np.int32)[:, se[s2]] + 1; cum = np.cumsum(Cst, 1)
        for b in LVS:
            de = dep_at[int((expc <= 8 * b).sum())]; D = ((lam * gE[de] + 1e-10) / (Qm + 1e-10)) ** 0.25
            dep = dep_at[(cum <= 8 * b).sum(1)]; code = recon_esc(u, dep) * sig; rho = np.linalg.norm((z - code) / D, axis=1)
            iD2 = 1 / D ** 2; s2it = R2 / lamsum
            ec = (s2it[:, None] * lam[None] * gE[dep]) * (dep > 0)
            Uen = np.maximum(R2 - (code ** 2).sum(1) - ec.sum(1), 1e-12); lu = lam[None] * (dep == 0)
            p1 = np.sqrt((ec * iD2).sum(1) + Uen * (lu * iD2).sum(1) / lu.sum(1))
            # alternative: exact uncoded energy split by the unweighted sphere, coded part from per-cell conditional variance
            p2 = np.sqrt(((lam * gE[dep]) * iD2).sum(1) * s2it)
            res[b][0].append(np.log2(rho / p1) * 16); res[b][1].append(np.log2(rho / p2) * 16)
    print(f"group {g}:", " | ".join(f"{b} B: sd {np.std(np.concatenate(res[b][0])):.1f} q.005-.995 {np.quantile(np.concatenate(res[b][0]), .005):.0f}..{np.quantile(np.concatenate(res[b][0]), .995):.0f}"
                                   f" (simple sd {np.std(np.concatenate(res[b][1])):.1f})" for b in LVS))
