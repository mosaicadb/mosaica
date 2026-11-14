# Threshold queries at n = 768 (BioCLIP-2, 200k sample): the mixed rule on a flat global-rotation cascade.
# Both sides from codes: exclude when the upper bound at c_ex is below t, certify in when the lower bound at
# c_in reaches t, exact read for what is left after the last level. Constants are computed as if N = 1e9 at the
# same answer share (counts scaled by 1e9 / N_sample). Critical constants as in thr.py (Theorem B, one-sided).
# usage: flat.py <levels, e.g. s256,s512,1,2,4,6> <nq>   (env WORK: corpus.npy, queries.npy, thr.npy from split.py)
# sK = the signs of the first K rotated coordinates (K/8 bytes), a partial 1-bit code nested in the full one.
import sys, os, math, time, numpy as np, faiss
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from consts2 import c_exact
SPEC = sys.argv[1].split(','); NQ = int(sys.argv[2]); LV = len(SPEC); W = os.environ['WORK']
BITS = tuple(1 if b.startswith('s') else int(b) for b in SPEC); KEEP = [int(b[1:]) if b.startswith('s') else None for b in SPEC]
faiss.omp_set_num_threads(2)
EPS = 1e-9; N9 = 1e9; GAMMA = 1.4; JG = 30
t0 = time.time()
def log(*a): print(f'[{time.time()-t0:5.0f}s]', *a, flush=True)
X = np.load(f'{W}/corpus.npy').astype(np.float32)
X /= np.maximum(np.linalg.norm(X, axis=1, keepdims=True), 1e-12)
N, n = X.shape; SC = N9 / N; K = 200
km = faiss.Kmeans(n, K, niter=8, seed=1); km.train(X); C = km.centroids.copy(); _, a = km.index.search(X, 1); a = a[:, 0]
R = X - C[a]; rn = np.linalg.norm(R, axis=1).astype(np.float64)
rng = np.random.default_rng(7)
G = rng.standard_normal((n, n)); Qr, Rr = np.linalg.qr(G); Rot = (Qr * np.sign(np.diag(Rr))).astype(np.float32)
O = (R / np.maximum(rn, 1e-12)[:, None].astype(np.float32)) @ Rot.T
PC = C @ Rot.T; cr = (C[a] * R).sum(1).astype(np.float64); cn2 = (C * C).sum(1).astype(np.float64); del R
rsafe = np.maximum(rn, 1e-12)
def best_scale(O, b):
    Lv = 2 ** b - 1; best = None
    for s in np.linspace(0.6, 6.0, 55) * np.sqrt(n) * (Lv / 3.0) / 2:
        Cd = np.clip(2 * np.floor(O[:20000] * s / 2) + 1, -Lv, Lv)
        cs = ((Cd * O[:20000]).sum(1) / np.linalg.norm(Cd, axis=1)).mean()
        if best is None or cs > best[0]: best = (cs, s)
    return best[1]
CD = []; FAC = []; SD = []; ECR = []
for b, kp in zip(BITS, KEEP):
    if b == 1:
        Cd = np.sign(O).astype(np.float32); Cd[Cd == 0] = 1
        if kp is not None: Cd[:, kp:] = 0
    else: Lv = 2 ** b - 1; Cd = np.clip(2 * np.floor(O * best_scale(O, b) / 2) + 1, -Lv, Lv).astype(np.float32)
    cnn = np.linalg.norm(Cd, axis=1).astype(np.float64); cos = (Cd * O).sum(1).astype(np.float64) / cnn
    fac = rn / (cnn * cos); sd = rn * np.sqrt(np.maximum(1 - cos * cos, 0)) / cos / np.sqrt(n - 1)
    ec = cr - fac * (Cd * PC[a]).sum(1)
    CD.append(Cd); FAC.append(fac); SD.append(sd); ECR.append(ec - cr)
    log(f'{b} bit: mean tan(phi) {np.mean(np.sqrt(1 - cos * cos) / cos):.4f} (sd {np.std(np.sqrt(1 - cos * cos) / cos):.4f}), mean sd {sd.mean():.5f}')
del O
EQB = tuple(kp / n if kp is not None else b for b, kp in zip(BITS, KEEP)); BYT = [n * b / 8 for b in np.diff((0,) + EQB)]
lg = np.arange(8.0, 24.01, 0.05); ctab = np.array([c_exact(n, 10 ** -x) for x in lg])
def cd(delta): return float(np.interp(-math.log10(delta), lg, ctab))
c_today = cd(EPS / (2 * N9 * 5)); c_all = cd(EPS / (N9 * 5)); c_single = cd(EPS / LV)   # c_all: the plain all-items constant for five levels, 8.72
c_far = cd((EPS / 6) / (LV * N9)); c_acc = cd((EPS / 6) / (4 * JG * LV * LV)); c_floor = (GAMMA * c_far + c_acc) / (1 + GAMMA)
log(f'N={N} n={n} levels={BITS} bytes per level {BYT}; as-if N=1e9: today two-sided {c_today:.2f}, all N harmful side {c_all:.2f}, single item {c_single:.2f}; band rule: c_far {c_far:.2f} c_acc {c_acc:.2f} floor {c_floor:.2f}')
Q = np.load(f'{W}/queries.npy')[:NQ].astype(np.float32); THR = np.load(f'{W}/thr.npy')[:NQ]; PQ = Q @ Rot.T
KS = (15, 150, 1500, 15000, 45000)
DS = (0.002, 0.005, 0.01)
SCH = ['allN'] + [f'allN d={d}' for d in DS]
acc = {Kk: {s: np.zeros(2 * LV + LV + 5) for s in SCH} for Kk in KS}; haz = {Kk: [] for Kk in KS}; sdL = {Kk: [] for Kk in KS}
for qi in range(NQ):
    q = Q[qi]; S = (X @ q).astype(np.float64); qc = (C @ q).astype(np.float64)
    al = (qc / cn2)[a]; w = np.sqrt(np.maximum(1 - qc * qc / cn2, 0))[a]; base = qc[a] + al * cr; Rw = rsafe * w
    est = [(CD[l] @ PQ[qi]).astype(np.float64) * FAC[l] + al * ECR[l] for l in range(LV)]
    Ss = np.sort(S)[::-1]
    for ki, Kk in enumerate(KS):
        t = THR[qi, ki]; g = t - base; match = S >= t; nS = int(match.sum())
        D0 = 0.002; haz[Kk].append(((S >= t - D0) & (S < t + D0)).sum() / (2 * D0 * nS))
        nearb = np.abs(S - t) < 0.005; sdL[Kk].append(np.median((SD[-1] * w)[nearb]) if nearb.any() else np.nan)
        den = np.sqrt(np.maximum(Rw * Rw - g * g, 0)); crit = []; crin = []
        for l in range(LV):
            with np.errstate(divide='ignore', invalid='ignore'):
                x = (g - est[l]) * rsafe / (SD[l] * den); y = (est[l] - g) * rsafe / (SD[l] * den)
            x = np.where(g > Rw, np.inf, np.where(est[l] >= g, 0.0, x)); x[np.isnan(x)] = np.inf
            y = np.where(g < -Rw, np.inf, np.where(est[l] <= g, -np.inf, y)); y[np.isnan(y)] = -np.inf; y[g > Rw] = -np.inf
            crit.append(x); crin.append(y)
        crind = {}
        for d in DS:
            gd = g - d; dend = np.sqrt(np.maximum(Rw * Rw - gd * gd, 0)); cl = []
            for l in range(LV):
                with np.errstate(divide='ignore', invalid='ignore'): y = (est[l] - gd) * rsafe / (SD[l] * dend)
                y = np.where(gd < -Rw, np.inf, np.where(est[l] <= gd, -np.inf, y)); y[np.isnan(y)] = -np.inf; y[gd > Rw] = -np.inf
                cl.append(y)
            crind[d] = cl
        def run(cex, cin, band=False, crin=crin, okin=match):
            mask = np.ones(N, bool); out = []; lost = 0; wrong = 0; cs_e = []; cs_i = []
            for l in range(LV):
                if band:
                    c = c_all
                    while True:
                        D = int((mask & (crit[l] <= c) & (crin[l] < c)).sum())
                        c2 = min(c_all, max(c_floor, cd((EPS / 6) / (2 * LV * max(D, 1) * SC))))
                        if c2 >= c - 1e-9: break
                        c = c2
                    ce = ci = c
                else: ce, ci = cex, cin
                ex = mask & (crit[l] > ce); inn = mask & ~ex & (crin[l] >= ci)
                lost += int((ex & match).sum()); wrong += int((inn & ~okin).sum())
                mask = mask & ~ex & ~inn; out.append(mask.sum()); cs_e.append(ce); cs_i.append(ci)
            return np.array(cs_e + cs_i + out + [int((mask & match).sum()), int((mask & ~match).sum()), lost, wrong, nS], float)
        for s, r in (('allN', run(c_all, c_all)),) + tuple((f'allN d={d}', run(c_all, c_all, False, crind[d], S >= t - d)) for d in DS):
            acc[Kk][s] += r
    if (qi + 1) % 10 == 0: log(f'{qi + 1} queries')
print(f'\nBioCLIP-2 corpus N={N}, n={n}, levels {SPEC} ({[int(x) for x in np.cumsum(BYT)]} B cumulative), eps={EPS}, constants as if N=1e9 at the same answer share, {NQ} held-out items as queries.')
print('und l = undecided after level l per true match; reads = exact reads per true match (match side + non-match side); lost / wrong-in are totals over all queries (must be 0 except single*).')
for Kk in KS:
    print(f'\nanswer {Kk} of {N} ({100 * Kk / N:.2f}%; {Kk * SC:.0e} at 1e9); hazard {np.mean(haz[Kk]):.1f} per unit score; median last-level sd*w near t {np.nanmean(sdL[Kk]):.5f}')
    print(f'{"scheme":12} ' + ' '.join(f'{"cex" + str(l + 1):>5}' for l in range(LV)) + ' ' + ' '.join(f'{"cin" + str(l + 1):>5}' for l in range(LV)) + ' ' + ' '.join(f'{"und " + str(l + 1):>8}' for l in range(LV)) + f' {"reads":>7} {"(match":>7} {"non)":>6} {"lost":>5} {"wrong":>5} {"% f16":>6}')
    for s in SCH:
        v = acc[Kk][s] / NQ; nS = v[-1]; und = v[2 * LV:3 * LV]; rm, rnm = v[3 * LV], v[3 * LV + 1]
        byt = N * BYT[0] + sum(und[l] * BYT[l + 1] for l in range(LV - 1)) + (rm + rnm) * 2 * n
        print(f'{s:12} ' + ' '.join(f'{x:5.2f}' for x in v[:2 * LV]) + ' ' + ' '.join(f'{x / nS:8.3f}' for x in und) + f' {(rm + rnm) / nS:7.3f} {rm / nS:7.3f} {rnm / nS:6.3f} {acc[Kk][s][3 * LV + 2]:5.0f} {acc[Kk][s][3 * LV + 3]:5.0f} {100 * byt / (N * 2 * n):6.2f}')
np.savez(f'{W}/flat_out.npz', **{f'{Kk}|{s}': acc[Kk][s] for Kk in KS for s in SCH}, BYT=np.array(BYT), NQ=NQ, N=N, LV=LV, c_all=c_all)
