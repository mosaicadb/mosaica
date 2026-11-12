# E1/E2: how the support table's skipping depends on the number of anchors (the 1e9 design keeps 1,256 anchors for 1e6 clusters),
# and the oracle (query itself as an anchor = exact cluster extremes) for item and text queries.
import numpy as np, sys, time
sys.path.insert(0, "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined")
from core import kmeans, unit
ROOT = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
I = np.load(ROOT + "combined/idx_m768/index.npz"); Xs = np.load(ROOT + "combined/idx_m768/Xs.npy", mmap_mode="r")
N, n, K = int(I["N"]), int(I["n"]), int(I["K"]); cstart, Mq, mn, ahat = I["cstart"], I["Mq"], I["mn"], I["ahat"]
Q, qcls, thr, P, nimg = I["Q"], I["qcls"], I["thr"], I["P"], int(I["nimg"])
rng = np.random.default_rng(7)
sel = np.r_[np.where(qcls == "item")[0][:100], np.where(qcls == "text")[0][::2][:150]]
Q, qcls, thr = Q[sel], qcls[sel], thr[sel]; NQ = len(Q); NT = 8
Mh = Mq / mn[:, None]
Ttr = unit(np.load(ROOT + "agent2/text_train.npy"))
sets = {"I1000": Mh, "T256": P[nimg:] / np.linalg.norm(P[nimg:], axis=1, keepdims=True), "T2206": Ttr}
for A in (250, 64, 16, 4): sets[f"I{A}"] = kmeans(Mh.copy(), A, rng, 20)
for A in (64, 16): sets[f"T{A}"] = kmeans(Ttr.copy(), A, rng, 20)
names = list(sets); off = np.r_[0, np.cumsum([len(sets[k]) for k in names])]; PA = np.concatenate([sets[k] for k in names]).astype(np.float32)
sl = {k: slice(off[i], off[i + 1]) for i, k in enumerate(names)}
CONF = {"I1000+T256 (as built)": ["I1000", "T256"], "I250+T256": ["I250", "T256"], "I64+T256": ["I64", "T256"], "I16+T256": ["I16", "T256"],
        "I4+T256": ["I4", "T256"], "I1000 only": ["I1000"], "I1000+T64": ["I1000", "T64"], "I1000+T16": ["I1000", "T16"],
        "I1000+T2206": ["I1000", "T2206"], "I16+T16": ["I16", "T16"], "I16+T2206": ["I16", "T2206"]}
QP = Q @ PA.T
ANC = {}
for c, parts in CONF.items():
    ids = np.concatenate([np.arange(sl[p].start, sl[p].stop) for p in parts]); j = ids[QP[:, ids].argmax(1)]
    beta = QP[np.arange(NQ), j]; rho = np.sqrt(np.maximum(1 - beta ** 2, 0)); ANC[c] = (j, beta, rho, (PA[j] @ Mh.T))
    print(c, "anchors", len(ids), "mean rho item/text: %.3f / %.3f" % (rho[qcls == "item"].mean(), rho[qcls == "text"].mean()), flush=True)
qm = Q @ Mq.T; DA = 0.5 / 32767 + 1e-7
tracks = ["C1 only"] + list(CONF) + ["oracle extremes (self anchor)"]
touched = {t: np.zeros((NT, NQ)) for t in tracks}; opened = {t: np.zeros((NT, NQ)) for t in tracks}; inopen = {t: np.zeros((NT, NQ)) for t in tracks}
viol = 0; t0 = time.time()
for c in range(K):
    lo_, hi_ = int(cstart[c]), int(cstart[c + 1]); nc = hi_ - lo_
    if nc == 0: continue
    x = unit(Xs[lo_:hi_]); true = x @ Q.T; PX = x @ PA.T
    HIc = np.ceil(PX.max(0).astype(np.float64) * 32768) / 32768; LOc = np.floor(PX.min(0).astype(np.float64) * 32768) / 32768
    ah = ahat[lo_:hi_].astype(np.float32) / 32767; m_ = mn[c]
    alo = np.maximum(ah - DA, -1)[:, None]; ahi = np.minimum(ah + DA, 1)[:, None]
    cm = np.clip(qm[:, c] / m_, -1, 1); s1 = np.sqrt(np.maximum(1 - cm * cm, 0))
    aU = np.clip(cm[None], alo, ahi); aL = np.clip(-cm[None], alo, ahi)
    ub1 = cm * aU + s1 * np.sqrt(1 - aU * aU) + 3e-5; lb1 = cm * aL - s1 * np.sqrt(1 - aL * aL) - 3e-5
    def acc(name, lb, ub, lbc, ubc):
        global viol
        viol += int(((true > ub) | (true < lb)).sum())
        for ti in range(NT):
            t = thr[:, ti]; al = (lb < t) & (ub >= t); touched[name][ti] += al.sum(0)
            op = (ubc >= t) & (lbc < t); opened[name][ti] += op; inopen[name][ti] += op * nc
    acc("C1 only", lb1, ub1, np.full(NQ, -np.inf), np.full(NQ, np.inf))
    for k, (j, beta, rho, pm) in ANC.items():
        hic, loc = HIc[j], LOc[j]; Bh = np.where(beta >= 0, beta * hic, beta * loc); Bl = np.where(beta >= 0, beta * loc, beta * hic)
        rm = cm - beta * pm[:, c]; rp = np.sqrt(np.maximum(rho * rho - rm * rm, 0)); rr = np.maximum(rho, 1e-12)
        aU2 = np.clip((rm / rr)[None], alo, ahi); aL2 = np.clip((-rm / rr)[None], alo, ahi)
        ub2 = Bh + rm * aU2 + rp * np.sqrt(1 - aU2 * aU2) + 3e-5; lb2 = Bl + rm * aL2 - rp * np.sqrt(1 - aL2 * aL2) - 3e-5
        acc(k, np.maximum(lb1, lb2), np.minimum(ub1, ub2), Bl - rho - 3e-5, Bh + rho + 3e-5)
    tmx = true.max(0) + 3e-5; tmn = true.min(0) - 3e-5
    acc("oracle extremes (self anchor)", np.maximum(lb1, tmn[None]), np.minimum(ub1, tmx[None]), tmn, tmx)
    if c % 100 == 0: print(c, round(time.time() - t0), "viol", viol, flush=True)
print("violations", viol)
TH = ["0.01%", "0.1%", "1%", "5%", "0.95", "0.9", "0.8", "0.7"]
for cl in ("item", "text"):
    m = qcls == cl; nts = range(8) if cl == "item" else range(4)
    print(f"\n== {cl} queries ({m.sum()}): items touched % (undecided after range+table) | clusters opened % | items in opened clusters %   thresholds", [TH[t] for t in nts])
    for t in tracks:
        print(f"  {t:30s}", " ".join(f"{100 * touched[t][ti][m].mean() / N:5.1f}" for ti in nts), "|", " ".join(f"{100 * opened[t][ti][m].mean() / K:5.1f}" for ti in nts),
              "|", " ".join(f"{100 * inopen[t][ti][m].mean() / N:5.1f}" for ti in nts))
