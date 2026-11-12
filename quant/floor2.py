# Where the deterministic levels sit against the floors, before and after, on real residuals with Gaussian query offsets.
# Mean CS half-width E||D Delta|| * rho_stored (Delta ~ N(0, diag Q) in the group basis; alpha and the sphere ignored), against
#   conjectured floor sum_i sqrt(min(gamma_i, theta)) and proved floor max_k kappa_k (prod gamma)^(1/2k) 2^(-B/k) (Corollary 1),
# with B = every stored bit through that level (code + a + norms).  usage: python3 floor2.py NAME
import numpy as np, sys, os, math
from scipy.special import gammaln
sys.path.insert(0, "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined")
from core import *
VEC = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
NAME = sys.argv[1]
IDX = VEC + (f"combined/idx_{NAME}/" if os.path.exists(VEC + f"combined/idx_{NAME}/index.npz") else f"quant/idx_{NAME}/")
I = dict(np.load(IDX + "index.npz")); Xs = np.load(IDX + "Xs.npy", mmap_mode="r")
G = int(I["G"]); n = int(I["n"]); grp, cstart, Mq, mn, ahat = I["grp"], I["cstart"], I["Mq"], I["mn"], I["ahat"]
gstart = np.searchsorted(grp, np.arange(G + 1))
def logkappa(k): return math.log(math.sqrt(2)) + gammaln((k + 1) / 2) - gammaln(k / 2) + 0.5 * math.log(2 * math.pi * math.e) - (1 / k) * (k / 2 * math.log(math.pi) - gammaln(k / 2 + 1))
def floors(gam, B):
    g = np.sort(gam)[::-1]; g = g[g > 1e-30]; lg = np.cumsum(np.log(g)); best = 0
    for k in range(1, len(g) + 1):
        best = max(best, math.exp(logkappa(k) + lg[k - 1] / (2 * k) - B * math.log(2) / k))
    lo, hi = 1e-30, g.max()
    for _ in range(200):
        th = math.sqrt(lo * hi)
        if 0.5 * np.log2(np.maximum(g / th, 1)).sum() > B: lo = th
        else: hi = th
    return best, np.sqrt(np.minimum(g, th)).sum()
LV = [4, 8, 12, 16, 24, 32]
rng = np.random.default_rng(1)
acc = {}
for g in range(G):
    ks = np.arange(gstart[g], gstart[g + 1])
    if cstart[ks[-1] + 1] - cstart[ks[0]] < 2000: continue
    lam, V = I["lam"][g], I["V"][g]; sig = np.sqrt(lam); lamsum = lam.sum()
    for prof in ("Qi", "Qmix"):
        Qe = I[prof][g]; gam = lam * Qe
        for var in ("before", "after"):
            Qd = I["Qi"][g] if var == "before" else I["Qmix"][g]
            se, dep_at, sdep, expc = esc_steps(lam, Qd, 2 * 8 * 32); s2 = np.where(sdep == 2)[0]
            hs = {b: [] for b in LV}
            for c in ks[:: max(1, len(ks) // 6)]:
                x = unit(Xs[cstart[c]:cstart[c + 1]]); z = (x - Mq[c]) @ V; m_ = mn[c]; ah = ahat[cstart[c]:cstart[c + 1]] / 32767
                s_it = np.sqrt(np.maximum(1 + m_ * m_ - 2 * m_ * ah, 1e-12) / lamsum)[:, None] if var == "after" else 1
                u = z / sig / s_it
                Cst = np.ones((len(x), len(se)), np.int32); Cst[:, s2] = np.floor(np.abs(u) / W).astype(np.int32)[:, se[s2]] + 1; cum = np.cumsum(Cst, 1)
                for b in LV:
                    de = dep_at[int((expc <= 8 * b).sum())]; D = ((lam * gE[de] + 1e-10) / (Qd + 1e-10)) ** 0.25
                    dep = dep_at[(cum <= 8 * b).sum(1)]; code = recon_esc(u, dep) * sig * s_it
                    rho = roundup16(np.linalg.norm((z - code) / D, axis=1))
                    hs[b].append(np.sqrt((D * D * Qe).sum()) * rho)
            for i, b in enumerate(LV):
                nlev = (1 + [4, 8, 16, 32].index(b) + 1) if var == "before" and b in (4, 8, 16, 32) else (i + 1)
                if var == "before" and b not in (4, 8, 16, 32): continue
                B = 8 * b + 16 + (8 * nlev if var == "before" else 4 * nlev)
                pf, cf = floors(gam, B); h = float(np.concatenate(hs[b]).mean())
                a = acc.setdefault((prof, var, b), [0, 0, 0, 0]); a[0] += h; a[1] += pf; a[2] += cf; a[3] += 1
for prof in ("Qi", "Qmix"):
    print(f"\n{NAME}, offsets with profile {prof}: level B | stored bits | mean CS half-width | /conjectured floor | /proved floor")
    for var in ("before", "after"):
        for b in LV:
            if (prof, var, b) not in acc: continue
            h, pf, cf, k = acc[(prof, var, b)]
            print(f"  {var:6s} {b:2d} B | h {h / k:.4f} | conj {cf / k:.4f} ratio {h / cf:.3f} | proved {pf / k:.4f} ratio {h / pf:.3f}")
