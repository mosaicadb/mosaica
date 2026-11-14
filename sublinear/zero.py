# Step 2. What is decided with no per-item code read, at a fixed answer size k, as N grows (nested subsamples, each with its
# own clustering of ~1,000 items per cluster) and as the visible set shrinks (the 1M clustering, visible subsets).
# Counted per query: clusters opened and items left undecided by the ball cut, the sorted annulus range, the support table
# plus range, and an oracle that knows each cluster's exact score extremes; and the shell count #{x : q.x >= t - w}.
# Every interval is checked against the exact score on every item it covers; violations are counted.
import numpy as np, json, os, sys
from common import *
Q, qtype, qkind, qi = queries()
rank = np.load(os.path.join(OUT, "rank.npy"))
KK = [10, 100, 1000, 10000]
SLACK = 2e-4
WS = [1, 2, 3, 4, 5, 6, 8]                      # shell widths in units of the query's score sd
WA = [0.02, 0.05, 0.1, 0.2, 0.3]                # absolute shell widths (cosine)
VIS = [977, 3906, 15625, 31250, 62500, 125000, 250000, 500000, 999900]

class Clu:
    def __init__(self, j, members):
        d = np.load(os.path.join(OUT, f"cl{j}.npz")); self.idx = members
        lab = d["lab"][members]; self.order = np.argsort(lab, kind="stable"); self.idx = members[self.order]; self.lab = lab[self.order]
        self.K = len(d["cnt"]); self.Mean = d["Mean"]; self.mn = np.linalg.norm(self.Mean, axis=1); self.Mh = self.Mean / self.mn[:, None]
        self.a = np.minimum(1, d["a"][self.idx]); self.nu = np.sqrt(np.maximum(0, 1 - self.a ** 2))
        self.st = np.concatenate([[0], np.flatnonzero(np.diff(self.lab)) + 1]); self.cl = self.lab[self.st]
        self.cnt = np.diff(np.concatenate([self.st, [len(self.lab)]]))
        # ball radius over ALL members of the stored clustering (valid for any subset of them)
        full = d["lab"]; m = full >= 0; amin = np.full(self.K, 1.0); np.minimum.at(amin, full[m], d["a"][m]); self.th = np.arccos(np.clip(amin, -1, 1))
        self.Pmax = d["Pmax"]; self.Pmin = d["Pmin"]; self.MM = self.Mh @ self.Mh.T
    def bounds(self, q):
        qmh = np.clip(self.Mh @ q, -1, 1); qp = np.sqrt(np.maximum(0, 1 - qmh ** 2)); c = self.lab
        w = qp[c] * self.nu; Ua = qmh[c] * self.a + w + SLACK; La = qmh[c] * self.a - w - SLACK
        j = int(np.argmax(qmh)); b0 = float(qmh[j]); rm = qmh - b0 * self.MM[j]; rr = np.sqrt(np.maximum(0, (1 - b0 * b0) - rm * rm))
        hi = np.maximum(b0 * self.Pmax[:, j], b0 * self.Pmin[:, j]); lo = np.minimum(b0 * self.Pmax[:, j], b0 * self.Pmin[:, j])
        Ut = hi[c] + rm[c] * self.a + rr[c] * self.nu + SLACK; Lt = lo[c] + rm[c] * self.a - rr[c] * self.nu - SLACK
        phi = np.arccos(qmh)
        Ub = np.cos(np.maximum(0, phi - self.th)) + SLACK; Lb = np.cos(np.minimum(np.pi, phi + self.th)) - SLACK          # ball, per cluster
        p0 = np.arccos(np.clip(b0, -1, 1)); t1 = np.arccos(np.clip(self.Pmax[:, j], -1, 1)); t2 = np.arccos(np.clip(self.Pmin[:, j], -1, 1))
        gap = np.where(p0 < t1, t1 - p0, np.where(p0 > t2, p0 - t2, 0.0))
        Uc = np.cos(gap) + SLACK; Lc = np.cos(np.minimum(np.pi, p0 + t2)) - SLACK                                         # table row alone, per cluster
        return Ua, La, np.minimum(Ua, Ut), np.maximum(La, Lt), Ub, Lb, Uc, Lc, Ut, Lt

def kth(s, k): return float(np.partition(s, len(s) - k)[len(s) - k])

class View:
    """A visible subset of a clustering's items (None = all), with its cluster boundaries."""
    def __init__(self, C, mask):
        self.sel = None if mask is None else np.flatnonzero(mask)
        self.lab = C.lab if mask is None else C.lab[self.sel]
        self.st = np.concatenate([[0], np.flatnonzero(np.diff(self.lab)) + 1]); self.cl = self.lab[self.st]
        self.cntv = np.bincount(self.lab, minlength=C.K)

def measure(C, vw, s, ks, sd, med, glob=None):
    """s: scores in C's order; vw: the visible subset."""
    Ua, La, Ux, Lx, Ub, Lb, Uc, Lc, Ut, Lt = C._b
    lab = vw.lab; cntv = vw.cntv; K = C.K
    if vw.sel is not None: sv = s[vw.sel]; Ua, La, Ux, Lx = Ua[vw.sel], La[vw.sel], Ux[vw.sel], Lx[vw.sel]
    else: sv = s
    nv = len(sv)
    cmax = np.full(K, -2.0); cmin = np.full(K, 2.0); cmax[vw.cl] = np.maximum.reduceat(sv, vw.st); cmin[vw.cl] = np.minimum.reduceat(sv, vw.st)
    out = {"n": nv, "clusters_nonempty": int((cntv > 0).sum())}
    srt = np.sort(sv)
    for k in ks:
        if k * 3 > nv: continue
        t = float(srt[nv - k]); r = {"t": t, "z": (t - med) / sd}
        ob = (Lb < t) & (Ub >= t) & (cntv > 0); r["ball_clusters"] = int(ob.sum()); r["ball_items"] = int(cntv[ob].sum())
        ua = (La < t) & (Ua >= t); r["annulus_items"] = int(ua.sum()); r["annulus_clusters"] = int((np.bincount(lab[ua], minlength=K) > 0).sum())
        ux = (Lx < t) & (Ux >= t); r["table_items"] = int(ux.sum()); r["table_clusters"] = int((np.bincount(lab[ux], minlength=K) > 0).sum())
        oc = (Lc < t) & (Uc >= t) & (cntv > 0); r["table_centres_read"] = int(oc.sum())
        mx = (cmin < t) & (cmax >= t); r["oracle_clusters"] = int(mx.sum()); r["oracle_items"] = int(cntv[mx].sum())
        mx2 = (cmin - 0.02 < t) & (cmax + 0.02 >= t); r["oracle02_items"] = int(cntv[mx2].sum())
        if glob is not None:      # extremes over all items of the clustering (what a cache built without the viewer's mask holds)
            gm = (glob[1] < t) & (glob[0] >= t) & (cntv > 0); r["oracle_all_items"] = int(cntv[gm].sum()); r["oracle_all_clusters"] = int(gm.sum())
        r["shell_sd"] = [int(nv - np.searchsorted(srt, t - w * sd, side="left")) for w in WS]
        r["shell_abs"] = [int(nv - np.searchsorted(srt, t - w, side="left")) for w in WA]
        out[str(k)] = r
    return out

def main():
    q0, q1 = int(sys.argv[1]), int(sys.argv[2]); tag = sys.argv[3]
    members = [np.flatnonzero(rank < s) for s in SIZES]
    CL = [Clu(j, members[j]) for j in range(len(SIZES))]
    top = CL[-1]; rk = rank[top.idx]
    vmask = {f"rand{v}": rk < v for v in VIS[:-1]}
    nq_before = np.cumsum(np.isin(np.arange(N), qi))                 # id-range sets: the first v non-query rows
    vmask.update({f"range{v}": (top.idx - nq_before[top.idx]) < v for v in VIS[:-1]})
    VW = [View(C, None) for C in CL]; VV = {name: View(top, m) for name, m in vmask.items()}
    viol = 0; checks = 0; recs = []
    for q in range(q0, q1):
        wait_lock()
        sfull = score_row(q); rec = {"q": q, "type": qtype[q], "kind": qkind[q], "sub": {}, "vis": {}}
        for j, C in enumerate(CL):
            s = sfull[C.idx]; C._b = C.bounds(Q[q]); Ua, La, Ux, Lx, Ub, Lb, Uc, Lc, Ut, Lt = C._b
            viol += int(((s > Ua) | (s < La)).sum()) + int(((s > Ut) | (s < Lt)).sum()); checks += 2 * len(s)
            cmax = np.maximum.reduceat(s, C.st); cmin = np.minimum.reduceat(s, C.st)
            viol += int(((cmax > Ub[C.cl]) | (cmin < Lb[C.cl])).sum()) + int(((cmax > Uc[C.cl]) | (cmin < Lc[C.cl])).sum()); checks += 2 * len(C.cl)
            sd = float(s.std()); med = float(np.median(s))
            if j == len(CL) - 1: rec["sd"] = sd; rec["med"] = med; rec["max"] = float(s.max())
            rec["sub"][str(SIZES[j])] = measure(C, VW[j], s, KK, sd, med)
            if j == len(CL) - 1:
                gl = (np.full(C.K, -2.0), np.full(C.K, 2.0)); gl[0][C.cl] = cmax; gl[1][C.cl] = cmin
                for name, vw in VV.items(): rec["vis"][name] = measure(C, vw, s, KK, sd, med, glob=gl)
        recs.append(rec)
        if (q - q0) % 20 == 0: log("query", q, "violations", viol, "of", checks)
    json.dump({"viol": viol, "checks": checks, "recs": recs}, open(os.path.join(OUT, f"zero_{tag}.json"), "w"))
    log("done; violations", viol, "of", checks, "interval checks")
main()
