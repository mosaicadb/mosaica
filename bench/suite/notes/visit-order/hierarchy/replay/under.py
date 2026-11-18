# Under: sub-clusters inside the clusters of the real fit (clusters, groups and rows unchanged; a sub-cluster is a
# list of rows of its cluster with a centre and a radius). Recall against items scanned, centres scored, groups
# touched and clusters touched (distinct parent clusters of the probed sub-clusters), for k-NN (k = 10) with and without visible sets, and for threshold queries.
#
# Leaves probed: clusters (flat), or sub-clusters ranked all together (direct), or two-level (the first P clusters by
# the flat spread key, then their sub-clusters ranked together), or group-limited (the sub-clusters of every cluster
# in the groups of the first P clusters, ranked together).
# Keys (qm = q.centre; sdK(q) = s.d. of q.m_c over the clusters; rad = RMS distance of a leaf's items to its centre,
# divided by its s.d. over the leaves of the structure; n = the leaf's visible items):
#   centre      qm
#   nonin .2    qm + 0.2 rad (raw rad; not invariant to a shift of the query; reports/annideas item 3)
#   spread f    qm + f sdK(q) rad
#   H r         qm + alpha z(10, n) sigma(q) + 0.3 sdK(q) rad, sigma^2 = q'Sigma q (top r directions + isotropic rest)
#   Hpar        the same with the parent cluster's rank-16 sigma scaled by rad_sub / rad_parent
#   gbT f       spread f with the sub-centre score taken as q.m_parent + the first T coordinates of the group's basis
#               (q.(m_sub - m_parent) = (V'q).(V^-1 (m_sub - m_parent)), truncated to T terms)
#   zeta        threshold queries: (qm - t) / sigma (the share of a leaf's items above t under a normal model)
# usage: python3 -I under.py DS OUT.json
import sys, os, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *
import numpy as np

D = DS(sys.argv[1])
out = {}
K, d, Q = D.K, D.d, D.Q
Q64 = Q.astype(np.float64)
G = int(D.grp.max()) + 1

cs = cached(D.ds, "st_real", lambda: leaf_stats(D.X, D.a, K, 16, D.Mq))
qmK = Q64 @ D.Mq.astype(np.float64).T
sdK = qmK.std(1)[:, None]
sigK = {r: sigma(Q, cs, r) for r in (4, 16)}
log("cluster statistics")

# group bases: query coordinates V'q and the inverse for item-side coordinates
V = np.load(os.path.join(D.fit, "V.npy")).astype(np.float64)
n_pad = V.shape[1]
Qp = np.zeros((D.nq, n_pad)); Qp[:, :d] = Q64
PG = np.einsum("qn,gnt->gqt", Qp, V)  # G x nq x n
Vinv = np.stack([np.linalg.inv(V[g]) for g in range(G)])
TS = [t for t in (8, 16, 32, 64, 128) if t < d]

subs = {}
for s in (4, 16, 64):
    z = subsplit(D, s)
    sub, par = z["sub"], z["par"]
    L = len(par)
    r = 4 if s <= 16 else 0
    st = cached(D.ds, f"st_sub{s}", lambda: leaf_stats(D.X, sub, L, r))
    qm = Q64 @ st["mean"].astype(np.float64).T
    # the first T group coordinates of the sub-centre's offset from its cluster's stored centre
    A = np.zeros((L, max(TS)))
    delta = np.zeros((L, n_pad)); delta[:, :d] = st["mean"].astype(np.float64) - D.Mq[par].astype(np.float64)
    gl = D.grp[par]
    qm_gb = {t: np.zeros((D.nq, L)) for t in TS}
    for g in range(G):
        ix = np.where(gl == g)[0]
        if not len(ix):
            continue
        Ag = delta[ix] @ Vinv[g].T
        if g == 0:
            err = np.abs(PG[g] @ Ag.T + qmK[:, par[ix]] - qm[:, ix]).max()
            assert err < 2e-3, err  # the full sum is the exact score (to the f32 basis)
        for t in TS:
            qm_gb[t][:, ix] = qmK[:, par[ix]] + PG[g][:, :t] @ Ag[:, :t].T
    subs[s] = dict(sub=sub, par=par, L=L, st=st, qm=qm, qm_gb=qm_gb, r=r,
                   sig=sigma(Q, st, 4) if r else None)
    log(f"sub x{s}: {L} leaves")


def flat_keys(nv, thr=None):
    radn = cs["rad"] / sdn(cs["rad"])
    ks = {"centre": qmK, "spread .4": qmK + 0.4 * sdK * radn[None]}
    if thr is None:
        dense = nv.sum() >= 8 * KNN * (nv > 0).sum()
        for r, al in ((4, 0.5), (16, 0.75)):
            ks[f"H r{r}"] = (qmK + al * z_of(KNN, nv)[None] * sigK[r] + 0.3 * sdK * radn[None]) if dense else ks["spread .4"]
    else:
        ks["zeta r16"] = (qmK - thr[:, None]) / sigK[16]
    return ks


def sub_keys(S, nv, thr=None):
    st, qm, par = S["st"], S["qm"], S["par"]
    radn = st["rad"] / sdn(st["rad"])
    ks = {"centre": qm, "nonin .2": qm + 0.2 * st["rad"][None]}
    for f in (0.2, 0.4, 0.6, 0.8, 1.2):
        ks[f"spread {f:g}"] = qm + f * sdK * radn[None]
    sp = sigK[16][:, par] * (st["rad"] / np.maximum(cs["rad"][par], 1e-9))[None]
    if thr is None:
        z = z_of(KNN, nv)[None]
        for al in (0.5, 0.75, 1.0):
            ks[f"Hpar a{al:g}"] = qm + al * z * sp + 0.3 * sdK * radn[None]
            if S["r"]:
                ks[f"Hown4 a{al:g}"] = qm + al * z * S["sig"] + 0.3 * sdK * radn[None]
        for t in TS:
            for f in (0.4, 0.8):
                ks[f"gb{t} {f:g}"] = S["qm_gb"][t] + f * sdK * radn[None]
    else:
        ks["zeta par"] = (qm - thr[:, None]) / sp
        if S["r"]:
            ks["zeta own4"] = (qm - thr[:, None]) / S["sig"]
    return ks


def task(tname, vis, thr_col=None):
    res = out.setdefault(tname, {})
    sets = D.truth_knn(vis) if thr_col is None else D.truth_thr(thr_col, vis)
    tot = np.array([len(x) for x in sets], np.float64)
    thr = None if thr_col is None else D.thr[:, thr_col]
    w = np.ones(D.N) if vis is None else vis.astype(np.float64)
    Nv = int(w.sum())
    print(f"\n== {D.name} | {tname} | visible {Nv} | answers per query {tot.mean():.0f}")
    print(f"{'structure, key':38s} recall at {' '.join(f'{x:g}' for x in SHARES)} of visible | items/groups/centres at recall "
          + ", ".join(str(t) for t in TARGETS))

    def ev(name, key, a, L, lg, extra, par=None):
        nv = np.bincount(a, weights=w, minlength=L)
        r = run(key, counts(a, L, sets), nv, lg, nv > 0, np.arange(L) if par is None else par)
        s = summarise(r, tot, Nv, extra)
        res[name] = s
        print(f"{name:38s} {fmt(s, ('items', 'groups', 'centres'))}", flush=True)

    nvK = np.bincount(D.a, weights=w, minlength=K)
    liveK = int((nvK > 0).sum())
    fk = flat_keys(nvK, thr)
    for kn, key in fk.items():
        ev(f"flat {kn}", key, D.a, K, D.grp, dict(centres=liveK))
    pkey = np.where(nvK[None] > 0, fk["spread .4"], -np.inf)
    prank = np.argsort(np.argsort(-pkey, 1, kind="stable"), 1)
    for s, S in subs.items():
        nv = np.bincount(S["sub"], weights=w, minlength=S["L"])
        live = int((nv > 0).sum())
        sk = sub_keys(S, nv, thr)
        lg = D.grp[S["par"]]
        for kn, key in sk.items():
            ev(f"x{s} direct {kn}", key, S["sub"], S["L"], lg, dict(centres=live), S["par"])
        two = ["spread 0.4", "spread 0.8"] + (["Hown4 a0.75"] if "Hown4 a0.75" in sk else []) + \
              (["zeta own4"] if "zeta own4" in sk else []) + [k for k in sk if k.startswith("gb16 ") or k.startswith("gb64 ")]
        for P in (8, 16, 32, 64, 128, 256):
            if P >= liveK:
                continue
            m = prank[:, S["par"]] < P
            scored = (m & (nv > 0)[None]).sum(1).mean()
            for kn in two:
                ev(f"x{s} two-level P{P} {kn}", np.where(m, sk[kn], -np.inf), S["sub"], S["L"], lg,
                   dict(centres=liveK + scored), S["par"])
            # group-limited: every sub-cluster in the groups of the first P clusters (a budget on groups, not clusters)
            gm = np.zeros((D.nq, G), bool)
            np.put_along_axis(gm, D.grp[np.argsort(-pkey, 1, kind="stable")[:, :P]], True, 1)
            m = gm[:, lg]
            scored = (m & (nv > 0)[None]).sum(1).mean()
            for kn in two[:1] + [k for k in two if k.startswith("Hown4")]:
                ev(f"x{s} group-limited P{P} {kn}", np.where(m, sk[kn], -np.inf), S["sub"], S["L"], lg,
                   dict(centres=liveK + scored), S["par"])


task("knn", None)
for f in ("random-1", "random-25", "clus-1", "clus-25", "cap-1", "cap-25"):
    task(f"knn {f}", D.visible(f))
for c, sh in enumerate(D.thr.shape[1] * [None]):
    task(f"thr {c}", None, c)
json.dump(out, open(sys.argv[2], "w"))
log("done")
