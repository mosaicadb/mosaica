# Replay of the process of policy.md (section 9) against the tuned orders, on the real fit of one dataset.
# Fit, groups, bases and row order are unchanged; sub-clusters are 16 labels per cluster. Exact search inside a
# read unit. Nothing is timed: cost is MODELLED from counts with the constants below, and rows, clusters, groups
# and centres are reported beside it.
#
# Cost constants (us), per dataset, from the real index's phase timings on these fits
# (../../results/phases-1-int8-order.txt): group = the group phase / groups among the first P clusters; item = the
# rest phase at nprobe 16 less b per cluster, per row; b = 4 per cluster on the index path (the planner's 1.6-7.8;
# not separable in the phases); full = the stored-vector phase per row (direct reads: 170-200 us for the first
# cluster unfiltered, 23 us for 320 visible rows under random-1); dir = 0.2 per cluster read directly; centre = the
# order phase / K. A sub-cluster read costs item per visible row of it, b once per expanded cluster, and 16 centres
# to expand (e_c); the label pass is not priced.
#
# Tuned arms (stop: a probe count, the same for every query):
#   T1   spread, f' = 0.4
#   T2   H r16 over the first 4 nprobe clusters of T1 (sparse fallback as q.rs)
#   T3   T1 with the group penalty: key - 0.25 sd(q) for a cluster whose group is not yet opened, greedy
#   T4   16 sub-clusters, two-level: the sub-clusters of the first 64 clusters of T1, spread 0.4
#   T5   16 sub-clusters, group-limited: the sub-clusters of the groups of the first 32 clusters of T1
#   O0, O1  the process's ORDER alone (clusters by P at tau*, tiers S0 / S1), stopped by a probe count
#   Each probed cluster takes the path the planner's rule gives it (direct when it holds at most 64 + 0.186 S
#   visible rows, S its rows; else the index path, opening its group).
# Process arms (stop: a global price lambda, swept), values n_vis P(tau) from calib.py's tables:
#   P0, P1, P2 clusters only, spread tier S0 / S1 / S2 (S2 for every cluster, not lazily); plan (1.1) with the
#              direct alternative
#   P0i, P1i   the same with each cluster's path fixed by the planner's rule (as the tuned arms: isolates order and
#              stopping from the choice of path)
#   P0s, P1s   with sub-clusters, expansion E1 (a cluster on the index path is expanded when it holds more visible
#              rows than its sub-centres cost in rows; planning value its whole value)
#   P1 ag0     P1 planned with a_g = 0 (the group cost is still charged)
#   P1 tau     P1 with the true k-th best score as tau (ceiling for the tau estimate); arms ending in `*` (run
#              with a fourth argument): tau* solved once before anything is read and never updated (one plan)
#   clair      the plan with the true answer counts per cluster as values (clairvoyant ceiling), paths as P1
# k-NN: tau from sum n P(tau) = k, re-solved at epochs (rows read doubled) with the found items counted exactly.
# Thresholds: tau = t, one plan.
# usage: python3 -I policy.py DS OUT.json [k]
import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *
import numpy as np
from calib import Fit, tables

COST = dict(laion=dict(group=27.0, b=4.0, item=0.057, dir=0.2, full=0.07, centre=0.07),
            yahoo=dict(group=14.5, b=4.0, item=0.04, dir=0.2, full=0.045, centre=0.067),
            glove=dict(group=4.0, b=4.0, item=0.027, dir=0.2, full=0.03, centre=0.045))
LAMS = np.logspace(0.5, -5.5, 19)
M_TOP = 4000
D = DS(sys.argv[1])
K_NN = int(sys.argv[3]) if len(sys.argv) > 3 else 10
C = COST[D.ds]
F = Fit(D)
TAB, _ = tables(D, F)
K, L, G = D.K, F.L, F.G
grpK, par = D.grp, F.par
mu, sdq, sp = F.feats(D.Q)
radn = F.rad["cluster"] / sdn(F.rad["cluster"])
radn_s = F.rad["sub"] / sdn(F.rad["sub"])
sig16 = sigma(D.Q, F.cs, 16)
EC = 16 * C["centre"]
out = {}


def interp_at(rec, vals, t):
    """vals at the first crossing of recall t along a curve ordered by increasing effort."""
    rec = np.asarray(rec)
    hit = np.where(rec >= t)[0]
    if not len(hit):
        return None
    i = int(hit[0])
    if i == 0:
        return {k: float(v[0]) for k, v in vals.items()}
    f = (t - rec[i - 1]) / max(rec[i] - rec[i - 1], 1e-12)
    return {k: float(v[i - 1] + f * (v[i] - v[i - 1])) for k, v in vals.items()}


class Task:
    def __init__(self, name, vis=None, thr_col=None):
        self.name, self.thr = name, thr_col
        w = np.ones(D.N) if vis is None else vis.astype(np.float64)
        self.nvK = np.bincount(D.a, weights=w, minlength=K)
        self.nvS = np.bincount(F.sub, weights=w, minlength=L)
        ids = np.arange(D.N) if vis is None else np.where(vis)[0]
        self.nvis = len(ids)
        if thr_col is None:
            sets = D.truth_knn(vis) if K_NN == 10 else [ids[np.argsort(-D.S[i, ids], kind="stable")[:K_NN]] for i in range(D.nq)]
            self.tau_true = np.array([D.S[i, s].min() for i, s in enumerate(sets)], np.float64)
        else:
            sets = D.truth_thr(thr_col, vis)
            self.tau_true = D.thr[:, thr_col].astype(np.float64)
        self.tot = np.array([len(s) for s in sets], np.float64)
        self.cntK, self.cntS = counts(D.a, K, sets), counts(F.sub, L, sets)
        m = min(M_TOP, len(ids) - 1)
        self.top_sc = np.zeros((D.nq, m)); self.top_cl = np.zeros((D.nq, m), np.int64); self.top_sub = np.zeros((D.nq, m), np.int64)
        for i in range(D.nq):
            s = D.S[i, ids]
            t = np.argpartition(-s, m)[:m]
            t = t[np.argsort(-s[t])]
            self.top_sc[i], self.top_cl[i], self.top_sub[i] = s[t], D.a[ids[t]], F.sub[ids[t]]
        self.direct_rule = self.nvK <= 64 + 0.186 * F.n["cluster"]  # the planner's rule (no visible set: never)
        if vis is None:
            self.direct_rule[:] = False


def tuned_curve(T, order, nprobes=None, sets=None):
    """Cost and counts of probing whole clusters in `order` (nq x K), each on the planner's path. With `sets`
    (list over nprobe of nq x K boolean masks) the probed set per nprobe is given instead of a prefix."""
    live = T.nvK > 0
    res = dict(rec=[], cost=[], rows=[], clusters=[], groups=[], centres=[])
    base = C["centre"] * live.sum()
    cD = C["dir"] + C["full"] * T.nvK
    cI = C["b"] + C["item"] * T.nvK
    cc = np.where(T.direct_rule, cD, cI)
    onehot = np.zeros((K, G)); onehot[np.arange(K), grpK] = 1
    onehot_idx = onehot * (~T.direct_rule)[:, None]
    if sets is None:
        rank = np.empty_like(order); rank[np.arange(D.nq)[:, None], order] = np.arange(K)[None]
        nprobes = pgrid(int(live.sum()))
        sets = [(rank < p) & live[None] for p in nprobes]
    for m in sets:
        g = ((m @ onehot_idx) > 0).sum(1)
        res["rec"].append(float(((T.cntK * m).sum(1) / np.maximum(T.tot, 1)).mean()))
        res["cost"].append(float((base + (m * cc[None]).sum(1) + C["group"] * g).mean()))
        res["rows"].append(float((m * T.nvK[None]).sum(1).mean()))
        res["clusters"].append(float(m.sum(1).mean()))
        res["groups"].append(float(g.mean()))
        res["centres"].append(float(live.sum()))
    return res


def pgrid(n):
    g, p = [], 1.0
    while p <= n:
        if not g or int(p) > g[-1]:
            g.append(int(p))
        p *= 1.15
    return g


def tuned_sub_curve(T, mask, key):
    """Sub-clusters within `mask` (nq x L) probed by `key`; a sub-cluster of a cluster on the direct path is read
    directly (full per row, dir once per cluster), else on the index path (item per row, b once per cluster, its
    group once)."""
    live = (T.nvS > 0)[None] & mask
    k = np.where(live, key, -np.inf)
    o = np.argsort(-k, 1, kind="stable")
    res = dict(rec=[], cost=[], rows=[], clusters=[], groups=[], centres=[])
    scored = live.sum(1)
    nmax = int(scored.max())
    rowcost = np.where(T.direct_rule[par], C["full"], C["item"]) * T.nvS
    for p in pgrid(nmax):
        rec, cost, rows, cl, gr = [], [], [], [], []
        for i in range(D.nq):
            s = o[i, :min(p, scored[i])]
            pc = np.unique(par[s])
            dr = T.direct_rule[pc]
            g = len(np.unique(grpK[pc[~dr]]))
            rec.append(T.cntS[i, s].sum() / max(T.tot[i], 1))
            cost.append(C["centre"] * ((T.nvK > 0).sum() + scored[i]) + rowcost[s].sum() + C["dir"] * dr.sum() + C["b"] * (~dr).sum() + C["group"] * g)
            rows.append(T.nvS[s].sum()); cl.append(len(pc)); gr.append(g)
        for kk, v in (("rec", rec), ("cost", cost), ("rows", rows), ("clusters", cl), ("groups", gr)):
            res[kk].append(float(np.mean(v)))
        res["centres"].append(float((T.nvK > 0).sum() + scored.mean()))
        if res["rec"][-1] > 0.995:
            break
    return res


def simulate(T, qi, lam, tier="S1", subs=False, direct_alt=True, tau_mode="run", ag_zero=False, clair=False):
    nvK, nvS = T.nvK, T.nvS
    tK, tS = TAB[("cluster", tier)], TAB[("sub", tier)]
    muK, sK, lrK = mu["cluster"][qi], sp[tier]["cluster"][qi], F.lr["cluster"]
    muS, sS, lrS = mu["sub"][qi], sp[tier]["sub"][qi], F.lr["sub"]
    k = T.tot[qi]
    readK = nvK <= 0
    expanded = np.zeros(K, bool)
    readS = np.zeros(L, bool)
    actS = np.zeros(0, np.int64)  # sub-clusters of expanded clusters, not yet read
    openg = np.zeros(G, bool)
    found = np.zeros(T.top_sc.shape[1], bool)
    tsc, tcl, tsub = T.top_sc[qi], T.top_cl[qi], T.top_sub[qi]
    cost = C["centre"] * (nvK > 0).sum()
    rows = clusters = groups = centres = epochs = 0
    cD = C["dir"] + C["full"] * nvK
    cI = C["b"] + C["item"] * nvK
    fixed = tau_mode == "true" or T.thr is not None or clair
    tau = T.tau_true[qi]
    star = tau_mode == "star" and not fixed  # tau* solved once before anything is read, then never updated

    def valsK(t):
        return nvK * tK.P((t - muK) / sK, lrK)

    while True:
        cand = ~readK & ~expanded
        if not fixed:
            lo, hi = -1.0, 1.0
            nf_sc = tsc[found]
            for _ in range(22):
                mid = 0.5 * (lo + hi)
                f = (nf_sc >= mid).sum() + valsK(mid)[cand].sum() - k
                if len(actS):
                    f += (nvS[actS] * tS.P((mid - muS[actS]) / sS[actS], lrS[actS])).sum()
                if f > 0:
                    lo = mid
                else:
                    hi = mid
            tau = 0.5 * (lo + hi)
            fixed = fixed or star
        v = T.cntK[qi].astype(np.float64) if clair else valsK(tau)
        Dn = np.where(cand, v - lam * cD, -np.inf)
        In = np.where(cand, v - lam * cI, -np.inf)
        if not direct_alt:
            Dn = np.where(T.direct_rule, Dn, -np.inf)
            In = np.where(T.direct_rule, -np.inf, In)
        gain = np.bincount(grpK, weights=np.maximum(np.maximum(In, 0) - np.maximum(Dn, 0), 0), minlength=G)
        ag = np.where(openg | ag_zero, 0.0, C["group"])
        openplan = openg | (gain > lam * ag)
        a_idx = (In > np.maximum(Dn, 0)) & openplan[grpK]
        a_dir = ~a_idx & (Dn > 0)
        # densities; clusters of a group to be opened share the block's density
        dens = np.where(a_idx, v / cI, np.where(a_dir, v / cD, -np.inf))
        newg = a_idx & ~openg[grpK]
        if newg.any():
            bv = np.bincount(grpK, weights=np.where(newg, v, 0), minlength=G)
            bc = np.bincount(grpK, weights=np.where(newg, cI, 0), minlength=G) + C["group"]
            dens = np.where(newg, (bv / bc)[grpK], dens)
        acts = [(dens[c], v[c] / cI[c] if a_idx[c] else 0.0, 0, c) for c in np.where(a_idx | a_dir)[0]]
        if len(actS):
            vs = nvS[actS] * tS.P((tau - muS[actS]) / sS[actS], lrS[actS])
            ws = C["item"] * nvS[actS]
            for j in np.where(vs > lam * ws)[0]:
                acts.append((vs[j] / ws[j], 0.0, 1, int(actS[j])))
        if not acts:
            break
        acts.sort(key=lambda x: (-x[0], -x[1]))
        epochs += 1
        rows0 = rows
        for _, _, kind, u in acts:
            if kind == 1:
                cost += C["item"] * nvS[u]; rows += nvS[u]; readS[u] = True
                found |= tsub == u
            elif a_dir[u]:
                cost += cD[u]; rows += nvK[u]; clusters += 1; readK[u] = True
                found |= tcl == u
            else:
                g = grpK[u]
                if not openg[g]:
                    openg[g] = True; groups += 1; cost += C["group"]
                if subs and nvK[u] > EC / C["item"]:
                    expanded[u] = True; clusters += 1; centres += 16
                    cost += C["b"] + EC
                    ss = np.where(par == u)[0]
                    ss = ss[nvS[ss] > 0]
                    vs = nvS[ss] * tS.P((tau - muS[ss]) / sS[ss], lrS[ss])
                    take = vs > lam * C["item"] * nvS[ss]
                    for s in ss[take]:
                        cost += C["item"] * nvS[s]; rows += nvS[s]; readS[s] = True
                        found |= tsub == s
                    actS = np.r_[actS, ss[~take]]
                else:
                    cost += cI[u]; rows += nvK[u]; clusters += 1; readK[u] = True
                    found |= tcl == u
            if not fixed and rows - rows0 >= max(rows0, 1):
                break
        actS = actS[~readS[actS]]
        if fixed:  # one plan: with a fixed tau a second pass could only add sub-clusters judged not worth reading
            break
        if epochs > 80:
            break
    rec = (T.cntK[qi][readK & (nvK > 0)].sum() + T.cntS[qi][readS].sum()) / max(k, 1)
    return rec, cost + C["centre"] * centres, rows, clusters, groups, (nvK > 0).sum() + centres, epochs


def process_curve(T, **kw):
    res = dict(rec=[], cost=[], rows=[], clusters=[], groups=[], centres=[], epochs=[], lam=[])
    for lam in LAMS:
        r = np.array([simulate(T, qi, lam, **kw) for qi in range(D.nq)], np.float64)
        m = r.mean(0)
        for i, kk in enumerate(("rec", "cost", "rows", "clusters", "groups", "centres", "epochs")):
            res[kk].append(float(m[i]))
        res["lam"].append(float(lam))
        if m[0] > 0.995:
            break
    return res


def show(name, res):
    cells = []
    for t in (0.8, 0.9, 0.95):
        e = interp_at(res["rec"], {k: np.array(v) for k, v in res.items() if k != "rec"}, t)
        cells.append("-" if e is None else f"{e['cost']:7.0f} us {e['rows']:7.0f} r {e['clusters']:5.1f} c {e['groups']:4.1f} g {e['centres']:6.0f} ce")
    print(f"{name:10s} " + " | ".join(f"{c:44s}" for c in cells) + (f" | epochs {np.mean(res['epochs']):.1f}, max recall {max(res['rec']):.3f}" if "epochs" in res else ""), flush=True)


def run_task(T, full=True):
    res = out.setdefault(T.name, {})
    print(f"\n== {D.name} | {T.name} | k {K_NN if T.thr is None else 'thr'} | visible {T.nvis} | per arm at recall 0.8 | 0.9 | 0.95: modelled us, rows, clusters, groups, centres")
    live = T.nvK > 0
    s1 = np.where(live[None], mu["cluster"] + 0.4 * sdq * radn[None], -np.inf)
    o1 = np.argsort(-s1, 1, kind="stable")
    res["T1"] = tuned_curve(T, o1); show("T1", res["T1"])
    if T.thr is None:
        dense = T.nvK.sum() >= 8 * K_NN * live.sum()
        if dense:
            h = mu["cluster"] + 0.75 * z_of(K_NN, T.nvK)[None] * sig16 + 0.3 * sdq * radn[None]
            rank1 = np.empty_like(o1); rank1[np.arange(D.nq)[:, None], o1] = np.arange(K)[None]
            sets = []
            for p in pgrid(int(live.sum())):
                hk = np.where((rank1 < 4 * p) & live[None], h, -np.inf)
                kth = -np.sort(-hk, 1)[:, min(p, K) - 1]
                sets.append((hk >= kth[:, None]) & (hk > -np.inf))
            res["T2"] = tuned_curve(T, None, sets=sets)
        else:
            res["T2"] = res["T1"]
        show("T2", res["T2"])
    # T3: greedy with the group penalty
    o3 = np.zeros_like(o1)
    for i in range(D.nq):
        key = s1[i].copy()
        opened = np.zeros(G, bool)
        pen = 0.25 * sdq[i, 0]
        for j in range(int(live.sum())):
            c = int(np.argmax(key - pen * ~opened[grpK]))
            o3[i, j] = c
            key[c] = -np.inf
            opened[grpK[c]] = True
        rest = np.setdiff1d(np.arange(K), o3[i, :int(live.sum())])
        o3[i, int(live.sum()):] = rest
    res["T3"] = tuned_curve(T, o3); show("T3", res["T3"])
    skey = mu["sub"] + 0.4 * sdq * radn_s[None]
    rank1 = np.empty_like(o1); rank1[np.arange(D.nq)[:, None], o1] = np.arange(K)[None]
    res["T4"] = tuned_sub_curve(T, (rank1 < 64)[:, par], skey); show("T4", res["T4"])
    gm = np.zeros((D.nq, G), bool)
    np.put_along_axis(gm, grpK[o1[:, :32]], True, 1)
    res["T5"] = tuned_sub_curve(T, gm[:, grpK[par]], skey); show("T5", res["T5"])
    # the process's order alone: clusters by P(tau*) (tau* from sum n P = k, or t), stopped by a probe count and
    # costed as the tuned arms are: isolates the order from the stopping rule and the choice of path
    for tier, nm in (("S0", "O0"), ("S1", "O1"), ("S2", "O2")):
        tk = TAB[("cluster", tier)]
        key = np.zeros((D.nq, K))
        for i in range(D.nq):
            if T.thr is None:
                lo, hi = -1.0, 1.0
                for _ in range(30):
                    mid = 0.5 * (lo + hi)
                    if (T.nvK * tk.P((mid - mu["cluster"][i]) / sp[tier]["cluster"][i], F.lr["cluster"])).sum() > T.tot[i]:
                        lo = mid
                    else:
                        hi = mid
                t = 0.5 * (lo + hi)
            else:
                t = T.tau_true[i]
            key[i] = tk.lnP((t - mu["cluster"][i]) / sp[tier]["cluster"][i], F.lr["cluster"])
        res[nm] = tuned_curve(T, np.argsort(-np.where(live[None], key, -np.inf), 1, kind="stable")); show(nm, res[nm])
    arms = [("P0", dict(tier="S0")), ("P1", dict(tier="S1")), ("P0i", dict(tier="S0", direct_alt=False)),
            ("P1i", dict(tier="S1", direct_alt=False)), ("P0s", dict(tier="S0", subs=True)), ("P1s", dict(tier="S1", subs=True)),
            ("P1si", dict(tier="S1", subs=True, direct_alt=False)),
            ("P2", dict(tier="S2")), ("P2i", dict(tier="S2", direct_alt=False)), ("P2s", dict(tier="S2", subs=True)),
            ("P2si", dict(tier="S2", subs=True, direct_alt=False))]
    if full:
        arms += [("P1 ag0", dict(tier="S1", ag_zero=True)), ("P1 tau", dict(tier="S1", tau_mode="true")), ("P2 tau", dict(tier="S2", tau_mode="true")),
                 ("clair", dict(tier="S1", clair=True)), ("clair i", dict(tier="S1", clair=True, direct_alt=False))]
    for name, kw in arms:
        res[name] = process_curve(T, **kw); show(name, res[name])
    json.dump(out, open(sys.argv[2], "w"))


if len(sys.argv) > 4:  # the tau ablation: tau* solved once and never updated (one plan), every k-NN task
    STAR = [("P0*", dict(tier="S0")), ("P1*", dict(tier="S1")), ("P2*", dict(tier="S2")), ("P1i*", dict(tier="S1", direct_alt=False)),
            ("P2i*", dict(tier="S2", direct_alt=False)), ("P1s*", dict(tier="S1", subs=True)), ("P2s*", dict(tier="S2", subs=True)),
            ("P1si*", dict(tier="S1", subs=True, direct_alt=False)), ("P2si*", dict(tier="S2", subs=True, direct_alt=False))]
    for T in [Task("knn")] + [Task(f"knn {f}", D.visible(f)) for f in ("random-1", "random-25", "clus-1", "clus-25", "cap-1", "cap-25")]:
        res = out.setdefault(T.name, {})
        print(f"\n== {D.name} | {T.name} | tau* never updated")
        for name, kw in STAR:
            res[name] = process_curve(T, tau_mode="star", **kw); show(name, res[name])
        json.dump(out, open(sys.argv[2], "w"))
    sys.exit()
run_task(Task("knn"))
for f in ("random-1", "random-25", "clus-1", "clus-25", "cap-1", "cap-25"):
    run_task(Task(f"knn {f}", D.visible(f)), full=False)
for c in range(D.thr.shape[1]):
    run_task(Task(f"thr {c}", None, c), full=False)
log("done")
