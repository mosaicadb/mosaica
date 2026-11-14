# Cluster-level certificates against the oracle, at four nested partition levels.
# For every query and answer size, counts items that no query-independent group summary decides,
# for a ladder of summaries, and the same ladder with the information held per member instead.
# Every bound is checked against the exact score; violations are counted.
import numpy as np, json, sys, time, os, multiprocessing as mp
from threadpoolctl import threadpool_limits
NPROC = int(os.environ.get("NPROC", "4")); _WORK = None
def _call(q0):
    with threadpool_limits(1): return _WORK(q0)
W = sys.argv[1]; LEVELS = [int(a) for a in sys.argv[2].split(",")]; OUT = sys.argv[3]
NQT = int(sys.argv[4]) if len(sys.argv) > 4 else 10**9
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
X = np.load(f"{W}/X.npy"); N, n = X.shape
ix = np.load(os.environ.get("INDEX", f"{W}/index.npz")); levels = ix["levels"]; grp0 = ix["grp"]; G = int(grp0.max()) + 1
qi = np.load(f"{W}/item_q_idx.npy"); Qitem = X[qi].copy()
Qtext = np.load(f"{W}/text_q.npy").astype(np.float32)[:NQT]; Qtext /= np.linalg.norm(Qtext, axis=1, keepdims=True)
kinds = [p[0] for p in json.load(open(f"{W}/text_q.json"))][:NQT]
Q = np.concatenate([Qtext, Qitem]); nq = len(Q); qkind = ["text"] * len(Qtext) + ["item"] * len(Qitem)
# one order in which every group and every cluster of every level is contiguous
perm = np.lexsort((levels[3], grp0[levels[0]])); X = X[perm]; levels = levels[:, perm]; gitem = grp0[levels[0]]
gb = np.concatenate([[0], np.flatnonzero(np.diff(gitem)) + 1, [N]])
S = np.empty((N, nq), np.float32)
for i in range(0, N, 100_000): S[i:i+100_000] = X[i:i+100_000] @ Q.T
FR = [1e-4, 1e-3, 1e-2, 5e-2]
TH = np.stack([np.partition(S[:, j], N - max(1, round(f * N)))[N - max(1, round(f * N))] for j in range(nq) for f in FR]).reshape(nq, len(FR))
log("scores done; mean thresholds text", TH[:len(Qtext)].mean(0).round(3), "item", TH[len(Qtext):].mean(0).round(3))
EDGES = [0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, n]; NB = len(EDGES) - 1
SLACK = 2e-4   # float32 arithmetic allowance on every bound
# MODE=pca: bound in each group's PCA basis. MODE=<file.npy>: a query-offset covariance; the bound then uses the dual pair of
# transforms that minimises the mean product of the two weighted norms (the geometric-mean metric), ridge EPS.
MODE = os.environ.get("MODE", "pca"); EPS = float(os.environ.get("EPS", "0.1")); DIAG = os.environ.get("DIAG", "0") == "1"
if MODE != "pca":
    SQ = np.load(MODE).astype(np.float64); SQ = SQ + EPS * np.trace(SQ) / n * np.eye(n)
    lq, vq = np.linalg.eigh(SQ); Aq = (vq * np.sqrt(lq)) @ vq.T; Aqi = (vq / np.sqrt(lq)) @ vq.T

def run_level(L):
    lab = levels[L]; cb = np.concatenate([[0], np.flatnonzero(np.diff(lab)) + 1, [N]]); K = len(cb) - 1; cs = cb[:-1]; cnt = np.diff(cb)
    cid = np.repeat(np.arange(K), cnt)
    Mean = (np.add.reduceat(X, cs, axis=0) / cnt[:, None]).astype(np.float32); mn = np.linalg.norm(Mean, axis=1); Mh = Mean / mn[:, None]
    cg = gitem[cs]
    a = np.concatenate([np.einsum("ij,ij->i", X[i:i+100_000], Mh[cid[i:i+100_000]]) for i in range(0, N, 100_000)]); nu = np.sqrt(np.maximum(0, 1 - a * a)); b = a * mn[cid] - mn[cid] ** 2
    # local bases per group from this level's residuals; residual coordinates stored by block
    Zb = [np.empty((N, EDGES[i+1] - EDGES[i]), np.float32) for i in range(NB)]
    VD = np.empty((G, n, n), np.float32); VQ = np.empty((G, n, n), np.float32); lam = np.empty((G, n))
    Mu = np.empty((K, n), np.float32)
    for g in range(G):
        s, e = gb[g], gb[g+1]; R = X[s:e] - Mean[cid[s:e]]
        cov = (R.T @ R).astype(np.float64) / max(e - s, 1)
        if MODE == "pca":
            l, v = np.linalg.eigh(cov); lam[g] = np.maximum(l[::-1], 1e-12); VD[g] = v[:, ::-1]; VQ[g] = v[:, ::-1]
        elif DIAG:
            # best weighting that is diagonal in the group's PCA basis: weights from the diagonal of the query covariance there
            l, v = np.linalg.eigh(cov); l = np.maximum(l, 1e-12 * l.max()); qd = np.einsum("ij,ik,kj->j", v, SQ, v)
            o = np.argsort(-qd * l); v = v[:, o]; l = l[o]; qd = qd[o]; lam[g] = qd * l; d = (l / qd) ** 0.25
            VD[g] = v / d; VQ[g] = v * d
        else:
            l, v = np.linalg.eigh(Aq @ cov @ Aq); l = np.maximum(l[::-1], 1e-12 * l.max()); v = v[:, ::-1]; lam[g] = l
            VD[g] = (Aq @ v) * l ** -0.25; VQ[g] = (Aqi @ v) * l ** 0.25
        Z = R @ VD[g]
        for i in range(NB): Zb[i][s:e] = Z[:, EDGES[i]:EDGES[i+1]]
        m = cg == g; Mu[m] = Mean[m] @ VQ[g]
        del R, Z
    Rn = np.stack([np.linalg.norm(z, axis=1) for z in Zb], 1)                      # per item, per block
    Mrb = np.stack([np.einsum("ij,ij->i", Zb[i], Mu[cid, EDGES[i]:EDGES[i+1]]) for i in range(NB)], 1)
    Rc = np.maximum.reduceat(Rn, cs, axis=0)                                       # per cluster maxima
    Mu2 = np.add.reduceat(Mu * Mu, EDGES[:-1], axis=1)
    bmax = np.maximum.reduceat(b, cs); bmin = np.minimum.reduceat(b, cs)
    rnorm = np.sqrt((Rn ** 2).sum(1)); Wc = np.maximum.reduceat(rnorm, cs)
    log(f"level {L}: K={K} median size {int(np.median(cnt))} rms residual {np.sqrt((rnorm**2).mean()):.4f} "
        f"half log-det of the top 64 local dimensions, bits per item {float((0.5*np.log2(lam[:, :64]).sum(1) * np.diff(gb)).sum() / N):.1f}")
    table = None
    if L == 0:
        Pmax = np.empty((K, K), np.float32); Pmin = np.empty((K, K), np.float32)
        for c0 in range(0, K, 50):
            c1 = min(K, c0 + 50); P = X[cb[c0]:cb[c1]] @ Mh.T; off = cs[c0:c1] - cb[c0]
            Pmax[c0:c1] = np.maximum.reduceat(P, off, axis=0); Pmin[c0:c1] = np.minimum.reduceat(P, off, axis=0)
        MM = Mh @ Mh.T; table = (Pmax, Pmin, MM); log("support table done")
    res = {"K": int(K), "median_size": int(np.median(cnt)), "rms_residual": float(np.sqrt((rnorm**2).mean())),
           "half_logdet_bits": {k: float((0.5*np.log2(lam[:, :k]).sum(1) * np.diff(gb)).sum() / N) for k in (16, 64, 256)}, "lam_block": [float(lam[:, EDGES[i]:EDGES[i+1]].sum(1).mean()) for i in range(NB)], "per_query": []}
    BQ = int(os.environ.get("BQ", "8"))
    def do_batch(q0):
        viol = 0; recs = []
        q1 = min(nq, q0 + BQ); Qb = Q[q0:q1]; m = q1 - q0
        C = np.empty((N, NB, m), np.float32); Qg = np.einsum("gij,qi->gjq", VQ, Qb)   # G x n x m
        for g in range(G):
            s, e = gb[g], gb[g+1]
            for i in range(NB): C[s:e, i] = Zb[i][s:e] @ Qg[g, EDGES[i]:EDGES[i+1]]
        QM = Mean @ Qb.T                                                           # K x m
        for jq in range(m):
            q = Qb[jq]; sc = np.ascontiguousarray(S[:, q0 + jq]); qm = QM[:, jq]; qmh = qm / mn; qg = Qg[cg, :, jq]   # K x n
            Cq = C[:, :, jq]
            qg2 = np.add.reduceat(qg * qg, EDGES[:-1], axis=1); qmu = np.add.reduceat(qg * Mu, EDGES[:-1], axis=1)
            # annulus (sorted range): free, uses the stored key a
            w = np.sqrt(np.maximum(0, 1 - qmh * qmh))[cid] * nu
            Ua = qmh[cid] * a + w + SLACK; La = qmh[cid] * a - w - SLACK
            viol += int(((sc > Ua) | (sc < La)).sum())
            if table is not None:
                Pmax, Pmin, MM = table; j = int(np.argmax(qmh)); beta = float(q @ Mh[j])
                rm = qmh - beta * MM[j]; rr = np.sqrt(np.maximum(0, (1 - beta * beta) - rm * rm))
                Ut = beta * Pmax[:, j][cid] + rm[cid] * a + rr[cid] * nu + SLACK; Lt = beta * Pmin[:, j][cid] + rm[cid] * a - rr[cid] * nu - SLACK
                viol += int(((sc > Ut) | (sc < Lt)).sum())
                Utab = np.minimum(Ua, Ut); Ltab = np.maximum(La, Lt)
            # exact parts and tails for alpha = alpha* (offset orthogonal to the centre) and alpha = 0
            parts = []; als = []
            for al in ((qmu.sum(1) / Mu2.sum(1), np.zeros(K, np.float32)) if MODE == "pca" else (qmu.sum(1) / Mu2.sum(1), qm / mn ** 2, np.zeros(K, np.float32))):
                D = Cq - al[cid, None] * Mrb; base = qm[cid] + al[cid] * b
                dn = np.sqrt(np.maximum(0, qg2 - 2 * al[:, None] * qmu + al[:, None] ** 2 * Mu2))   # K x NB
                parts.append((D, base, dn)); als.append(al)
            chk = parts[0][1] + parts[0][0].sum(1); assert np.abs(chk - sc).max() < 3e-3, np.abs(chk - sc).max()
            rec = {"kind": qkind[q0 + jq], "sub": kinds[q0 + jq] if q0 + jq < len(Qtext) else "item", "th": TH[q0 + jq].tolist()}
            # diagnostics
            zc = sc - qmh[cid] * a; wn = np.sqrt(np.maximum(1e-12, 1 - qmh * qmh))[cid] * np.maximum(nu, 1e-6)
            mc = np.add.reduceat(sc, cs) / cnt; vw = float(((sc - mc[cid]) ** 2).mean()); vt = float(sc.var())
            D, base, dn = parts[0]
            rec.update(between_share=1 - vw / vt, sd_total=np.sqrt(vt), sd_within=np.sqrt(vw), cos_rms=float(np.sqrt(((zc / wn) ** 2).mean())),
                       qperp=float(np.sqrt(np.maximum(0, 1 - qmh * qmh))[cid].mean()), cs_mean=float(wn.mean()),
                       blockcs_mean=float((dn[cid] * Rn).sum(1).mean()), dn2_block=(((dn ** 2) * cnt[:, None]).sum(0) / N).tolist(),
                       median=float(np.median(sc)))
            cmax = np.maximum.reduceat(sc, cs); cmin = np.minimum.reduceat(sc, cs)
            out = {k: [] for k in ("annulus", "table", "oracle", "oracle02", "oracle05")}
            for t in TH[q0 + jq]:
                out["annulus"].append(int(((La < t) & (Ua >= t)).sum()))
                if table is not None: out["table"].append(int(((Ltab < t) & (Utab >= t)).sum()))
                for key, d in (("oracle", 0.0), ("oracle02", 0.02), ("oracle05", 0.05)):
                    out[key].append(int(cnt[(cmin - d < t) & (cmax + d >= t)].sum()))
            # ladder: the first j blocks of local coordinates exact, the rest bounded block by block
            pre = []
            for D, base, dn in parts:
                Ecum = np.concatenate([base[:, None], base[:, None] + np.cumsum(D, axis=1)], 1)                  # N x (NB+1)
                Tm = np.concatenate([np.cumsum((dn[cid] * Rn)[:, ::-1], axis=1)[:, ::-1], np.zeros((N, 1), np.float32)], 1)
                Tc = np.concatenate([np.cumsum((dn * Rc)[:, ::-1], axis=1)[:, ::-1], np.zeros((K, 1))], 1)
                pre.append((Ecum, Tm, Tc))
            cl = [[] for _ in FR]; ca = [[] for _ in FR]; me = [[] for _ in FR]
            for j in range(NB + 1):
                Uc = np.full(K, np.inf); Lc = np.full(K, -np.inf); Um = np.full(N, np.inf, np.float32); Lm = np.full(N, -np.inf, np.float32)
                for Ecum, Tm, Tc in pre:
                    E = Ecum[:, j]
                    Um = np.minimum(Um, E + Tm[:, j] + SLACK); Lm = np.maximum(Lm, E - Tm[:, j] - SLACK)
                    Uc = np.minimum(Uc, np.maximum.reduceat(E, cs) + Tc[:, j] + SLACK); Lc = np.maximum(Lc, np.minimum.reduceat(E, cs) - Tc[:, j] - SLACK)
                viol += int(((sc > Um) | (sc < Lm)).sum()) + int(((cmax > Uc) | (cmin < Lc)).sum())
                for it, t in enumerate(TH[q0 + jq]):
                    uc = (Lc < t) & (Uc >= t)
                    cl[it].append(int(cnt[uc].sum())); ca[it].append(int(((La < t) & (Ua >= t) & uc[cid]).sum())); me[it].append(int(((Lm < t) & (Um >= t)).sum()))
            out["cluster"] = cl; out["cluster_ann"] = ca; out["member"] = me
            # one weighted norm per member instead of one per block: with the member's own m.r (two scalars), with the
            # group's extremes of m.r (one sortable key), and the group ball (no per-member read)
            U2 = np.full(N, np.inf, np.float32); L2 = np.full(N, -np.inf, np.float32); U1 = U2.copy(); L1 = L2.copy()
            Ub = np.full(K, np.inf); Lb = np.full(K, -np.inf)
            for (D, base, dn), al in zip(parts, als):
                dT = np.sqrt((dn ** 2).sum(1)); t2 = dT[cid] * rnorm + SLACK
                U2 = np.minimum(U2, base + t2); L2 = np.maximum(L2, base - t2)
                hi = qm + np.maximum(al * bmax, al * bmin); lo = qm + np.minimum(al * bmax, al * bmin)
                U1 = np.minimum(U1, hi[cid] + t2); L1 = np.maximum(L1, lo[cid] - t2)
                Ub = np.minimum(Ub, hi + dT * Wc + SLACK); Lb = np.maximum(Lb, lo - dT * Wc - SLACK)
            viol += int(((sc > U2) | (sc < L2)).sum()) + int(((sc > U1) | (sc < L1)).sum()) + int(((cmax > Ub) | (cmin < Lb)).sum())
            for key in ("norm2", "norm1", "norm1_ann", "ball", "ball_norm1"): out[key] = []
            for t in TH[q0 + jq]:
                u1 = (L1 < t) & (U1 >= t); ub = (Lb < t) & (Ub >= t)
                out["norm2"].append(int(((L2 < t) & (U2 >= t)).sum())); out["norm1"].append(int(u1.sum()))
                out["norm1_ann"].append(int((u1 & (La < t) & (Ua >= t)).sum())); out["ball"].append(int(cnt[ub].sum()))
                out["ball_norm1"].append(int((u1 & ub[cid]).sum()))
            rec["counts"] = out; recs.append(rec)
        return recs, viol
    global _WORK
    _WORK = do_batch; viol = 0
    with mp.get_context("fork").Pool(NPROC) as pool:
        for k, (recs, v) in enumerate(pool.imap(_call, range(0, nq, BQ))):
            res["per_query"] += recs; viol += v
            if k % 8 == 7: log(f"level {L}: queries {min(nq, (k + 1) * BQ)}/{nq}, violations {viol}")
    res["violations"] = viol
    return res

allres = {"N": N, "n": n, "edges": EDGES, "fractions": FR, "levels": {}}
for L in LEVELS:
    allres["levels"][L] = run_level(L)
    json.dump(allres, open(OUT, "w"))
log("done")
