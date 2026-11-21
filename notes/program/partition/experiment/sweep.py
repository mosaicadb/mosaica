# Item 7: the sweep for P2. One group, so that only K moves; the build's steps, seed 1, continued to a fixed point.
# usage: sweep.py full K   (pool = all fit rows)   |   sweep.py sub K   (pool = a random 37,500 of the fit rows, seed 7)
#        sweep.py seeds    (W_P spectra of A1, A2, A3, for the spread)
# The centres, the fixed point and W_P use the pool's rows only; coding is scored on the held-out rows; S2+3 and S4a
# on every row placed at its nearest stored centre.
import sys
from lib import *

X = load_X()
os.makedirs(WORK + "/sweep", exist_ok=True)
A = A_SI


def spectra(cell, K, M, mask):
    Cc, cnt = scatters(X, cell, K, M, mask)
    WP = Cc.sum(0) / cnt.sum()
    gW, _ = basis(WP, A)
    own = np.zeros(n)
    for c in range(K):
        g = np.linalg.eigvalsh(A @ (Cc[c] / cnt[c]) @ A)[::-1]
        own += np.maximum(g, 0) * cnt[c]
    own /= cnt.sum()                       # row-weighted mean over cells of each cell's own sorted spectrum
    ownu = np.zeros(n)
    for c in range(K):
        ownu += np.maximum(np.linalg.eigvalsh(A @ (Cc[c] / cnt[c]) @ A)[::-1], 0)
    ownu /= K
    return WP, gW, own, ownu, cnt


if sys.argv[1] == "seeds":
    out = {}
    for s in (1, 2, 3):
        cell, _ = load_part(f"A{s}")
        K = int(cell.max()) + 1
        M, _ = means(X, cell, K)
        _, gW, own, ownu, _ = spectra(cell, K, M, FIT)
        out[f"A{s}"] = {"gamma": gW.tolist(), "own": own.tolist()}
        log("seed", s, "gamma1", gW[0])
    json.dump(out, open(f"{WORK}/sweep/seed_spectra.json", "w"))
    sys.exit()

which, K = sys.argv[1], int(sys.argv[2])
name = f"sweep-{which}-K{K}"
pool = np.flatnonzero(FIT)
if which == "sub":
    pool = np.sort(np.random.default_rng(7).choice(pool, 37_500, replace=False))
pm = np.zeros(N, bool)
pm[pool] = True
cell, _, M, C = build_A(X, 1, K=K, G=1, pool=pool)
# build_A takes means over all fit rows; over the pool, a cell with no pool row is dropped and its rows go to the
# nearest remaining stored centre
K = int(cell.max()) + 1
npool = np.bincount(cell[pool], minlength=K)
if (npool == 0).any():
    keep = np.flatnonzero(npool > 0)
    cell = argmax_ip(X, unit(M[keep]))
    cell, _ = compact(cell)
    K = int(cell.max()) + 1
M, _ = means(X, cell, K, pm)
hist = []
for p in range(1, 301):
    new = argmax_ip(X, unit(M))
    np_ = np.bincount(new[pool], minlength=K)
    if (np_ == 0).any():
        keep = np.flatnonzero(np_ > 0)
        remap = -np.ones(K, np.int64)
        remap[keep] = np.arange(len(keep))
        cell = remap[cell]
        M = M[keep]
        K = len(keep)
        new = argmax_ip(X, unit(M))
    moved = float((new[pool] != cell[pool]).mean())
    cell = new
    M, _ = means(X, cell, K, pm)
    hist.append(moved)
    if moved < 0.001:
        break
log(name, "fixed point after", len(hist), "passes; K", K, "last moves", [round(h, 4) for h in hist[-3:]])
WP, gW, own, ownu, cnt = spectra(cell, K, M, pm)
out = {"name": name, "K": K, "pool_rows": len(pool), "passes": len(hist), "moved_hist": hist,
       "msq_resid_pool": float(np.trace(WP)), "gamma": gW.tolist(), "own_mean_spectrum": own.tolist(),
       "own_mean_spectrum_unweighted": ownu.tolist(), "pool_rows_per_cell_min": int(cnt.min()),
       "pool_rows_per_cell_median": float(np.median(cnt))}
covs = [WP]
for key, Ak in AS.items():
    w0, wB, by = coding(X, cell, M, np.zeros(K, np.int64), covs, Ak)
    for k, v in coding_summary(w0, wB, by).items():
        out[f"{key}_{k}"] = v
log(name, {k: round(v, 4) for k, v in out.items() if k.startswith("Si_")})
SCT = scores_T(X)
out.update({f"st_{k}": v for k, v in structure(X, SCT, cell, M).items()})
log(name, "violations", out["st_viol_S2"], out["st_viol_S3"])
json.dump(out, open(f"{WORK}/sweep/{name}.json", "w"))
log("done")
