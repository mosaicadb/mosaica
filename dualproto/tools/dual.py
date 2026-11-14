# Export the dual pair of transforms (geometric-mean metric) for the Rust v2 prototype.
# usage: python dual.py MODE OUTDIR      MODE: item (Sigma_Q from corpus items) | mix (mean of normalised item and text)
# Writes into OUTDIR (a copy of the baseline export, files symlinked): V.npy = query-side basis VQ (columns), lam.npy =
# per-coordinate mean square of the item-side coordinates z = VQ8^-1 r under the int8 basis the Rust build will use,
# r2.npy = mean |r|^2 per group, and profiles prof_qi / prof_h0.25 / prof_qt / prof_qmix in the query-side coordinates.
import numpy as np, sys, os, json, time
MODE, OUT = sys.argv[1], sys.argv[2]
P = "/tmp/claude-0/-home-user-mosaica/8c244050-1af9-5b77-a4e8-97638e77c857/scratchpad/vc/proto/"
VC = "/tmp/claude-0/-home-user-mosaica/8c244050-1af9-5b77-a4e8-97638e77c857/scratchpad/vc/"
BASE = P + "rustdata/m768/"
RIDGE = float(os.environ.get("RIDGE", "0.1"))
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
def unit(a):
    a = np.asarray(a, dtype=np.float32); return a / np.linalg.norm(a, axis=1, keepdims=True)

I = np.load(P + "idx_m768/index.npz")
Xs = np.load(P + "idx_m768/Xs.npy", mmap_mode="r")
N, n, K, G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"])
Mq = I["Mq"].astype(np.float32); mn = I["mn"]; Mh = (Mq / mn[:, None]).astype(np.float32)
assign, cstart, grp, qi = I["assign"], I["cstart"], I["grp"], I["qi"]
lamP, VP = I["lam"], I["V"]
cnt = np.diff(cstart); pc = cnt / cnt.sum()
gstart = np.searchsorted(grp[assign], np.arange(G + 1))
rng = np.random.default_rng(3)
pool = np.setdiff1d(np.arange(N), qi)

# ---- Sigma_Q: offsets q - (q.m^) m^ from clusters drawn in proportion to their size, 40 per query ----
def cov(Q):
    S = np.zeros((n, n)); c = 0
    for i in range(0, len(Q), 200):
        q = Q[i:i+200]; ks = rng.choice(K, (len(q), 40), p=pc); C = Mh[ks]                 # b x 40 x n
        D = (q[:, None, :] - np.einsum("bkn,bn->bk", C, q)[:, :, None] * C).reshape(-1, n).astype(np.float64)
        S += D.T @ D; c += len(D)
    return S / c
items = unit(Xs[np.sort(rng.choice(pool, 3000, replace=False))])
Si = cov(items)
if MODE == "item": SQ = Si
elif MODE == "mix":
    St = cov(unit(np.load(VC + "text_train.npy"))); SQ = 0.5 * (Si / np.trace(Si) + St / np.trace(St))
else: raise SystemExit("MODE is item or mix")
SQ = SQ + RIDGE * np.trace(SQ) / n * np.eye(n)
lq, vq_ = np.linalg.eigh(SQ); A = (vq_ * np.sqrt(lq)) @ vq_.T; Ai = (vq_ / np.sqrt(lq)) @ vq_.T
log("Sigma_Q", MODE, "largest eigenvalue share", float(lq.max() / lq.sum()), "condition", float(lq.max() / lq.min()))

# ---- per group: the pair, its int8 query-side basis, the item-side coordinates' mean squares ----
Vout = np.zeros((G, n, n), np.float32); lam = np.zeros((G, n)); r2 = np.zeros(G); gam_all = np.zeros((G, n)); cond = np.zeros(G)
VQ8 = []
for g in range(G):
    SR = (VP[g].astype(np.float64) * lamP[g]) @ VP[g].T.astype(np.float64)
    gam, W = np.linalg.eigh(A @ SR @ A); gam = gam[::-1]; W = W[:, ::-1]; gam = np.maximum(gam, 1e-12 * gam[0])
    VQ = (Ai @ W) * gam ** 0.25; VD = (A @ W) * gam ** -0.25
    assert np.abs(VQ.T @ VD - np.eye(n)).max() < 1e-6
    V32 = VQ.astype(np.float32); Vout[g] = V32; gam_all[g] = gam
    vsc = np.abs(V32.astype(np.float64)).max(0) / 127.0
    v8 = np.clip(np.round(V32 / vsc), -127, 127); vq = v8 * vsc; VQ8.append(vq)
    Xinv = np.linalg.inv(vq); sv = np.linalg.svd(vq, compute_uv=False); cond[g] = sv[0] / sv[-1]
    s, e = gstart[g], gstart[g + 1]
    R = np.asarray(Xs[s:e], dtype=np.float32) - Mq[assign[s:e]]
    Z = R.astype(np.float64) @ Xinv.T
    lam[g] = np.maximum((Z * Z).mean(0), 1e-30); r2[g] = (R.astype(np.float64) ** 2).sum(1).mean()
    if g % 16 == 0:
        log("group", g, "items", e - s, "cond(VQ8)", round(cond[g], 1), "sum mean-square z / sum sqrt gamma", float(lam[g].sum() / np.sqrt(gam).sum()),
            "max ratio z ms / sqrt gamma", float((lam[g] / np.sqrt(gam)).max()), "r2", r2[g])
log("bases done; largest condition number", cond.max())

# ---- profiles in the query-side coordinates (as build.py computes Qi and Qt) ----
pq = unit(Xs[rng.choice(pool, 5000, replace=False)]); pk = np.argsort(-(pq @ Mq.T), axis=1)[:, :4]
Ttr = unit(np.load(VC + "text_train.npy")); tk = np.argsort(-(Ttr @ Mq.T), axis=1)[:, :50]
Qi = np.zeros((G, n)); Qt = np.zeros((G, n)); QiP = np.zeros((G, n)); QtP = np.zeros((G, n))
for g in range(G):
    rows, cols = np.where(grp[pk] == g)
    D = pq[rows] - Mq[pk[rows, cols]]
    Qi[g] = ((D @ VQ8[g]) ** 2).mean(0) if len(rows) > 50 else lam[g]
    QiP[g] = ((D @ VP[g]) ** 2).mean(0) if len(rows) > 50 else lamP[g]
    rows, cols = np.where(grp[tk] == g)
    if len(rows) < 200:
        ks = np.where(grp == g)[0]; rows = np.repeat(np.arange(len(Ttr)), 2); kk = ks[np.arange(len(rows)) % len(ks)]
    else: kk = tk[rows, cols]
    D = Ttr[rows] - Mq[kk]
    Qt[g] = ((D @ VQ8[g]) ** 2).mean(0); QtP[g] = ((D @ VP[g]) ** 2).mean(0)
iso = np.repeat(Qi.sum(1, keepdims=True) / n, n, 1); H25 = (0.75 * np.sqrt(Qi) + 0.25 * np.sqrt(iso)) ** 2
Qmix = ((np.sqrt(Qi) + np.sqrt(Qt)) / 2) ** 2
w = np.diff(gstart) / N
def slack(lm, q): return float((np.sqrt(lm * q).sum(1) * w).sum())
log("zero-bit Cauchy-Schwarz slack E sum sqrt(lam q), item offsets: PCA", slack(lamP, QiP), "dual", slack(lam, Qi),
    "| text offsets: PCA", slack(lamP, QtP), "dual", slack(lam, Qt))

os.makedirs(OUT, exist_ok=True)
for f in os.listdir(BASE):
    if f in ("V.npy", "lam.npy") or f.startswith("prof_"): continue
    d = os.path.join(OUT, f)
    if not os.path.exists(d): os.symlink(BASE + f, d)
np.save(OUT + "/V.npy", Vout); np.save(OUT + "/lam.npy", lam); np.save(OUT + "/r2.npy", r2); np.save(OUT + "/gamma.npy", gam_all)
for k, v in (("qi", Qi), ("h0.25", H25), ("qt", Qt), ("qmix", Qmix)): np.save(OUT + f"/prof_{k}.npy", np.ascontiguousarray(v))
json.dump(dict(mode=MODE, ridge=RIDGE, cond_max=float(cond.max()), slack=dict(item_pca=slack(lamP, QiP), item_dual=slack(lam, Qi), text_pca=slack(lamP, QtP), text_dual=slack(lam, Qt))),
          open(OUT + "/dual.json", "w"), indent=1)
log("written", OUT)
