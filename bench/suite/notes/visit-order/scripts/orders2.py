# Probe-order comparison on one fit (rust/tools/fit.py, merged defaults) and one query set: recall@10 against the share
# of rows scanned for
#   O0 int8 m + f rad with Approx::fit's f on corpus pseudo-queries (in the built index's row order, row_ent_index.bin)
#   rad grid f in {0, .1, .2, .35, .5, .75} (the oracle picks the best per dataset; f = 0 is O1, q.m alone)
#   q.m_hat (normalised means)
#   O2  q.m + f' sigma(q) rad / sigma(rad), sigma(q) = std over clusters of q.m (and a MAD variant: 1.4826 MAD for both)
#   O3  q.m + mult * sqrt(q' Sigma_c q) * z(k, n_c), Sigma_c = top-r PCA + isotropic rest (r 4, 16) or exact; k = 10
#   faiss IVF (IndexFlatIP quantizer, IP, trained on 200,000 rows with default_rng(1), as faiss_sys.py) at nlist = K
# The scores use q rounded to f16 and the int8 centres, as v2::q (prep2, centres64).
# usage: python -I orders2.py FITDIR CORPUS.npy QUERIES.npy GT.npy OUT.json NAME [--no-faiss]
import sys, os, json, time
import numpy as np
from scipy.stats import norm

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common_r import curve, summarise, fit_objective, FGRID  # noqa: E402

fitdir, corpus, qpath, gpath, outp, name = sys.argv[1:7]
do_faiss = "--no-faiss" not in sys.argv
t0 = time.time()
log = lambda *a: print(f"[{name} {time.time() - t0:5.0f}s]", *a, flush=True)
AVG_SHARES = [0.005, 0.008, 0.01, 0.016, 0.02, 0.03, 0.04, 0.063, 0.08, 0.1, 0.125, 0.15, 0.2, 0.25]
SHARES = sorted(set(AVG_SHARES + [0.3]))
F2 = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0]
MULT = [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0]
KNN = 10

meta = json.load(open(os.path.join(fitdir, "meta.json")))
cstart = np.load(os.path.join(fitdir, "cstart.npy"))
perm = np.load(os.path.join(fitdir, "perm.npy"))
mq8 = np.load(os.path.join(fitdir, "mq8.npy"))
msc = np.load(os.path.join(fitdir, "msc.npy"))
Xs = np.load(os.path.join(fitdir, "Xs.npy"), mmap_mode="r")
N, n = Xs.shape
n0 = meta["n_orig"]
K = len(cstart) - 1
sizes = np.diff(cstart)
row_cl = np.repeat(np.arange(K), sizes)
ent_row = np.empty(N, np.int64)
ent_row[perm] = np.arange(N)
Mq = (mq8.astype(np.float64) * msc[:, None]).astype(np.float32).astype(np.float64)
f16 = lambda q: np.asarray(q, np.float32).astype(np.float16).astype(np.float64)
B = 100_000

# ---- per-cluster statistics (cached per fit): mean, rad (about the int8 centre), PCA of the covariance about the mean
cache = os.path.join(fitdir, "cluster_stats.npz")
if os.path.exists(cache):
    z = np.load(cache)
    Mean, rad_q, lam, U, tr = z["Mean"], z["rad_q"], z["lam"], z["U"], z["tr"]
else:
    Mean = np.zeros((K, n)); rad_q = np.zeros(K); lam = np.zeros((K, 16)); U = np.zeros((K, n, 16), np.float32)
    tr = np.zeros(K)
    for c in range(K):
        x = np.asarray(Xs[cstart[c]:cstart[c + 1]], np.float32)
        m = x.astype(np.float64).mean(0)
        Mean[c] = m
        rad_q[c] = np.sqrt(((x.astype(np.float64) - Mq[c]) ** 2).sum(1).mean())
        xc = x - m.astype(np.float32)
        C = (xc.T.astype(np.float64) @ xc.astype(np.float64)) / len(x)
        tr[c] = np.trace(C)
        w, v = np.linalg.eigh(C)
        lam[c] = w[::-1][:16]
        U[c] = v[:, ::-1][:, :16]
    np.savez(cache, Mean=Mean, rad_q=rad_q, lam=lam, U=U, tr=tr)
log(f"K {K}, N {N}, n {n} (n0 {n0}), sizes {sizes.min()}..{sizes.max()}; cluster stats in {time.time() - t0:.0f} s")

# ---- O0's f: Approx::fit on pseudo-queries in the built index's row order (cached per fit)
pcache = os.path.join(fitdir, "pseudo_fit.json")
if os.path.exists(pcache):
    pf = json.load(open(pcache))
else:
    row_ent_ix = np.fromfile(os.path.join(fitdir, "row_ent_index.bin"), np.uint32)
    nq = 1000
    r = np.arange(N, dtype=np.uint64)
    with np.errstate(over="ignore"):
        h = ((r ^ np.uint64(0x5BD1E995)) * np.uint64(0x9E3779B97F4A7C15)) % np.uint64(N) * np.uint64(2) < np.uint64(3 * nq)
    xr = ent_row[row_ent_ix[np.where(h)[0]]]
    P = np.asarray(Xs[np.sort(xr)], np.float32)[np.argsort(np.argsort(xr))]
    pn = np.linalg.norm(P.astype(np.float64), axis=1)
    keep = pn >= 0.5
    xr, P, pn = xr[keep][:nq], P[keep][:nq], pn[keep][:nq]
    P16 = f16((P / pn[:, None]).astype(np.float32))
    kk = KNN + 1
    bs, bi = np.full((len(P), 0), -np.inf, np.float32), np.zeros((len(P), 0), np.int64)
    P32 = P16.astype(np.float32)
    for i in range(0, N, B):
        S = P32 @ np.asarray(Xs[i:i + B], np.float32).T
        s = np.concatenate([bs, S], 1)
        ii = np.concatenate([bi, np.broadcast_to(np.arange(i, i + S.shape[1]), S.shape)], 1)
        top = np.argpartition(-s, kk - 1, axis=1)[:, :kk]
        bs, bi = np.take_along_axis(s, top, 1), np.take_along_axis(ii, top, 1)
    o = np.argsort(-bs, 1, kind="stable")
    bi, bsv = np.take_along_axis(bi, o, 1), np.take_along_axis(bs, o, 1)
    pgt = np.array([[x for x in b if x != me][:KNN] for b, me in zip(bi, xr)])
    f0, tab = fit_objective(lambda f: P16 @ Mq.T + f * rad_q[None, :], row_cl[pgt], sizes)
    pf = dict(f=f0, table=tab, n=len(xr), top1=float(bsv[:, 1].mean()), top10=float(bsv[:, KNN].mean()))
    json.dump(pf, open(pcache, "w"), indent=1)
f0 = pf["f"]
log(f"O0 f (Approx::fit on pseudo-queries) {f0}: " + ", ".join(f"{t['f']}: {t['rows'] / N * 100:.2f}%" for t in pf["table"]))

# ---- queries and truth
Q0 = np.load(qpath).astype(np.float64)
Q = np.zeros((len(Q0), n)); Q[:, :Q0.shape[1]] = Q0
Qr = f16(Q)
gt = np.load(gpath)[:, :KNN].astype(np.int64)
gtcl = row_cl[ent_row[gt]]
nq = len(Q)

# exact q' Sigma_c q = E_c[(q.x)^2] - (q.mean_c)^2, and the 1st / 10th neighbour similarity, from one pass
E2 = np.zeros((nq, K))
Qr32 = Qr.astype(np.float32)
B2 = max(5000, int(2e7 // nq))
for i in range(0, N, B2):
    S = (Qr32 @ np.asarray(Xs[i:i + B2], np.float32).T).astype(np.float64)
    c = row_cl[i:i + B2]
    st = np.r_[0, np.where(np.diff(c))[0] + 1]
    E2[:, c[st]] += np.add.reduceat(S * S, st, axis=1)
qmean = Qr @ Mean.T
var_exact = np.maximum(E2 / sizes[None, :] - qmean ** 2, 0)
gts = np.array([Qr[j] @ np.asarray(Xs[np.sort(ent_row[gt[j]])], np.float64).T for j in range(nq)])
log(f"{nq} queries; 1st / 10th neighbour similarity {gts.max(1).mean():.3f} / {gts.min(1).mean():.3f} "
    f"(pseudo-queries {pf['top1']:.3f} / {pf['top10']:.3f})")


def var_pca(r):
    Ur = np.ascontiguousarray(U[:, :, :r].transpose(1, 0, 2).reshape(n, K * r), dtype=np.float32)
    proj = (Qr.astype(np.float32) @ Ur).astype(np.float64).reshape(nq, K, r)
    head = (proj ** 2 * lam[None, :, :r]).sum(2)
    rest = np.maximum(tr - lam[:, :r].sum(1), 0) / max(n0 - r, 1)
    qq = (Qr ** 2).sum(1)
    return head + rest[None, :] * np.maximum(qq[:, None] - (proj ** 2).sum(2), 0)


zq = np.where(sizes > KNN, norm.ppf(1 - KNN / np.maximum(sizes, KNN + 1)), 0.0)
qm = Qr @ Mq.T
sd_q = qm.std(1, keepdims=True)
mad = lambda a, ax=None: 1.4826 * np.median(np.abs(a - np.median(a, axis=ax, keepdims=True)), axis=ax, keepdims=True)
mad_q = mad(qm, 1)
rad_n = rad_q / rad_q.std()
rad_m = rad_q / mad(rad_q)
Mhat = Mean / np.linalg.norm(Mean, axis=1, keepdims=True)

res = dict(name=name, K=int(K), N=int(N), n0=n0, nq=nq, f0=f0, pseudo=pf, sim_top1=float(gts.max(1).mean()),
           sim_top10=float(gts.min(1).mean()), sd_q_mean=float(sd_q.mean()), mad_q_mean=float(mad_q.mean()),
           rad_sd=float(rad_q.std()), rad_mad=float(mad(rad_q).item()), orders={})


def add(key, score):
    rec, items = curve(score, gtcl, sizes)
    s = summarise(rec, items, N, shares=SHARES)
    s["avg"] = float(np.mean([s["fixed"][f"{x:g}"] for x in AVG_SHARES]))
    res["orders"][key] = s


for f in FGRID:
    add(f"rad f={f}", qm + f * rad_q[None, :])
add("m_hat", Qr @ Mhat.T)
for f in F2:
    add(f"O2 sd f'={f}", qm + f * sd_q * rad_n[None, :])
    add(f"O2 mad f'={f}", qm + f * mad_q * rad_m[None, :])
vs = {"r4": var_pca(4), "r16": var_pca(16), "exact": var_exact}
for k, v in vs.items():
    sq = np.sqrt(v) * zq[None, :]
    for m in MULT:
        add(f"O3 {k} x{m}", qm + m * sq)
    res[f"O3_{k}_bonus_sd"] = float(sq.std(1).mean())
    if k != "exact":
        for m in (0.5, 0.75, 1.0):
            for f in (0.1, 0.2, 0.3):
                add(f"H {k} x{m} f'={f}", qm + m * sq + f * sd_q * rad_n[None, :])
res["orders"]["O0"] = res["orders"][f"rad f={f0}"]
log("O0 " + " ".join(f"{k}:{v:.3f}" for k, v in res["orders"]["O0"]["fixed"].items()) + f" avg {res['orders']['O0']['avg']:.3f}")

if do_faiss:
    import faiss
    X = np.load(corpus, mmap_mode="r")
    d = X.shape[1]
    rng = np.random.default_rng(1)
    trn = np.ascontiguousarray(X[np.sort(rng.choice(len(X), min(len(X), 200_000), replace=False))], dtype=np.float32)
    qz = faiss.IndexFlatIP(d)
    ix = faiss.IndexIVFFlat(qz, d, K, faiss.METRIC_INNER_PRODUCT)
    ix.train(trn)
    Cf = qz.reconstruct_n(0, K)
    a = np.concatenate([qz.search(np.ascontiguousarray(X[i:i + B], dtype=np.float32), 1)[1][:, 0] for i in range(0, len(X), B)])
    fs = np.bincount(a, minlength=K)
    rec, items = curve(Q0.astype(np.float32).astype(np.float64) @ Cf.T.astype(np.float64), a[gt], fs)
    s = summarise(rec, items, N, shares=SHARES)
    s["avg"] = float(np.mean([s["fixed"][f"{x:g}"] for x in AVG_SHARES]))
    res["orders"][f"faiss nlist={K}"] = s
    log(f"faiss nlist {K}: " + " ".join(f"{k}:{v:.3f}" for k, v in s["fixed"].items()) + f" avg {s['avg']:.3f}")

best = sorted(res["orders"].items(), key=lambda kv: -kv[1]["avg"])[:8]
for k, v in best:
    log(f"  {k:22s} avg {v['avg']:.3f}  " + " ".join(f"{x}:{v['fixed'][x]:.3f}" for x in ("0.005", "0.016", "0.063", "0.25")))
json.dump(res, open(outp, "w"), indent=1)
log("saved", outp)
