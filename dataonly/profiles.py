# Data-only query-offset profiles for one index, from corpus items held out from the test queries (no text anywhere).
#   Qi4  : as the build's Qi (each pseudo-query against its 4 nearest centres), own sample, to check sampling noise
#   Qi50 : each pseudo-query against its 50 nearest centres (the neighbourhood size the text profile used)
# Also prints the modelled Cauchy-Schwarz width ratio of each candidate profile against item and (where present) text truth.
# usage: python3 profiles.py NAME
import numpy as np, sys, os
VEC = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
NAME = sys.argv[1]
IDX = VEC + (f"combined/idx_{NAME}/" if os.path.exists(VEC + f"combined/idx_{NAME}/index.npz") else f"quant/idx_{NAME}/")
I = dict(np.load(IDX + "index.npz")); Xs = np.load(IDX + "Xs.npy", mmap_mode="r")
N, n, G = int(I["N"]), int(I["n"]), int(I["G"]); Mq, grp, V, lam = I["Mq"], I["grp"], I["V"], I["lam"]
rng = np.random.default_rng(777)
pool = np.setdiff1d(np.arange(N), I["qi"]); pi = np.sort(rng.choice(pool, 5000, replace=False))
pq = np.asarray(Xs[pi], np.float32); pq /= np.linalg.norm(pq, axis=1, keepdims=True)
order = np.argsort(-(pq @ Mq.T), axis=1)
out = {}
for nm, k, mn_rows in (("Qi4", 4, 50), ("Qi50", 50, 200)):
    pk = order[:, :k]; Pg = np.zeros((G, n))
    for g in range(G):
        rows, cols = np.where(grp[pk] == g)
        Pg[g] = (((pq[rows] - Mq[pk[rows, cols]]) @ V[g]) ** 2).mean(0) if len(rows) > mn_rows else lam[g]
    out[nm] = Pg
np.savez(VEC + f"dataonly/prof_{NAME}.npz", **out)
def cost(T, P, e):
    w = np.sqrt(np.sqrt(e / P)); return (np.sqrt((T * w ** 2).sum(1)) * np.sqrt((e / w ** 2).sum(1)) / np.sqrt(T * e).sum(1)).mean()
Qi = I["Qi"]; tr = Qi.sum(1, keepdims=True); iso = np.ones_like(Qi) / n * tr
def blend(a, b, w): return ((1 - w) * np.sqrt(a) + w * np.sqrt(b)) ** 2
cands = [("Qi (build)", Qi), ("Qi4 (resample)", out["Qi4"]), ("Qi50", out["Qi50"]), ("iso", iso)] + \
        [(f"blend Qi+iso w={w}", blend(Qi, iso, w)) for w in (0.25, 0.5, 0.75)] + [("Qmix (text)", I["Qmix"])]
has_text = not np.allclose(I["Qmix"], Qi)
print(NAME, "traces: lam %.3f Qi %.3f Qi50 %.3f" % (lam.sum(1).mean(), Qi.sum(1).mean(), out["Qi50"].sum(1).mean()), ("Qt %.3f" % I["Qt"].sum(1).mean()) if has_text else "")
for nm, P in cands:
    if nm.startswith("Qmix") and not has_text: continue
    print(f"  {nm:22s} modelled CS width / optimum: item {cost(Qi, P, lam):.4f}" + (f"  text {cost(I['Qt'], P, lam):.4f}" if has_text else ""))
