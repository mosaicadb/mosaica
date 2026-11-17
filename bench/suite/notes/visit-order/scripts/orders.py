# The probe-order replay on one fit (rust/tools/fit.py's output): recall@10 against rows scanned for the 100 test
# text queries under each cluster order, plus Approx::fit's choice of f on corpus pseudo-queries (as the bindings'
# Index::new does) and on 1,000 learn text queries (as fit_queries would).
# usage: python -I orders.py FITDIR PREPDIR LEARNDIR OUT.json
import sys, os, json, time
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common_r import curve, summarise, fit_objective, FGRID  # noqa: E402

fitdir, prep, learndir, outp = sys.argv[1:5]
t0 = time.time()
log = lambda *a: print(f"[{time.time() - t0:5.0f}s]", *a, flush=True)
meta = json.load(open(os.path.join(fitdir, "meta.json")))
cstart = np.load(os.path.join(fitdir, "cstart.npy"))
perm = np.load(os.path.join(fitdir, "perm.npy"))  # row -> entity
mq8 = np.load(os.path.join(fitdir, "mq8.npy"))
msc = np.load(os.path.join(fitdir, "msc.npy"))
Xs = np.load(os.path.join(fitdir, "Xs.npy"), mmap_mode="r")
N, n = Xs.shape
K = len(cstart) - 1
sizes = np.diff(cstart)
row_cl = np.repeat(np.arange(K), sizes)
ent_row = np.empty(N, np.int64)
ent_row[perm] = np.arange(N)
Mq = (mq8.astype(np.float64) * msc[:, None]).astype(np.float32).astype(np.float64)  # centres64 in Rust
log(f"K {K}, N {N}, n {n}, sizes {sizes.min()}..{sizes.max()} (median {np.median(sizes):.0f})")

# per-cluster sums over the stored f16 rows (the vectors Rust reads): sum x, sum |x|^2
Msum = np.zeros((K, n))
sq = np.zeros(K)
B = 100_000
for i in range(0, N, B):
    x = np.asarray(Xs[i:i + B], np.float64)
    c = row_cl[i:i + B]
    # reduceat over contiguous cluster runs (rows are in cluster order)
    starts = np.r_[0, np.where(np.diff(c))[0] + 1]
    Msum[c[starts]] += np.add.reduceat(x, starts, axis=0)
    sq[c[starts]] += np.add.reduceat((x * x).sum(1), starts)
Mean = Msum / sizes[:, None]
rad_q = np.sqrt(np.maximum(sq - 2 * (Msum * Mq).sum(1) + sizes * (Mq * Mq).sum(1), 0) / sizes)  # Approx::fit's rad
rad_f = np.sqrt(np.maximum(sq - sizes * (Mean * Mean).sum(1), 0) / sizes)
chk = np.mean(np.clip(np.round(Mean / msc[:, None]), -127, 127).astype(np.int8) == mq8)
Mhat = Mean / np.linalg.norm(Mean, axis=1, keepdims=True)
log(f"means: int8 agreement {chk:.5f}; |m| {np.linalg.norm(Mean, axis=1).mean():.3f} "
    f"({np.linalg.norm(Mean, axis=1).min():.3f}..{np.linalg.norm(Mean, axis=1).max():.3f}); "
    f"rad {rad_q.mean():.3f} ({rad_q.min():.3f}..{rad_q.max():.3f}); int8 rounding |m - mq| {np.linalg.norm(Mean - Mq, axis=1).mean():.4f}")

# the k-means centres (recorded by fitwrap.py), mapped to the fit's cluster numbering through the rows' argmax
C = None
if os.path.exists(os.path.join(fitdir, "kmeans_C.npy")):
    C0 = np.load(os.path.join(fitdir, "kmeans_C.npy")).astype(np.float32)
    a0 = np.concatenate([(np.asarray(Xs[i:i + 50_000], np.float32) @ C0.T).argmax(1) for i in range(0, N, 50_000)])
    cmap = np.array([np.bincount(a0[cstart[j]:cstart[j + 1]], minlength=len(C0)).argmax() for j in range(K)])
    agree = float(np.mean(a0 == cmap[row_cl]))
    C = C0[cmap].astype(np.float64)
    log(f"k-means centres: {len(C0)} -> {K}; rows whose argmax centre is their cluster's: {agree:.6f}; "
        f"distinct {len(np.unique(cmap))}")

f16 = lambda q: np.asarray(q, np.float32).astype(np.float16).astype(np.float64)  # prep2's q64 (query_f16)

# test queries and truth (entity ids -> rows -> clusters)
Q = np.load(os.path.join(prep, "queries.npy"))
Qr = f16(Q.astype(np.float32))
gt = np.load(os.path.join(prep, "order.npy"))[:, :10]
gtcl = row_cl[ent_row[gt]]

# pseudo-queries exactly as v2::q::pseudo_queries(ix, 1000, 10): rows picked by the hash, in row order, truncated
nq = 1000
r = np.arange(N, dtype=np.uint64)
with np.errstate(over="ignore"):
    h = ((r ^ np.uint64(0x5BD1E995)) * np.uint64(0x9E3779B97F4A7C15)) % np.uint64(N) * np.uint64(2) < np.uint64(3 * nq)
hrows = np.where(h)[0]
ixp = os.path.join(fitdir, "row_ent_index.bin")  # the built index's row -> entity (encoded/row_ent.bin), if extracted
row_orders = {"fit (Xs) row order": hrows}
if os.path.exists(ixp):
    row_ent_ix = np.fromfile(ixp, np.uint32)
    row_orders = {"index row order": ent_row[row_ent_ix[hrows]], **row_orders}


def pseudo(xr):
    P = np.asarray(Xs[np.sort(xr)], np.float32)[np.argsort(np.argsort(xr))]
    pn = np.linalg.norm(P.astype(np.float64), axis=1)
    keep = pn >= 0.5
    xr, P, pn = xr[keep][:nq], P[keep][:nq], pn[keep][:nq]
    P16 = f16((P / pn[:, None]).astype(np.float32))
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
    pgt = np.array([[x for x in b if x != me][:10] for b, me in zip(bi, xr)])  # own row left out
    log(f"pseudo-queries: {len(xr)} kept; clusters {row_cl[xr].min()}..{row_cl[xr].max()} of {K} "
        f"({len(np.unique(row_cl[xr]))} distinct); mean 1st-neighbour sim {bsv[:, 1].mean():.3f}, 10th {bsv[:, 10].mean():.3f}")
    return P16, row_cl[pgt], bsv


kk = 11
pseudos = {name: pseudo(xr) for name, xr in row_orders.items()}
pname = next(iter(pseudos))
P16, pgtcl, bsv = pseudos[pname]

# learn text queries (fit_queries)
LQ = np.load(os.path.join(learndir, "learn_q.npy"))
Lgt = np.load(os.path.join(learndir, "learn_gt.npy"))[:, :10]
LQr = f16(LQ)
Lgtcl = row_cl[ent_row[Lgt]]

res = dict(fit=fitdir, K=int(K), N=int(N), sizes=dict(min=int(sizes.min()), max=int(sizes.max()), median=float(np.median(sizes))),
           int8_agreement=float(chk), mean_norm=float(np.linalg.norm(Mean, axis=1).mean()), rad_mean=float(rad_q.mean()),
           rad_range=[float(rad_q.min()), float(rad_q.max())], orders={})

# Approx::fit on pseudo-queries and on learn queries (each over the grid), and on the test queries (oracle, for reference)
key_q = lambda QQ: (lambda f: QQ @ Mq.T + f * rad_q[None, :])
f_pseudo, tab_pseudo = fit_objective(key_q(P16), pgtcl, sizes)
alt = {name: fit_objective(key_q(v[0]), v[1], sizes) for name, v in pseudos.items()}
for name, (fa, ta) in alt.items():
    log(f"pseudo-queries in {name}: f {fa}; " + ", ".join(f"{t['f']}: {t['rows'] / N * 100:.2f}%" for t in ta))
f_learn, tab_learn = fit_objective(key_q(LQr), Lgtcl, sizes)
f_test, tab_test = fit_objective(key_q(Qr), gtcl, sizes)
res["fit_f"] = dict(pseudo_rows=pname, pseudo_variants={k: dict(f=v[0], table=v[1]) for k, v in alt.items()}, pseudo=f_pseudo, learn=f_learn, test_oracle=f_test,
                    tables=dict(pseudo=tab_pseudo, learn=tab_learn, test=tab_test))
log(f"Approx::fit f: pseudo-queries {f_pseudo}, learn queries {f_learn}, test queries (oracle) {f_test}")
for name, tab in (("pseudo", tab_pseudo), ("learn", tab_learn), ("test", tab_test)):
    log(f"  {name}: " + ", ".join(f"f {t['f']}: p95 {t['p95']} rows {t['rows'] / N * 100:.2f}%" for t in tab))

# similarity scale: test queries vs pseudo-queries (top-1 and top-10 similarity to the corpus)
tr = ent_row[gt]
ts = np.array([Q[i] @ np.asarray(Xs[np.sort(tr[i])], np.float64).T for i in range(len(Q))])
lr = ent_row[Lgt]
ls = np.array([LQ[i] @ np.asarray(Xs[np.sort(lr[i])], np.float64).T for i in range(len(LQ))])
res["sims"] = dict(pseudo_top1=float(bsv[:, 1].mean()), pseudo_top10=float(bsv[:, 10].mean()),
                   test_top1=float(ts.max(1).mean()), test_top10=float(ts.min(1).mean()),
                   learn_top1=float(ls.max(1).mean()), learn_top10=float(ls.min(1).mean()))
log("similarity of the 1st / 10th neighbour:", {k: round(v, 3) for k, v in res["sims"].items()})

orders = {}
for f in FGRID:
    orders[f"int8 m + {f} rad"] = Qr @ Mq.T + f * rad_q[None, :]
for f in FGRID:
    orders[f"f32 m + {f} rad"] = Qr @ Mean.T + f * rad_f[None, :]
orders["m_hat (normalised mean)"] = Qr @ Mhat.T
if C is not None:
    orders["C (k-means centre)"] = Qr @ C.T
for name, sc in orders.items():
    rec, items = curve(sc, gtcl, sizes)
    res["orders"][name] = summarise(rec, items, N)
# per-query recall at ~1.6% under the fitted f and under the best order, for the zero-recall comparison
res["fitted_f_order"] = f"int8 m + {f_pseudo} rad"
res["learn_f_order"] = f"int8 m + {f_learn} rad"
for name in [res["fitted_f_order"], "m_hat (normalised mean)"] + (["C (k-means centre)"] if C is not None else []):
    s = res["orders"][name]
    log(f"{name:28s} " + " ".join(f"{k}:{v:.3f}" for k, v in s["fixed"].items()) +
        f" | nprobe {', '.join(f'{p}: {v['recall']:.3f} ({v['share'] * 100:.1f}%)' for p, v in s['nprobe'].items())}"
        f" | zero@{s['zero16']['share'] * 100:.2f}%: {s['zero16']['n_zero']}")
json.dump(res, open(outp, "w"), indent=1)
log("saved", outp)
