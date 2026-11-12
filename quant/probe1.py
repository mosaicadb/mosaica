# Probe: how much narrower are deterministic-level certificates under (a) the exact range over box ∩ ellipsoid
# (Lagrangian dual, sound for every multiplier), (b) + the free sphere ||r|| = R from the stored a, (c) + the level-0
# ellipsoid, and (d) weights/allocation fitted on the mixed query profile instead of items only.
# Sample of p768 clusters and items; all queries; deterministic levels 0..32 B, no prefilter.  usage: python3 probe1.py [frac]
import numpy as np, sys, math
sys.path.insert(0, "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined")
from core import *
ROOT = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined/"
FRAC = float(sys.argv[1]) if len(sys.argv) > 1 else 0.02
NAME = sys.argv[2] if len(sys.argv) > 2 else "p768"
I = dict(np.load(ROOT + f"idx_{NAME}/index.npz")); Xs = np.load(ROOT + f"idx_{NAME}/Xs.npy", mmap_mode="r")
N, n, K, G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"])
assign, cstart, grp, Mq, mn, ahat = I["assign"], I["cstart"], I["grp"], I["Mq"], I["mn"], I["ahat"]
Q, qcls, thr = I["Q"], I["qcls"], I["thr"][:, :4]; NQ = len(Q)
LV = [0, 4, 8, 16, 32]; DA = 0.5 / 32767 + 1e-7
rng = np.random.default_rng(5)
gstart = np.searchsorted(grp, np.arange(G + 1))
VARS = ["cs", "box", "box+sph", "box+sph+e0"]
def dws(D, QV, MV):
    w2 = D.astype(np.float64) ** 2; Aqm = (MV * w2) @ QV.T; Bm = (MV * MV) @ w2; Cq = (QV * QV) @ w2
    al = Aqm / Bm[:, None]; dwa = np.sqrt(np.maximum(Cq[None] - Aqm ** 2 / Bm[:, None], 0)); return al.astype(np.float32), dwa.astype(np.float32)
def cellbox(u, dep, sig, code):
    """interval for z (not e) per coordinate implied by the escape code"""
    a = np.abs(u); s = np.where(u >= 0, 1.0, -1.0).astype(np.float32)
    wj = (W * np.exp2(2.0 - np.maximum(dep, 2))).astype(np.float32); idx = np.floor(a / wj)
    lo = idx * wj; hi = (idx + 1) * wj
    lo = np.where(dep == 1, 0, lo); hi = np.where(dep == 1, np.inf, hi)
    lo = np.where(dep == 0, 0, lo); hi = np.where(dep == 0, np.inf, hi)
    zl = np.where(s > 0, lo, -hi) * sig; zh = np.where(s > 0, hi, -lo) * sig
    zl = np.where(dep == 0, -np.inf, zl); zh = np.where(dep == 0, np.inf, zh)
    return zl - 1e-6 * np.abs(zl) - 1e-7, zh + 1e-6 * np.abs(zh) + 1e-7
def dual_max(w, zl, zh, quads, R2):
    """max w.z over box [zl,zh] ∩ {sum (z-c_k)^2 / D_k^2 <= rho_k^2} ∩ {|z|^2 <= R2}; min over a grid of multipliers.
    w: (B,Qn,n); zl,zh: (B,n); quads: list of (c (B,n), invD2 (n,), rho2 (B,), mu-scale (B,Qn)); R2 (B,)"""
    best = np.full(w.shape[:2], np.inf, np.float32)
    grids = []
    k0 = len(quads)
    for f in (0.45, 0.6, 0.75, 0.9, 1.0):
        grids.append([f if k == 0 else 0 for k in range(k0)] + [0])
    if R2 is not None:
        for f, g_ in ((0.75, 0.15), (0.9, 0.1), (0.6, 0.3), (0, 1.0)):
            grids.append([f] + [0] * (k0 - 1) + [g_])
    if k0 > 1:
        for f, g0 in ((0.75, 0.2), (0.6, 0.4), (0.9, 0.1)):
            grids.append([f, g0] + [0] * (k0 - 2) + ([0.05] if R2 is not None else [0]))
    for gr in grids:
        A = np.zeros(w.shape, np.float32); Bv = np.zeros(w.shape, np.float32); const = np.zeros(w.shape[:2], np.float32)
        for (c, iD2, rho2, msc), f in zip(quads, gr[:k0]):
            if f == 0: continue
            mu = (f * msc)[:, :, None]; A += mu * iD2; Bv += mu * iD2 * c[:, None, :]; const += f * msc * rho2[:, None]
            const -= (f * msc * ((c * c * iD2).sum(1))[:, None])
        if gr[-1] > 0 and R2 is not None:
            nu = (gr[-1] * nusc)[:, :, None]; A += nu; const += gr[-1] * nusc * R2[:, None]
        # phi(z) = w z - sum mu (z - c)^2 iD2 - nu z^2 = w z - A z^2 + 2 Bv z - (mu c^2 iD2 terms, moved to const)
        zs = (w + 2 * Bv) / (2 * np.maximum(A, 1e-30)); zs = np.clip(zs, zl[:, None], zh[:, None])
        val = (w * zs - A * zs * zs + 2 * Bv * zs).sum(2) + const
        best = np.minimum(best, val)
    return best
out = {v: {cl: np.zeros((len(LV) + 1, 4)) for cl in ("item", "text")} for v in VARS + ["cs-mix", "box+sph+e0-mix"]}
cost = {v: {cl: np.zeros(4) for cl in ("item", "text")} for v in out}
hsum = {v: np.zeros((len(LV), 2)) for v in out}; hcnt = np.zeros((len(LV), 2))
viol = {v: 0 for v in out}; npairs = {"item": 0, "text": 0}
for g in range(G):
    ks = np.arange(gstart[g], gstart[g + 1])
    if len(ks) == 0: continue
    lam, V, Qi, Qm = I["lam"][g], I["V"][g], I["Qi"][g], I["Qmix"][g]; sig = np.sqrt(lam).astype(np.float32)
    QV = (Q @ V).astype(np.float32); MV = (Mq[ks] @ V).astype(np.float32)
    PROF = {"": Qi, "-mix": Qm}; SET = {}
    for tag, Qp in PROF.items():
        se, dep_at, sdep, expc = esc_steps(lam, Qp, 1.5 * 8 * LV[-1]); s2 = np.where(sdep == 2)[0]
        Ds = [(((lam + 1e-10) / (Qp + 1e-10)) ** 0.25).astype(np.float32)]; deps = [None]
        for l in range(1, len(LV)):
            de = dep_at[int((expc <= 8 * LV[l]).sum())]; Ds.append((((lam * gE[de] + 1e-10) / (Qp + 1e-10)) ** 0.25).astype(np.float32))
        SET[tag] = (se, dep_at, s2, Ds, [dws(D, QV, MV) for D in Ds])
    for ci, c in enumerate(ks):
        lo_, hi_ = int(cstart[c]), int(cstart[c + 1]); nc = hi_ - lo_
        take = np.sort(rng.choice(nc, max(1, int(round(nc * FRAC))), replace=False)) + lo_
        x = unit(Xs[take]); true = x @ Q.T; z = (x - Mq[c]) @ V; u = z / sig
        ah = ahat[take].astype(np.float32) / 32767; m_ = mn[c]; mr = (m_ * ah - m_ * m_)[:, None]; qmc = (Q @ Mq[c])[None]
        R2 = (1 + m_ * m_ - 2 * m_ * (ah - DA)) * (1 + 1e-5) + 1e-6
        for cl in npairs: npairs[cl] += len(take) * int((qcls == cl).sum())
        for tag, (se, dep_at, s2, Ds, DW) in SET.items():
            Cst = np.ones((len(take), len(se)), np.int32); k2 = np.floor(np.abs(u) / W).astype(np.int32)
            Cst[:, s2] = k2[:, se[s2]] + 1; cum = np.cumsum(Cst, 1)
            vs = ["cs", "box", "box+sph", "box+sph+e0"] if tag == "" else ["cs", "box+sph+e0"]
            LB = {v: np.full(true.shape, -np.inf, np.float32) for v in vs}; UB = {v: np.full(true.shape, np.inf, np.float32) for v in vs}
            quads = []
            for l in range(len(LV)):
                D = Ds[l]; al, dwa = DW[l]; al = al[ci][None]; dwa = dwa[ci][None]
                if l == 0: dep = np.zeros_like(u, dtype=np.int64); code = np.zeros_like(z)
                else: dep = dep_at[(cum <= 8 * LV[l]).sum(1)].astype(np.int64); code = recon_esc(u, dep) * sig
                e = z - code; rho = roundup16(np.linalg.norm(e / D, axis=1)).astype(np.float32)
                cq = code @ QV.T; cmv = (code @ MV[ci])[:, None]
                est = qmc + cq + al * (mr - cmv); slack = np.abs(al) * m_ * DA
                hcs = dwa * rho[:, None]
                iD2 = (1 / (D.astype(np.float32) ** 2))
                # dual: work in z; objective w.z with w = QV - al MV (per query); est_z = qmc + al*mr ; bound on w.z
                wv = (QV[None] - al[0][:, None][None] * MV[ci][None, None, :]).astype(np.float32)   # (1,Qn,n)
                wv = np.broadcast_to(wv, (len(take),) + wv.shape[1:])
                base = (qmc + al * mr)
                zl, zh = cellbox(u, dep, sig, code)
                msc = (dwa / (2 * rho[:, None])).astype(np.float32)          # CS-optimal mu for the current ellipsoid
                cur = (code, iD2, rho * rho, msc)
                if l == 0: e0 = (np.zeros_like(z), iD2, rho * rho, None)
                wn = np.linalg.norm(wv, axis=2); nusc = (wn / (2 * np.sqrt(R2)[:, None])).astype(np.float32)
                res = {}
                res["cs"] = (est - hcs, est + hcs)
                if tag == "":
                    ub = dual_max(wv, zl, zh, [cur], None); lb = -dual_max(-wv, zl, zh, [cur], None)
                    res["box"] = (base + np.maximum(lb, -hcs + (est - base)), base + np.minimum(ub, hcs + (est - base)))
                    ub = dual_max(wv, zl, zh, [cur], R2); lb = -dual_max(-wv, zl, zh, [cur], R2)
                    res["box+sph"] = (base + np.maximum(lb, -hcs + (est - base)), base + np.minimum(ub, hcs + (est - base)))
                if l == 0: res["box+sph+e0"] = res.get("box+sph") or (lambda ub, lb: (base + np.maximum(lb, -hcs + (est - base)), base + np.minimum(ub, hcs + (est - base))))(dual_max(wv, zl, zh, [cur], R2), -dual_max(-wv, zl, zh, [cur], R2))
                else:
                    q0 = (e0[0], e0[1], e0[2], (msc * 0 + np.float32(1)) * E0SC)
                    ub = dual_max(wv, zl, zh, [cur, q0], R2); lb = -dual_max(-wv, zl, zh, [cur, q0], R2)
                    res["box+sph+e0"] = (base + np.maximum(lb, -hcs + (est - base)), base + np.minimum(ub, hcs + (est - base)))
                if l == 0: E0SC = msc.copy()
                for v in vs:
                    lb, ub = res[v]; sl = slack * 1.0001 + 2e-5 + 1e-4 * np.abs(ub - lb)
                    lb = lb - sl; ub = ub + sl
                    viol[v + tag] += int(((true < lb) | (true > ub)).sum())
                    np.maximum(LB[v], lb, out=LB[v]); np.minimum(UB[v], ub, out=UB[v])
                    for ci2, cl in enumerate(("item", "text")):
                        m = qcls == cl; hsum[v + tag][l, ci2] += float(((ub - lb)[:, m] / 2).sum())
                    for ti in range(4):
                        t = thr[:, ti]; alive = (LB[v] < t) & (UB[v] >= t)
                        for cl in ("item", "text"):
                            m = qcls == cl; out[v + tag][cl][l + 1, ti] += alive[:, m].sum()
                            if l == 0: out[v + tag][cl][0, ti] += len(take) * m.sum()
    print("group", g, "viol", viol, flush=True)
INC = [3, 5, 5, 9, 17]
print(f"\n{NAME}, sample frac {FRAC}; pairs {npairs}; violations {viol}")
print("mean half-width per level (0,4,8,16,32 B), by class:")
for v in out:
    print(f"  {v:16s} item", " ".join(f"{hsum[v][l, 0] / npairs['item']:.4f}" for l in range(5)), "| text", " ".join(f"{hsum[v][l, 1] / npairs['text']:.4f}" for l in range(5)))
for cl in ("item", "text"):
    print(f"\n{cl}: share undecided after each level (0.01% / 0.1% / 1% / 5%), and proxy bytes through 32 B + 17*undecided")
    for v in out:
        o = out[v][cl] / out[v][cl][0]
        cst = (np.array(INC)[:, None] * o[:5]).sum(0)
        print(f"  {v:16s}", " | ".join(" ".join(f"{o[l, ti]:.3f}" for l in range(1, 6)) for ti in range(4)), "| proxy", " ".join(f"{c:.2f}" for c in cst))
