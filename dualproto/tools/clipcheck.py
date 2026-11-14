# The dead-list experiment (exp4 PCA basis vs exp5 simultaneous diagonalisation, uniform quantiser of FINDINGS 4) redone
# on a subset of the prototype's groups, with and without clipping. Reports, per budget, the clip rate (coded coordinates
# outside the quantiser's range), the mean certified half-width h, and the share of (item, query) pairs undecided at each
# query's 1% threshold. The unclipped quantiser keeps the same cells but extends the outer ones without bound (not a
# fixed-rate code: its outer indices would need escape bits, not counted here).
import numpy as np, sys, time
P = "/tmp/claude-0/-home-user-mosaica/8c244050-1af9-5b77-a4e8-97638e77c857/scratchpad/vc/proto/"
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
def unit(a):
    a = np.asarray(a, dtype=np.float32); return a / np.linalg.norm(a, axis=1, keepdims=True)
I = np.load(P + "idx_m768/index.npz"); Xs = np.load(P + "idx_m768/Xs.npy", mmap_mode="r")
N, n, K, G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"])
Mq = I["Mq"].astype(np.float32); assign, cstart, grp, qi = I["assign"], I["cstart"], I["grp"], I["qi"]
lamP, VP, Q, qcls, thr = I["lam"], I["V"], I["Q"].astype(np.float32), I["qcls"], I["thr"]
NG = int(sys.argv[1]) if len(sys.argv) > 1 else 12
STEP = {2: 0.9957, 3: 0.5860, 4: 0.3352, 5: 0.1881, 6: 0.1041, 7: 0.0569, 8: 0.0308}
GT = np.array([1, .3634, .1188, .03744, .01154, .00349, .00104, .0003043, .0000876])
BUDGETS = [32, 64, 128, 256, 384, 768]
def quant(Z, bits, clip):
    out = np.zeros_like(Z); nclip = 0; ncoded = 0
    for b in np.unique(bits):
        if b == 0: continue
        cols = np.where(bits == b)[0]; z = Z[:, cols]; ncoded += z.size
        if b == 1: out[:, cols] = np.where(z >= 0, 0.7979, -0.7979); continue
        s = STEP[int(b)]; L = 2 ** int(b); k = np.floor(z / s)
        nclip += int(((k < -L // 2) | (k > L // 2 - 1)).sum())
        out[:, cols] = ((np.clip(k, -L // 2, L // 2 - 1) if clip else k) + 0.5) * s
    return out, nclip, ncoded
def alloc(a, B):                       # greedy bits minimising sum sqrt(a GT(b)), a = per-coordinate query x item energy
    b = np.zeros(n, int); sg = np.sqrt(GT); gain = np.sqrt(a) * (sg[0] - sg[1])
    for _ in range(B):
        i = int(gain.argmax()); b[i] += 1; gain[i] = np.sqrt(a[i]) * (sg[b[i]] - sg[b[i] + 1]) if b[i] < 8 else -1
    return b
rng = np.random.default_rng(0); pool = np.setdiff1d(np.arange(N), qi)
pq = unit(Xs[np.sort(rng.choice(pool, 20000, replace=False))]); pk = np.argsort(-(pq @ Mq.T), axis=1)
def cov_delta(mask, kk):
    rows, cols = np.where(mask[:, :kk]); M = Mq[pk[rows, cols]]
    if __import__('os').environ.get('PROJ'):              # offsets q - (q.m^)m^ instead of q - m
        Mh_ = M / np.linalg.norm(M, axis=1, keepdims=True); M = (Mh_ * (pq[rows] * Mh_).sum(1, keepdims=True))
    D = (pq[rows] - M).astype(np.float64)
    return D.T @ D / max(len(rows), 1), len(rows)
SD_all, _ = cov_delta(np.ones_like(pk, bool), 8)
gsz = np.bincount(grp[assign], minlength=G); gsel = np.argsort(-gsz)[::max(1, G // NG)][:NG]   # spread over sizes
log("groups", gsel.tolist(), "items", int(gsz[gsel].sum()))
T1 = thr[:, 2]; isitem = qcls == "item"
acc = {}
for g in gsel:
    ks = np.where(grp == g)[0]; kpos = -np.ones(K, int); kpos[ks] = np.arange(len(ks))
    lam = np.maximum(lamP[g], 1e-12); U = VP[g].astype(np.float64)
    # exp4: PCA basis, profile from the 4 nearest clusters; exp5: simultaneous diagonalisation, 8 nearest, 0.8/0.2 shrink
    m4 = grp[pk[:, :4]] == g; rows, cols = np.where(m4)
    Qe = (((pq[rows] - Mq[pk[rows, cols]]) @ U) ** 2).mean(0) if len(rows) > 50 else lam
    SDg, npair = cov_delta(np.pad(grp[pk[:, :8]] == g, ((0, 0), (0, pk.shape[1] - 8))), 8)
    SDg = SD_all if npair < 200 or __import__('os').environ.get('GLOBAL') else 0.8 * SDg + 0.2 * SD_all
    RIDGE = float(__import__('os').environ.get('RIDGE', '0')); SDg = SDg + RIDGE * np.trace(SDg) / n * np.eye(n)
    Lh = np.sqrt(lam); gam, Pm = np.linalg.eigh((U * Lh).T @ SDg @ (U * Lh)); gam = np.maximum(gam[::-1], 1e-30); Pm = Pm[:, ::-1]
    bases = {"pca": (U, U, np.sqrt(lam), lam * Qe), "simdiag": ((U / Lh) @ Pm, (U * Lh) @ Pm, np.ones(n), gam)}
    if __import__('os').environ.get('DUALDIR'):                # the exported dual pair: z = VD^T r with mean square sqrt(gamma)
        dd = __import__('os').environ['DUALDIR']; VQd = np.load(dd + "/V.npy", mmap_mode="r")[g].astype(np.float64); gd = np.load(dd + "/gamma.npy")[g]
        bases["dual"] = (np.linalg.inv(VQd).T, VQd, gd ** 0.25, gd)
    if __import__('os').environ.get('ONLY'): bases = {k: v for k, v in bases.items() if k == __import__('os').environ['ONLY']}
    s, e = np.searchsorted(grp[assign], [g, g + 1])
    for name, (Vi, Vq, sig, en) in bases.items():           # z = r Vi (item side), query side q Vq, <q Vq, z> = q.r
        QV = Q @ Vq; MV = Mq[ks] @ Vq; Dl = QV[None] - MV[:, None]
        specs = []
        for B in BUDGETS:
            bits = alloc(en, B); Dv = ((sig ** 2 * GT[bits] + 1e-20) / (en / sig ** 2 + 1e-20)) ** 0.25
            specs.append((B, bits, Dv, np.sqrt(((Dl * Dv) ** 2).sum(2))))
        for c in range(s, e, 20000):
            c1 = min(e, c + 20000); x = np.asarray(Xs[c:c1], np.float32); a = assign[c:c1]; ap = kpos[a]
            true = x @ Q.T; base = (Mq[a] @ Q.T); z = (x - Mq[a]) @ Vi; mva = MV[ap]
            for B, bits, Dv, dw in specs:
                for clip in (True, False):
                    code, ncl, nco = quant(z / sig, bits, clip); code *= sig
                    cn2 = (code * code).sum(1) + 1e-20; gm = (z * code).sum(1) / cn2
                    ev = z - gm[:, None] * code; sc = code @ QV.T; ew = np.linalg.norm(ev / Dv, axis=1); me = (mva * ev).sum(1)
                    est = base + gm[:, None] * sc + me[:, None]; h = dw[ap] * ew[:, None] * 1.0005 + 5e-5
                    viol = int((np.abs(est - true) > h).sum())
                    und = (est - h < T1[None]) & (est + h >= T1[None])
                    k_ = (name, B, clip); r = acc.setdefault(k_, np.zeros(8))
                    r += [ncl, nco, h[:, isitem].sum(), h[:, ~isitem].sum(), und[:, isitem].sum(), und[:, ~isitem].sum(), len(x), viol]
    log("group", g, "done")
nI, nT = isitem.sum(), (~isitem).sum()
print("\nbasis    budget(B) clip  clip-rate%   mean h item   mean h text   undecided@1% item%  text%  violations")
for (name, B, clip), r in sorted(acc.items()):
    print(f"{name:8s} {B//8:6d}   {str(clip):5s} {100*r[0]/max(r[1],1):9.3f}   {r[2]/r[6]/nI:11.5f}   {r[3]/r[6]/nT:11.5f}   {100*r[4]/r[6]/nI:11.3f}  {100*r[5]/r[6]/nT:7.3f}  {int(r[7])}")
