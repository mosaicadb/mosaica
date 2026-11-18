# Over: a tree above the cluster centres, searched with a beam. Partitions: the real fit (K about 500; unchanged
# clusters, groups and rows) and faiss k-means refits at K = 5,000 and 20,000 (a DIFFERENT partition: new clusters,
# new groups made as fit.py makes them, new row order). Trees: the groups as one level; top-down k-means of the
# centres with the fan-outs given (not size-balanced; the node sizes are printed).
# Beam: at every tree level the children of the kept nodes are scored and some kept; at the bottom the leaves
# (clusters) under the kept nodes are scored and the best nprobe probed. Two widths:
#   m<m>  the best B = m * nprobe nodes are kept at every level (all when there are fewer)
#   w<m>  the best nodes are kept until they hold m * nprobe live clusters between them (a width in leaves, so that
#         a level of large nodes keeps few of them; centres scored at the bottom are then about m * nprobe)
# Leaf key: spread = q.m_c + 0.4 sd(q) rad_c / sd(rad), sd(q) over every cluster (see `local` for the s.d. over the
# scored clusters only). Node keys (M the item mean of the node, sd_l over the nodes of its level):
#   c       q.M
#   sp f    q.M + f sd_l(q) R / sd_l(R), R the RMS distance of the node's items to M
#   st a    q.M + a z(1, n) sqrt(q'C q) + 0.4 sd(q) meanrad / sd(rad): C the covariance of the node's centres (top 8
#           directions + isotropic rest), n its live clusters, z(1, n) = Phi^-1(1 - 1/n), meanrad the mean rad of its
#           clusters (the leaf key's radius term carried up)
# Counted per (tree, key, m, nprobe): recall, items scanned, centres scored (node and cluster centres: the order's
# cost), groups touched, and the share of true answers lost at each level (the subtree holding it dropped there).
# usage: python3 -I over.py DS OUT.json
import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *
import numpy as np

D = DS(sys.argv[1])
Q = D.Q
Q64 = Q.astype(np.float64)
out = {}
MS = (1, 2, 4, 8)


def build_tree(C, fan, seed=3):
    K = len(C)
    levels, cur = [], np.zeros(K, np.int64)
    for b in fan:
        new, nid = np.zeros(K, np.int64), 0
        for g in range(int(cur.max()) + 1):
            ix = np.where(cur == g)[0]
            lab, nl = compact(kmeans(C[ix], min(b, max(1, len(ix) // 2)), 15, seed + g))
            new[ix] = nid + lab
            nid += nl
        levels.append(new)
        cur = new
    return levels


def node_stats(anc, st, r=8):
    """Per node of one level: item mean M, RMS radius R, mean rad of its clusters, and the covariance of its
    clusters' centres (item-weighted) as top r directions + trace."""
    n_nodes = int(anc.max()) + 1
    d = st["mean"].shape[1]
    M = np.zeros((n_nodes, d)); R = np.zeros(n_nodes); mr = np.zeros(n_nodes); tr = np.zeros(n_nodes)
    lam = np.zeros((n_nodes, r)); U = np.zeros((n_nodes, r, d), np.float32)
    o = np.argsort(anc, kind="stable")
    stt = np.r_[0, np.cumsum(np.bincount(anc, minlength=n_nodes))]
    for g in range(n_nodes):
        ix = o[stt[g]:stt[g + 1]]
        w = st["n"][ix].astype(np.float64)
        c = st["mean"][ix].astype(np.float64)
        m = (w[:, None] * c).sum(0) / w.sum()
        M[g] = m
        xc = (c - m) * np.sqrt(w / w.sum())[:, None]
        tr[g] = (xc * xc).sum()
        R[g] = np.sqrt(tr[g] + (w * st["rad"][ix] ** 2).sum() / w.sum())
        mr[g] = (w * st["rad"][ix]).sum() / w.sum()
        if len(ix) > 1:
            _, s, v = np.linalg.svd(xc, full_matrices=False)
            k = min(r, len(s))
            lam[g, :k] = s[:k] ** 2
            U[g, :k] = v[:k]
    return dict(M=M, R=R, mr=mr, tr=tr, lam=lam, U=U)


class Part:
    def __init__(self, name, a, grp, centres=None, trees=()):
        self.name, self.a, self.grp = name, a, grp
        self.K = K = len(grp)
        self.st = st = cached(D.ds, f"st_{name}", lambda: leaf_stats(D.X, a, K, 4, centres))
        self.C = st["mean"] if centres is None else centres
        self.qm = Q64 @ self.C.astype(np.float64).T
        self.sd = self.qm.std(1)[:, None]
        self.radn = st["rad"] / sdn(st["rad"])
        self.sig4 = sigma(Q, st, 4)
        self.onehot = np.zeros((K, int(grp.max()) + 1), np.float32)
        self.onehot[np.arange(K), grp] = 1
        self.trees = {}
        for tn, fan in trees:
            lv = [grp.copy()] if fan == "groups" else build_tree(self.C, fan)
            levels = []
            for i, anc in enumerate(lv):
                ns = cached(D.ds, f"ns_{name}_{tn.replace(' ', '')}_{i}", lambda: node_stats(anc, st))
                par = None
                if i:
                    par = np.zeros(int(anc.max()) + 1, np.int64)
                    par[anc] = lv[i - 1]
                qM = Q64 @ ns["M"].T
                levels.append(dict(anc=anc, par=par, qM=qM, R=ns["R"] / sdn(ns["R"]), mr=ns["mr"] / sdn(st["rad"]),
                                   sg=sigma(Q, dict(lam=ns["lam"], U=ns["U"], tr=ns["tr"]), 8),
                                   nl=np.bincount(anc)))
            self.trees[tn] = levels
            log(f"{name} tree {tn}: nodes per level {[len(l['nl']) for l in levels]}, clusters per bottom node "
                f"{levels[-1]['nl'].min()}..{levels[-1]['nl'].max()} (median {int(np.median(levels[-1]['nl']))})")


def node_key(kind, lev, P, live_leaves):
    qM = lev["qM"]
    if kind == "c":
        return qM
    if kind.startswith("sp"):
        return qM + float(kind.split()[1]) * qM.std(1)[:, None] * lev["R"][None]
    al = float(kind.split()[1])
    nl = np.bincount(lev["anc"], weights=live_leaves.astype(np.float64), minlength=qM.shape[1])
    return qM + al * z_of(1, nl)[None] * lev["sg"] + 0.4 * P.sd * lev["mr"][None]


def leafmask(key, mask, nl, W):
    """The best nodes of `mask` by key, taken until they hold W leaves between them."""
    k = np.where(mask, key, -np.inf)
    o = np.argsort(-k, 1, kind="stable")
    before = np.cumsum(nl[o], 1) - nl[o]
    kept = np.zeros_like(mask)
    np.put_along_axis(kept, o, before < W, 1)
    return kept & mask


def topmask(key, mask, B):
    k = np.where(mask, key, -np.inf)
    if B >= key.shape[1]:
        return mask
    idx = np.argpartition(-k, B - 1, 1)[:, :B]
    kept = np.zeros_like(mask)
    np.put_along_axis(kept, idx, True, 1)
    return kept & mask


def pgrid(K):
    g, p = [], 1.0
    while p <= K:
        if not g or int(p) > g[-1]:
            g.append(int(p))
        p *= 1.2
    return g


def beam(P, tree, nkind, m, cnt, tot, nv, leafkey, local=False, stop=0.997, leaves=False):
    """The beam's curve over nprobe. Returns lists per nprobe."""
    live = nv > 0
    res = dict(p=[], rec=[], items=[], centres=[], groups=[], loss=[])
    levels = P.trees[tree] if tree else []
    nkeys = [node_key(nkind, lev, P, live) for lev in levels]
    nll = [np.bincount(lev["anc"], weights=live.astype(np.float64), minlength=len(lev["nl"])) for lev in levels]
    nlive = [x > 0 for x in nll]
    totsum = tot.sum()
    for p in pgrid(int(live.sum())):
        B = m * p
        centres = np.zeros(len(Q))
        loss = []
        kept = None
        dead = np.zeros(cnt.shape, bool)  # leaves already lost
        for li, lev in enumerate(levels):
            cand = np.broadcast_to(nlive[li][None], nkeys[li].shape) if kept is None else kept[:, lev["par"]] & nlive[li][None]
            centres += cand.sum(1)
            kept = leafmask(nkeys[li], cand, nll[li], B) if leaves else topmask(nkeys[li], cand, B)
            gone = ~kept[:, lev["anc"]] & ~dead
            loss.append(float((cnt * gone).sum() / totsum))
            dead |= gone
        cand = np.broadcast_to(live[None], cnt.shape) if kept is None else kept[:, levels[-1]["anc"]] & live[None]
        centres += cand.sum(1)
        key = leafkey
        if local:
            sdl = np.array([P.qm[i, cand[i]].std() if cand[i].sum() > 1 else 0.0 for i in range(len(Q))])[:, None]
            key = P.qm + 0.4 * sdl * P.radn[None]
        probed = topmask(key, cand, p)
        f = (cnt * probed).sum(1)
        loss.append(float((cnt * (cand & ~probed)).sum() / totsum))
        res["p"].append(p)
        res["rec"].append(float((f / np.maximum(tot, 1)).mean()))
        res["items"].append(float((probed * nv[None]).sum(1).mean()))
        res["centres"].append(float(centres.mean()))
        res["groups"].append(float(((probed.astype(np.float32) @ P.onehot) > 0).sum(1).mean()))
        res["loss"].append(loss)
        if res["rec"][-1] >= stop:
            break
    return res


def at(res, t, what):
    rec = np.array(res["rec"])
    hit = np.where(rec >= t)[0]
    if not len(hit):
        return None
    i = int(hit[0])
    v = np.array(res[what], np.float64)
    if i == 0:
        return float(v[0])
    f = (t - rec[i - 1]) / max(rec[i] - rec[i - 1], 1e-12)
    return float(v[i - 1] + f * (v[i] - v[i - 1]))


def line(name, res):
    c = []
    for t in TARGETS:
        x = [at(res, t, w) for w in ("items", "centres", "groups")]
        c.append("-" if x[0] is None else "/".join(f"{v:.0f}" for v in x))
    return f"{name:44s} " + " | ".join(f"{x:>18s}" for x in c)


def lossline(res, flat, t):
    """Per-level loss at the nprobe where the flat order reaches recall t."""
    hit = np.where(np.array(flat["rec"]) >= t)[0]
    if not len(hit) or hit[0] >= len(res["p"]):
        return "-"
    i = int(hit[0])
    return f"np {res['p'][i]:5d} rec {res['rec'][i]:.3f} (flat {flat['rec'][i]:.3f}) lost by level " + \
        " ".join(f"{x:.3f}" for x in res["loss"][i][:-1]) + f" bottom {res['loss'][i][-1]:.3f}"


def task(P, tname, vis, thr_col=None, full=True):
    res = out.setdefault(P.name, {}).setdefault(tname, {})
    sets = D.truth_knn(vis) if thr_col is None else D.truth_thr(thr_col, vis)
    tot = np.array([len(x) for x in sets], np.float64)
    w = np.ones(D.N) if vis is None else vis.astype(np.float64)
    nv = np.bincount(P.a, weights=w, minlength=P.K)
    cnt = counts(P.a, P.K, sets)
    print(f"\n== {D.name} | {P.name} (K {P.K}) | {tname} | visible {int(w.sum())}, live clusters {int((nv > 0).sum())} | "
          f"items/centres/groups at recall " + ", ".join(str(t) for t in TARGETS))
    spread = P.qm + 0.4 * P.sd * P.radn[None]
    leafkeys = {"spread": spread}
    if full:
        leafkeys["centre"] = P.qm
        if thr_col is None:
            dense = nv.sum() >= 8 * KNN * (nv > 0).sum()
            leafkeys["H r4"] = P.qm + 0.5 * z_of(KNN, nv)[None] * P.sig4 + 0.3 * P.sd * P.radn[None] if dense else spread
    if thr_col is not None:
        leafkeys["zeta r4"] = (P.qm - D.thr[:, thr_col][:, None]) / P.sig4
    flat = {}
    for kn, key in leafkeys.items():
        flat[kn] = res[f"flat {kn}"] = beam(P, None, "c", 1, cnt, tot, nv, key)
        print(line(f"flat {kn}", flat[kn]), flush=True)
    for tn in P.trees:
        for nk in (("c", "sp 1", "st 0.5", "st 1") if full else ("c", "st 1")):
            for m in (MS if full else ()):
                r = res[f"{tn} | {nk} | m{m}"] = beam(P, tn, nk, m, cnt, tot, nv, spread)
                print(line(f"{tn} | {nk} | m{m}", r), "||", lossline(r, flat["spread"], 0.9), flush=True)
            for m in ((2, 4, 8, 16) if full else (4, 16)):
                r = res[f"{tn} | {nk} | w{m}"] = beam(P, tn, nk, m, cnt, tot, nv, spread, leaves=True)
                print(line(f"{tn} | {nk} | w{m}", r), "||", lossline(r, flat["spread"], 0.9), flush=True)
        if full:
            for m in (4, 16):
                r = res[f"{tn} | st 1 local | w{m}"] = beam(P, tn, "st 1", m, cnt, tot, nv, spread, local=True, leaves=True)
                print(line(f"{tn} | st 1, local sd | w{m}", r), flush=True)
                if "H r4" in leafkeys:
                    r = res[f"{tn} | st 1 leaf H r4 | w{m}"] = beam(P, tn, "st 1", m, cnt, tot, nv, leafkeys["H r4"], leaves=True)
                    print(line(f"{tn} | st 1, leaf H r4 | w{m}", r), flush=True)


parts = [Part("real", D.a, D.grp, D.Mq, [("groups", "groups"), ("hkm 16", [16]), ("hkm 32", [32]), ("hkm 8x8", [8, 8])])]
for K, trees in ((5000, [("groups", "groups"), ("hkm 256", [256]), ("hkm 16x16", [16, 16]), ("hkm 32x32", [32, 32])]),
                 (20000, [("groups", "groups"), ("hkm 256", [256]), ("hkm 32x32", [32, 32]), ("hkm 16x16x16", [16, 16, 16])])):
    z = refit(D, K)
    parts.append(Part(f"refit{K}", z["a"], z["grp"], None, trees))
for P in parts:
    task(P, "knn", None)
    for f in ("random-1", "random-25", "clus-1", "clus-25", "cap-1", "cap-25"):
        task(P, f"knn {f}", D.visible(f), full=False)
    for c in range(D.thr.shape[1]):
        task(P, f"thr {c}", None, c, full=False)
    json.dump(out, open(sys.argv[2], "w"))
log("done")
