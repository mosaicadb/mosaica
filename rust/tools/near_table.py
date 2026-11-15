# Step 5, part A: can a sparse support table replace the K x K one at no query cost?
# On a fit.py directory (Xs.npy in cluster order, cstart, int8 centres), counts the items each query touches (items
# whose interval still straddles the threshold after the centre bound, the anchor bound and the stored key) and the
# clusters it opens, for the full table (one anchor per cluster, every cluster's extremes against every anchor, as
# build_table2) and for sparse variants:
#   near M: exact extremes of an anchor's M nearest clusters only (by centre direction);
#   far = cell: other clusters bounded by their cell's extremes against the anchor (anchors x cells table);
#   far = none: other clusters get no anchor bound (centre bound and key only);
#   A coarse anchors: every cluster's exact extremes against A anchors (k-means of the centres), a K x A table;
#   text anchors: the same against k-means of training prompts, added to an image set;
#   approx anchor: the query's anchor is the best among the clusters of its A nearest cells, not over all K.
# The bound arithmetic is review/e1_anchors.py's. Every bound is checked against the exact scores (violations must be 0).
# usage: python3 -I near_table.py FITDIR ITEMQ.npy TEXTQ.npy [--nq 200] [--cells 32] [--seed 7] [--text-train T.npy]
import argparse, json, os, time
import numpy as np

ap = argparse.ArgumentParser()
ap.add_argument("fit")
ap.add_argument("itemq")
ap.add_argument("textq")
ap.add_argument("--nq", type=int, default=200)
ap.add_argument("--cells", type=int, default=32)
ap.add_argument("--seed", type=int, default=7)
ap.add_argument("--text-train", default="", help="training prompts (npy) for text anchors")
a = ap.parse_args()
T0 = time.time()
log = lambda *s: print(f"[{time.time() - T0:5.0f}s]", *s, flush=True)
f = lambda x: os.path.join(a.fit, x)

meta = json.load(open(f("meta.json")))
Xs = np.load(f(meta.get("xs", "Xs.npy")), mmap_mode="r")
N, n = Xs.shape
cstart = np.load(f("cstart.npy")).astype(np.int64)
M8 = np.load(f("mq8.npy")).astype(np.float64) * np.load(f("msc.npy")).astype(np.float64)[:, None]
K = len(cstart) - 1
mn = np.linalg.norm(M8, axis=1)
Mh = M8 / mn[:, None]                      # anchors: one per cluster, the centre's direction (build_table2)


def unitq(path):
    q = np.load(path).astype(np.float32)[: a.nq]
    w = np.zeros((len(q), n), np.float32)
    w[:, : q.shape[1]] = q
    return w / np.linalg.norm(w, axis=1, keepdims=True)


Q = np.concatenate([unitq(a.itemq), unitq(a.textq)])
cls = np.array(["item"] * min(a.nq, len(np.load(a.itemq, mmap_mode="r"))) + ["text"] * (len(Q) - min(a.nq, len(np.load(a.itemq, mmap_mode="r")))))
NQ = len(Q)
log(f"N {N}, n {n}, K {K}; {NQ} queries ({(cls == 'item').sum()} item, {(cls == 'text').sum()} text)")

rng = np.random.default_rng(a.seed)


def kmeans(X, k, iters=25):
    c = X[rng.choice(len(X), k, replace=False)].copy()
    for _ in range(iters):
        lab = (X @ c.T).argmax(1)
        for i in range(k):
            if (lab == i).any():
                v = X[lab == i].sum(0)
                c[i] = v / np.linalg.norm(v)
    return c, (X @ c.T).argmax(1)


# anchor sets: one per cluster (as built), coarse image anchors (k-means on the centre directions; at 1B, A coarse
# anchors have the density of A anchors here, whatever K is), text anchors (k-means on training prompts)
SETS = {"I": Mh}
for A in (32, 128, 512):
    SETS[f"I{A}"] = kmeans(Mh, A)[0]
if a.text_train:
    T = np.load(a.text_train).astype(np.float64)
    T = np.pad(T, ((0, 0), (0, n - T.shape[1])))
    T /= np.linalg.norm(T, axis=1, keepdims=True)
    for A in (64, 256):
        SETS[f"T{A}"] = kmeans(T, A)[0]
names = list(SETS)
AOFF = dict(zip(names, np.r_[0, np.cumsum([len(SETS[k]) for k in names])][:-1]))
AN = np.concatenate([SETS[k] for k in names])
NA = len(AN)
log("anchor sets " + ", ".join(f"{k} {len(v)}" for k, v in SETS.items()))

# pass 1: exact scores, every anchor's extremes over every cluster, keys
S = np.empty((N, NQ), np.float32)
HI = np.empty((NA, K))                     # HI[anchor, cluster]
LO = np.empty((NA, K))
AH = np.empty(N, np.float64)
Mh32 = AN.astype(np.float32)
for c in range(K):
    r0, r1 = cstart[c], cstart[c + 1]
    if r1 == r0:
        HI[:, c], LO[:, c] = -np.inf, np.inf
        continue
    x = np.asarray(Xs[r0:r1], np.float32)
    x /= np.maximum(np.linalg.norm(x, axis=1, keepdims=True), 1e-30)
    S[r0:r1] = x @ Q.T
    P = (x @ Mh32.T).astype(np.float64)
    HI[:, c] = np.ceil((P.max(0) + 2e-6) * 32768) / 32768
    LO[:, c] = np.floor((P.min(0) - 2e-6) * 32768) / 32768
    AH[r0:r1] = np.round(P[:, c] * 32767) / 32767   # the stored key, x . m-hat as i16
log("scores, full table and keys done")

FR = [1e-4, 1e-3, 1e-2, 5e-2]
TH = [f"{100 * x:g}%" for x in FR] + ["t=0.95", "t=0.9"]
thr = np.empty((NQ, len(TH)))
for j in range(NQ):
    s = S[:, j]
    for i, fr in enumerate(FR):
        k = int(fr * N)
        thr[j, i] = np.partition(s, N - k)[N - k]
    thr[j, 4:] = [0.95, 0.9]

# cells: k-means on the centre directions (the partition's coarse cells, at scale)
C = a.cells
cen, cell = kmeans(Mh, C)
CHI = np.stack([HI[:, cell == i].max(1) if (cell == i).any() else np.full(NA, -np.inf) for i in range(C)], 1)
CLO = np.stack([LO[:, cell == i].min(1) if (cell == i).any() else np.full(NA, np.inf) for i in range(C)], 1)
sizes = np.bincount(cell, minlength=C)
log(f"{C} cells of {sizes.min()}..{sizes.max()} clusters")

# near lists: each anchor's M nearest clusters by centre direction (itself first)
order = np.argsort(-(Mh @ Mh.T), axis=1)
MS = [16, 64, 256]
near = {m: np.zeros((NA, K), bool) for m in MS}
for m in MS:
    near[m][np.arange(K)[:, None], order[:, :m]] = True


# the query's anchor: the best of the named sets (exact), or the best per-cluster anchor among the clusters of its
# A nearest cells
QA = Q.astype(np.float64) @ AN.T
def best(*sets):
    ok = np.zeros(NA, bool)
    for k in sets:
        ok[AOFF[k]:AOFF[k] + len(SETS[k])] = True
    return np.where(ok[None], QA, -np.inf).argmax(1)


exact_a = best("I")
qcell = np.argsort(-(Q.astype(np.float64) @ cen.T), axis=1)
approx_a = {}
for A in (1, 4):
    ok = np.zeros((NQ, NA), bool)
    ok[:, :K] = np.stack([np.isin(cell, qcell[j, :A]) for j in range(NQ)])
    approx_a[A] = np.where(ok, QA, -np.inf).argmax(1)


def extremes(anc, m, far):
    """Per query and cluster, the (hi, lo) the query's cut uses; +-inf where it has no anchor bound."""
    hi, lo = HI[anc], LO[anc]                                # NQ x K
    if m is None:
        return hi, lo, np.zeros((NQ, K), bool)
    nb = near[m][anc]
    if far == "cell":
        fhi, flo = CHI[anc][:, cell], CLO[anc][:, cell]
    else:
        fhi, flo = np.full((NQ, K), np.inf), np.full((NQ, K), -np.inf)
    return np.where(nb, hi, fhi), np.where(nb, lo, flo), nb


TRACKS = {"full table (K anchors)": (exact_a, None, None)}
for A in (32, 128, 512):
    TRACKS[f"{A} coarse anchors"] = (best(f"I{A}"), None, None)
if a.text_train:
    TRACKS["128 coarse + 64 text anchors"] = (best("I128", "T64"), None, None)
    TRACKS["512 coarse + 256 text anchors"] = (best("I512", "T256"), None, None)
    TRACKS["K + 256 text anchors"] = (best("I", "T256"), None, None)
for m in MS:
    TRACKS[f"near {m}, far cell"] = (exact_a, m, "cell")
TRACKS["near 64, far none"] = (exact_a, 64, "none")
TRACKS["full table, anchor from 1 cell"] = (approx_a[1], None, None)
TRACKS["full table, anchor from 4 cells"] = (approx_a[4], None, None)
TRACKS["no table"] = None

DA = 0.5 / 32767 + 1e-7
qm = Q.astype(np.float64) @ M8.T                      # q . m
cm_all = np.clip(qm / mn[None, :], -1, 1)
NT = len(TH)
touched = {t: np.zeros((NT, NQ)) for t in TRACKS}
opened = {t: np.zeros((NT, NQ)) for t in TRACKS}
celldec = {t: np.zeros((NT, NQ)) for t in TRACKS}      # clusters decided by their cell's bound alone
viol = {t: 0 for t in TRACKS}
EXT = {t: (extremes(*v) if v is not None else None) for t, v in TRACKS.items()}
BETA = {t: (QA[np.arange(NQ), v[0]] if v is not None else None) for t, v in TRACKS.items()}
for c in range(K):
    r0, r1 = cstart[c], cstart[c + 1]
    nc = r1 - r0
    if nc == 0:
        continue
    true = S[r0:r1].astype(np.float64)                 # nc x NQ
    ah = AH[r0:r1]
    alo, ahi = np.maximum(ah - DA, -1)[:, None], np.minimum(ah + DA, 1)[:, None]
    cm = cm_all[:, c]
    s1 = np.sqrt(np.maximum(1 - cm * cm, 0))
    aU, aL = np.clip(cm[None], alo, ahi), np.clip(-cm[None], alo, ahi)
    ub1 = cm * aU + s1 * np.sqrt(1 - aU * aU) + 3e-5
    lb1 = cm * aL - s1 * np.sqrt(1 - aL * aL) - 3e-5
    for t, v in TRACKS.items():
        if v is None:
            lb, ub, lbc, ubc, cd = lb1, ub1, np.full(NQ, -np.inf), np.full(NQ, np.inf), np.zeros(NQ, bool)
        else:
            anc = v[0]
            hi_all, lo_all, nb = EXT[t]
            hi, lo = hi_all[:, c], lo_all[:, c]
            beta = BETA[t]
            rho = np.sqrt(np.maximum(1 - beta ** 2, 0))
            fin = np.isfinite(hi) & np.isfinite(lo)
            hi0, lo0 = np.where(fin, hi, 1.0), np.where(fin, lo, -1.0)
            Bh = np.where(beta >= 0, beta * hi0, beta * lo0)
            Bl = np.where(beta >= 0, beta * lo0, beta * hi0)
            pm = AN[anc] @ Mh[c]
            rm = cm - beta * pm
            rp = np.sqrt(np.maximum(rho * rho - rm * rm, 0))
            rr = np.maximum(rho, 1e-12)
            aU2, aL2 = np.clip((rm / rr)[None], alo, ahi), np.clip((-rm / rr)[None], alo, ahi)
            ub2 = Bh + rm * aU2 + rp * np.sqrt(1 - aU2 * aU2) + 3e-5
            lb2 = Bl + rm * aL2 - rp * np.sqrt(1 - aL2 * aL2) - 3e-5
            ub2, lb2 = np.where(fin[None], ub2, np.inf), np.where(fin[None], lb2, -np.inf)
            lb, ub = np.maximum(lb1, lb2), np.minimum(ub1, ub2)
            lbc, ubc = np.where(fin, Bl - rho - 3e-5, -np.inf), np.where(fin, Bh + rho + 3e-5, np.inf)
            cd = fin & ~nb[:, c] if v[1] is not None else np.zeros(NQ, bool)
        viol[t] += int(((true > ub) | (true < lb)).sum())
        for ti in range(NT):
            th = thr[:, ti]
            touched[t][ti] += ((lb < th) & (ub >= th)).sum(0)
            op = (ubc >= th) & (lbc < th)
            opened[t][ti] += op
            celldec[t][ti] += cd & ~op
    if c % 100 == 0:
        log(f"cluster {c}")

print("\nviolations:", {t: v for t, v in viol.items() if v} or "none")
for q in ("item", "text"):
    sel = cls == q
    nts = range(NT) if q == "item" else range(4)
    print(f"\n== {q} queries ({sel.sum()}): items touched % | clusters opened % | clusters decided by a cell bound %")
    print(f"  {'':42s}", " ".join(f"{TH[i]:>6s}" for i in nts))
    base = touched["full table (K anchors)"]
    for t in TRACKS:
        print(f"  {t:42s}", " ".join(f"{100 * touched[t][i][sel].mean() / N:6.2f}" for i in nts), "|",
              " ".join(f"{100 * opened[t][i][sel].mean() / K:5.1f}" for i in nts), "|",
              " ".join(f"{100 * celldec[t][i][sel].mean() / K:5.1f}" for i in nts))
    print(f"  touched relative to the full table:")
    for t in TRACKS:
        print(f"  {t:42s}", " ".join(f"{touched[t][i][sel].mean() / max(base[i][sel].mean(), 1e-9):6.3f}" for i in nts))
