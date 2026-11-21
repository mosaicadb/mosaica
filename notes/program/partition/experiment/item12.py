# Items 1 and 2. usage: item12.py seeds | lloyd
#   seeds: A0 (setup.npz) and A1, A2, A3 (the build's steps, seeds 1, 2, 3), S0 for each, every score.
#   lloyd: Lloyd continued on all fit rows from A1; scores after 1, 2, 5, 10, 20 passes and at the fixed point.
import sys
from lib import *

X = load_X()
SCT = scores_T(X)


def S0(cell, M):
    """Share of rows whose largest key over the stored centre directions is not at their own cell."""
    best = argmax_ip(X, unit(M))
    off = best != cell
    return {"S0_all": float(off[VALID].mean()), "S0_fit": float(off[FIT].mean()), "S0_held": float(off[HELD].mean())}


if sys.argv[1] == "seeds":
    cell = s0["a"].astype(np.int32)
    grp = s0["grp"].astype(np.int32)
    M, _ = means(X, cell, 200)
    # A0's S0 against its own stored means (all rows, as in setup.npz) and against fit-row means
    Mst = s0["Mean"]
    ex = S0(cell, M)
    ex.update({"S0_all_setupMean": float((argmax_ip(X, unit(Mst)) != cell)[VALID].mean())})
    log("A0", ex)
    score_partition("A0", X, SCT, cell, grp, M=M, extra=ex)
    for seed in (1, 2, 3):
        cell, grp, M, C = build_A(X, seed)
        ex = S0(cell, M)
        log(f"A{seed}", "K", int(cell.max()) + 1, "G", int(grp.max()) + 1, ex)
        np.save(f"{WORK}/parts/A{seed}_centroids.npy", C)
        score_partition(f"A{seed}", X, SCT, cell, grp, M=M, extra=ex)

elif sys.argv[1] == "lloyd":
    cell, grp = load_part("A1")
    K = int(cell.max()) + 1
    M, _ = means(X, cell, K)
    REPORT = {1, 2, 5, 10, 20}
    hist = []
    fitidx = np.flatnonzero(FIT)
    for p in range(1, 301):
        new = argmax_ip(X, unit(M))                       # every row to the stored centre of largest inner product
        nfit = np.bincount(new[FIT], minlength=K)
        if (nfit == 0).any():                              # a centre that takes no fit row has no mean: dropped
            log("pass", p, "dropping", int((nfit == 0).sum()), "centres with no fit rows")
            keep = np.flatnonzero(nfit > 0)
            remap = -np.ones(K, np.int64)
            remap[keep] = np.arange(len(keep))
            cell = remap[cell]
            M = M[keep]
            K = len(keep)
            new = argmax_ip(X, unit(M))
        moved = float((new[FIT] != cell[FIT]).mean())
        moved_all = float((new[VALID] != cell[VALID]).mean())
        cell = new
        M, _ = means(X, cell, K)
        mk = float((X[fitidx[::7]].astype(np.float64) * unit(M)[cell[fitidx[::7]]]).sum(1).mean())
        hist.append({"pass": p, "moved_fit": moved, "moved_all": moved_all, "K": K, "mean_key_fit_sample": mk})
        log("pass", p, hist[-1])
        last = moved < 0.001
        if p in REPORT or last:
            grp = build_groups(M, 12, np.random.default_rng(1))
            score_partition(f"A1-lloyd{p}" if not last else "A1-fp", X, SCT, cell, grp, M=M,
                            extra={"pass": p, "moved_fit": moved, **{k: v for k, v in S0(cell, M).items()}})
        if last:
            break
    json.dump(hist, open(f"{WORK}/lloyd_hist.json", "w"), indent=1)
log("done")
