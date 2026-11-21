# Item 4: cells to bases by coding cost (P3.1, P3.2). usage: itemB.py BASELINE
# B0 (zero read) and B16 (16 B level), each in-sample (the theorem's scheme) and cross-fitted; objectives per pass on
# fit and held-out rows; then one pass of single-cell moves with both groups refitted, from each in-sample result.
# Sigma_Q is Si throughout the scheme; the scores are then taken for Si and Sm.
import sys
from lib import *

base = sys.argv[1]
X = load_X()
SCT = scores_T(X)
cell, grp0 = load_part(base)
K = int(cell.max()) + 1
G = int(grp0.max()) + 1
M, _ = means(X, cell, K)
# halves of each cell's fit rows, by parity of rank (in permutation position) within the cell
fitidx = np.flatnonzero(FIT)
o = np.lexsort((POS[fitidx], cell[fitidx]))
fs = fitidx[o]
rank = np.arange(len(fs)) - np.searchsorted(cell[fs], cell[fs])
HA = np.zeros(N, bool); HA[fs[rank % 2 == 0]] = True
HB = np.zeros(N, bool); HB[fs[rank % 2 == 1]] = True
C_all, n_all = scatters(X, cell, K, M, FIT)
C_A, n_A = scatters(X, cell, K, M, HA)
C_B, n_B = scatters(X, cell, K, M, HB)
C_h, n_h = scatters(X, cell, K, M, HELD)
log("scatters done; fit/half A/half B/held rows", n_all.sum(), n_A.sum(), n_B.sum(), n_h.sum())
flat = lambda C: C.reshape(len(C), -1)
A = A_SI


def metrics(Cs, ns, grp, B):
    """Per group: W_g * T_g diag(s) T_g^T (the level's quadratic form) and W_g, from pooled covariances."""
    Pw = np.zeros((G, n, n))
    Ws = np.zeros(G)
    Ng = np.zeros(G)
    for g in range(G):
        m = grp == g
        Ng[g] = ns[m].sum()
        gam, T = basis(Cs[m].sum(0) / Ng[g], A)
        s, W, _ = level(gam, B)
        Pw[g] = W * ((T * s) @ T.T)
        Ws[g] = W
    return Pw, Ws, Ng


def costs(C, Pw):
    return flat(C) @ flat(Pw).T                       # K x G: sum over a cell's rows of w_B(x | c, g)^2


def angle(grp):
    Mh = unit(M)
    ang = []
    for c in range(K):
        gd = Mh[grp == grp[c]].mean(0)
        gd /= np.linalg.norm(gd)
        ang.append(np.degrees(np.arccos(np.clip(Mh[c] @ gd, -1, 1))))
    return float(np.mean(ang))


def run(B, cross):
    grp = grp0.copy()
    rec = []
    Cfit, nfit = (C_A, n_A) if cross else (C_all, n_all)
    Ccost = C_B if cross else C_all
    for p in range(21):
        Pw, Ws, Ng = metrics(Cfit, nfit, grp, B)
        ca = costs(C_all, Pw)
        ch = costs(C_h, Pw)
        o_fit = ca[np.arange(K), grp].sum() / n_all.sum()
        o_held = ch[np.arange(K), grp].sum() / n_h.sum()
        cc = ca if not cross else costs(Ccost, Pw)
        best = cc.argmin(1)
        cur = cc[np.arange(K), grp]
        want = np.flatnonzero((cc[np.arange(K), best] < cur * (1 - 1e-6)) & (best != grp))
        gsize = np.array([n_all[grp == g].sum() for g in range(G)])
        moved = refused = 0
        if p < 20:
            for c in want:
                g0, g1 = grp[c], best[c]
                if gsize[g0] - n_all[c] < FLOOR:
                    refused += 1
                    continue
                grp[c] = g1
                gsize[g0] -= n_all[c]
                gsize[g1] += n_all[c]
                moved += 1
        rec.append({"pass": p, "obj_fit_msq": float(o_fit), "obj_held_msq": float(o_held),
                    "insample_value_sumNgWg2_over_N": float((Ng * Ws * Ws).sum() / Ng.sum()),
                    "moved": moved, "refused_floor": refused, "groups_nonempty": int(len(np.unique(grp)))})
        log(f"B{B} {'cross' if cross else 'in'} pass {p}", rec[-1])
        if moved == 0:
            break
    return grp, rec


def single_moves(grp, B):
    grp = grp.copy()
    order = np.random.default_rng(1).permutation(K)
    pooled = [C_all[grp == g].sum(0) for g in range(G)]
    Ng = np.array([n_all[grp == g].sum() for g in range(G)])
    val = np.zeros(G)
    Pw = np.zeros((G, n, n))

    def fitg(Cg, ng):
        gam, T = basis(Cg / ng, A)
        s, W, _ = level(gam, B)
        return W, W * ((T * s) @ T.T)
    for g in range(G):
        W, Pw[g] = fitg(pooled[g], Ng[g])
        val[g] = Ng[g] * W * W
    start = val.sum() / Ng.sum()
    kept = tried = refused = 0
    for c in order:
        cc = flat(C_all[c][None]) @ flat(Pw).T
        cc = cc[0]
        g0 = grp[c]
        others = [g for g in np.argsort(cc) if g != g0]
        g1 = others[0]
        if Ng[g0] - n_all[c] < FLOOR:
            refused += 1
            continue
        tried += 1
        W0, P0 = fitg(pooled[g0] - C_all[c], Ng[g0] - n_all[c])
        W1, P1 = fitg(pooled[g1] + C_all[c], Ng[g1] + n_all[c])
        nv0 = (Ng[g0] - n_all[c]) * W0 * W0
        nv1 = (Ng[g1] + n_all[c]) * W1 * W1
        if nv0 + nv1 < val[g0] + val[g1]:
            grp[c] = g1
            pooled[g0] = pooled[g0] - C_all[c]
            pooled[g1] = pooled[g1] + C_all[c]
            Ng[g0] -= n_all[c]
            Ng[g1] += n_all[c]
            val[g0], val[g1] = nv0, nv1
            Pw[g0], Pw[g1] = P0, P1
            kept += 1
    end = val.sum() / Ng.sum()
    log(f"B{B} single moves: tried {tried}, kept {kept}, refused {refused}, value {start:.6f} -> {end:.6f}")
    return grp, {"tried": tried, "kept": kept, "refused_floor": refused, "value_before": float(start), "value_after": float(end)}


summary = {"angle_baseline": angle(grp0)}
for B in (0, 16):
    for cross in (False, True):
        nm = f"B{B}-{'cf' if cross else 'in'}"
        grp, rec = run(B, cross)
        summary[nm] = {"passes": rec, "angle": angle(grp), "cells_changed_group": int((grp != grp0).sum()),
                       "fit_rows_changed_group": int(n_all[grp != grp0].sum())}
        save_part(nm, cell, grp)
        if not cross:
            g2, mv = single_moves(grp, B)
            summary[nm + "-moves"] = {**mv, "angle": angle(g2), "cells_changed_group": int((g2 != grp0).sum())}
            save_part(nm + "-moves", cell, g2)
        json.dump(summary, open(f"{WORK}/itemB_summary.json", "w"), indent=1)
del C_A, C_B
for nm in ("B0-in", "B0-cf", "B16-in", "B16-cf", "B0-in-moves", "B16-in-moves"):
    _, g = load_part(nm)
    score_partition(nm, X, SCT, cell, g, M=M, struct=False)
log("done")
