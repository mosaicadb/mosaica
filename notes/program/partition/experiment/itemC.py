# Item 5: rows to the cell where their interval is narrowest (P3.5). usage:
#   itemC.py BASELINE B16PART first          the first-step numbers for C16, C0, Cmix, C16-in (no refit)
#   itemC.py BASELINE B16PART run RULE       iterate RULE (C16, C0, Cmix, C16-in) to its stopping point, then score
# Candidates per row: the 4 baseline centres of largest inner product (fixed), plus the row's starting cell; for C16-in
# the 4 of largest inner product among its own group's cells, plus its starting cell. Sigma_Q = Si for the scheme.
import sys
from lib import *

base, bpart, mode = sys.argv[1], sys.argv[2], sys.argv[3]
X = load_X()
SCT = scores_T(X)
cell0, grp = load_part(bpart)
K = int(cell0.max()) + 1
G = int(grp.max()) + 1
_, gbase = load_part(base)
assert (load_part(base)[0] == cell0).all()
M0, _ = means(X, cell0, K)
Mh0 = unit(M0).astype(np.float32)
A = A_SI
CD = f"{WORK}/C"
os.makedirs(CD, exist_ok=True)


def candidates(ingroup):
    p = f"{CD}/cand_{'in' if ingroup else 'all'}.npy"
    if os.path.exists(p):
        return np.load(p)
    cand = np.zeros((N, 5), np.int32)
    for s in range(0, N, 20_000):
        P = X[s:s + 20_000] @ Mh0.T
        if ingroup:
            P[grp[None, :] != grp[cell0[s:s + 20_000]][:, None]] = -np.inf
        cand[s:s + 20_000, :4] = np.argpartition(-P, 4, axis=1)[:, :4]
    cand[:, 4] = cell0
    np.save(p, cand)
    return cand


def forms(cell, alive, rule, norm=None):
    """Per group the quadratic form of the rule's cost, and W16 * P16 for reporting, from fit rows."""
    M, cnt = means_alive(cell, alive)
    Cc, c2 = scatters(X, cell, K, M, FIT)
    P = np.zeros((G, n, n), np.float32)
    P16 = np.zeros((G, n, n), np.float32)
    for g in range(G):
        m = (grp == g) & alive
        Ng = c2[m].sum()
        gam, T = basis(Cc[m].sum(0) / Ng, A)
        s, W, _ = level(gam, 16)
        P16[g] = W * ((T * s) @ T.T)
        if rule in ("C16", "C16-in"):
            P[g] = P16[g]
        elif rule == "C0":
            P[g] = np.sqrt(gam).sum() * (T @ T.T)
        elif rule == "Cmix":
            acc = np.zeros((n, n))
            for B in (4, 8, 16, 32):
                s, W, _ = level(gam, B)
                acc += W * ((T * s) @ T.T) / norm[B]
            P[g] = acc
    return M.astype(np.float32), P, P16


def means_alive(cell, alive):
    S, cnt = cell_sums(X, cell, K, FIT)
    M = np.where(cnt[:, None] > 0, S / np.maximum(cnt, 1)[:, None], 0.0)
    return M, cnt


def pair_costs(rows, cand, alive, M, P):
    """Cost of each row at each of its candidate cells (inf where dead or duplicate)."""
    out = np.full((len(rows), cand.shape[1]), np.inf)
    cr = cand[rows]
    for j in range(cand.shape[1]):
        c = cr[:, j]
        ok = alive[c]
        if j < cand.shape[1] - 1:
            ok &= ~(cr[:, j + 1:] == c[:, None]).any(1)
        for g in range(G):
            sel = np.flatnonzero(ok & (grp[c] == g))
            for i in range(0, len(sel), 20_000):
                ss = sel[i:i + 20_000]
                D = X[rows[ss]] - M[c[ss]]
                out[ss, j] = ((D @ P[g]) * D).sum(1, dtype=np.float64)
    return out


def assign(rows, cell, cand, alive, M, P):
    """Each row to its cheapest candidate, keeping its cell on a tie."""
    co = pair_costs(rows, cand, alive, M, P)
    cr = cand[rows]
    j = co.argmin(1)
    best = cr[np.arange(len(rows)), j]
    bc = co[np.arange(len(rows)), j]
    curc = np.where(cr == cell[rows][:, None], co, np.inf).min(1)
    keep = curc <= bc * (1 + 1e-9)
    new = np.where(keep, cell[rows], best).astype(np.int32)
    return new, np.where(keep, curc, bc)


def mix_norm():
    """Mean over fit rows of w_B^2 at the baseline (its cells and groups), B = 4, 8, 16, 32."""
    Cc, c2 = scatters(X, cell0, K, M0, FIT)
    out = {}
    for B in (4, 8, 16, 32):
        tot = 0.0
        for g in range(int(gbase.max()) + 1):
            m = gbase == g
            gam, _ = basis(Cc[m].sum(0) / c2[m].sum(), A)
            _, W, _ = level(gam, B)
            tot += c2[m].sum() * W * W
        out[B] = tot / c2.sum()
    return out


fit_rows = np.flatnonzero(FIT)
held_rows = np.flatnonzero(HELD)
alive0 = np.ones(K, bool)

if mode == "first":
    norm = mix_norm()
    log("Cmix normalisers", norm)
    res = {"mix_norm": norm}
    M, _, P16 = forms(cell0, alive0, "C16")
    w16_before = np.sqrt(pair_costs(held_rows, cell0[:, None], alive0, M, P16)[:, 0])
    res["w16_held_before"] = float(w16_before.mean())
    best = {}
    for rule in ("C16", "C0", "Cmix", "C16-in"):
        cand = candidates(rule == "C16-in")
        M, P, P16 = forms(cell0, alive0, rule, norm)
        nf, _ = assign(fit_rows, cell0, cand, alive0, M, P)
        nh, _ = assign(held_rows, cell0, cand, alive0, M, P)
        tmp = cell0.copy()
        tmp[held_rows] = nh
        w16_after = np.sqrt(pair_costs(held_rows, tmp[:, None], alive0, M, P16)[:, 0])
        # the cheapest candidate with no tie rule, for the C0/C16 comparison
        co = pair_costs(fit_rows, cand, alive0, M, P)
        best[rule] = cand[fit_rows, co.argmin(1)]
        res[rule] = {"fit_change_cell": float((nf != cell0[fit_rows]).mean()),
                     "fit_change_group": float((grp[nf] != grp[cell0[fit_rows]]).mean()),
                     "held_change_cell": float((nh != cell0[held_rows]).mean()),
                     "w16_held_after": float(w16_after.mean())}
        log(rule, res[rule])
    res["cheapest_differs_C0_C16"] = float((best["C0"] != best["C16"]).mean())
    log("cheapest differs C0 vs C16", res["cheapest_differs_C0_C16"])
    json.dump(res, open(f"{WORK}/itemC_first.json", "w"), indent=1)

elif mode == "run":
    rule = sys.argv[4]
    cand = candidates(rule == "C16-in")
    norm = json.load(open(f"{WORK}/itemC_first.json"))["mix_norm"]
    norm = {int(k): v for k, v in norm.items()}
    cell = cell0.copy()
    alive = alive0.copy()
    hist = []
    hp = f"{CD}/{rule}_hist.json"
    start = 1
    for p in range(15, 0, -1):                             # resume from the last saved pass
        if os.path.exists(f"{CD}/{rule}_pass{p}.npz"):
            d = np.load(f"{CD}/{rule}_pass{p}.npz")
            cell, alive = d["cell"], d["alive"]
            hist = json.load(open(hp))
            start = p + 1
            break
    M, P, P16 = forms(cell, alive, rule, norm)
    if start == 1:
        o16 = pair_costs(fit_rows, cell[:, None], alive, M, P16)[:, 0].mean()
        orule = pair_costs(fit_rows, cell[:, None], alive, M, P)[:, 0].mean()
        h16 = pair_costs(held_rows, cell[:, None], alive, M, P16)[:, 0].mean()
        hist.append({"pass": 0, "moved_fit": 0.0, "fit_w16sq": float(o16), "fit_rule": float(orule), "held_w16sq_baseline_cells": float(h16)})
    done = start > 1 and hist[-1]["moved_fit"] < 0.001
    for p in range(start, 16):
        if done:
            break
        nf, _ = assign(fit_rows, cell, cand, alive, M, P)
        old = cell[fit_rows]
        # the floor: a group may not fall below 3n fit rows; moves out of a group that would are refused this pass
        gnew = np.bincount(grp[nf], minlength=G)
        refused = 0
        for g in np.flatnonzero(gnew < FLOOR):
            out = (grp[old] == g) & (grp[nf] != g)
            refused += int(out.sum())
            nf[out] = old[out]
        moved = float((nf != old).mean())
        cell = cell.copy()
        cell[fit_rows] = nf
        # cells left with under 32 fit rows are dropped and their rows reassigned
        dropped = 0
        while True:
            cf = np.bincount(cell[fit_rows], minlength=K)
            small = np.flatnonzero(alive & (cf < 32))
            if len(small) == 0:
                break
            alive[small] = False
            dropped += len(small)
            r = fit_rows[~alive[cell[fit_rows]]]
            nr, _ = assign(r, cell, cand, alive, M, P)
            nr = np.where(alive[nr], nr, -1)
            if (nr < 0).any():                                # every candidate dead: nearest live centre
                bad = r[nr < 0]
                Mh = unit(np.where(alive[:, None], M, 1.0)).astype(np.float32)
                sc = X[bad] @ Mh.T
                sc[:, ~alive] = -np.inf
                nr[nr < 0] = sc.argmax(1)
            cell[r] = nr
        # held-out rows of dead cells keep a place until the end: their cell's centre is gone, so use their candidates
        M, P, P16 = forms(cell, alive, rule, norm)
        o16 = pair_costs(fit_rows, cell[:, None], alive, M, P16)[:, 0].mean()
        orule = pair_costs(fit_rows, cell[:, None], alive, M, P)[:, 0].mean()
        hcell = cell.copy()
        hcell[held_rows] = cell0[held_rows]
        hcell_alive = np.where(alive[hcell[held_rows]], hcell[held_rows], cand[held_rows, 0])
        hcell[held_rows] = hcell_alive
        nh, _ = assign(held_rows, hcell, cand, alive, M, P)
        hc = cell.copy()
        hc[held_rows] = nh
        h16 = pair_costs(held_rows, hc[:, None], alive, M, P16)[:, 0].mean()
        hrule = pair_costs(held_rows, hc[:, None], alive, M, P)[:, 0].mean()
        hist.append({"pass": p, "moved_fit": moved, "refused_floor": refused, "cells_dropped": dropped,
                     "cells_alive": int(alive.sum()), "fit_w16sq": float(o16), "fit_rule": float(orule),
                     "held_w16sq": float(h16), "held_rule": float(hrule),
                     "fit_change_cell_from_start": float((cell[fit_rows] != cell0[fit_rows]).mean()),
                     "fit_change_group_from_start": float((grp[cell[fit_rows]] != grp[cell0[fit_rows]]).mean())})
        log(rule, "pass", p, hist[-1])
        np.savez(f"{CD}/{rule}_pass{p}.npz", cell=cell, alive=alive)
        json.dump(hist, open(hp, "w"), indent=1)
        if moved < 0.001:
            break
    # held-out rows assigned once at the end by the same rule
    hcell = cell.copy()
    hcell[held_rows] = np.where(alive[cell0[held_rows]], cell0[held_rows], cand[held_rows, 0])
    nh, _ = assign(held_rows, hcell, cand, alive, M, P)
    cell[held_rows] = nh
    # the item queries' own rows (not fit, not held) by the same rule
    rest = np.flatnonzero(~VALID)
    hcell[rest] = np.where(alive[cell0[rest]], cell0[rest], cand[rest, 0])
    nr, _ = assign(rest, hcell, cand, alive, M, P)
    cell[rest] = nr
    assert alive[cell].all()
    cfin, gfin = compact(cell, grp)
    score_partition(rule, X, SCT, cfin, gfin, extra={"C_hist": hist})
log("done")
