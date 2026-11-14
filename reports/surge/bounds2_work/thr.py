# Threshold queries on the prebuilt 1/2/4-bit cascade (read only): survivors per level under each way of
# choosing the exclusion constant, with verified inclusion. Thresholds are the K-th best score, K = answer size.
# Per item and level the "critical constant" is the smallest c at which Theorem B's upper bound reaches t
# (one-sided form): crit = (g - est) |r| / (sd sqrt(R^2 - g^2)), g = t - known part, R = |r| w.
# An item passes level l at constant c iff crit_l <= c. Likewise crit_in for the lower bound (certify in).
# usage: thr.py <name> <nq>
import sys, os, math, time, numpy as np, h5py
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from consts2 import c_exact
name, NQ = sys.argv[1], int(sys.argv[2])
wd = f'/home/joe/code/vexact-data/{name}/cascade'
EPS = 1e-9; LV = 3; KS = (10, 100, 1000, 10000, 100000)
t0 = time.time()
def log(*a): print(f'[{time.time()-t0:5.0f}s]', *a, flush=True)
def unit(A): A = A.astype(np.float32); return A / np.maximum(np.linalg.norm(A, axis=1, keepdims=True), 1e-12)
L = lambda key: np.load(f'{wd}/{key}.npy', mmap_mode='r')
X = np.asarray(L('X')); a = np.asarray(L('a')); C = np.asarray(L('C')); Rot = np.asarray(L('Rot'))
rn = np.asarray(L('rn')).astype(np.float64); cr = np.asarray(L('cr')).astype(np.float64)
N, n = X.shape; cn2 = (C * C).sum(1).astype(np.float64); rsafe = np.maximum(rn, 1e-12)
CODES = ('c1', 'c2', 'c4n'); BYTES = (n / 8, n / 8, n / 4, 2 * n)
CD = {c: np.asarray(L(c)) for c in CODES}
FAC = {c: np.asarray(L(c + '_fac')).astype(np.float64) for c in CODES}; SD = {c: np.asarray(L(c + '_sd')).astype(np.float64) for c in CODES}
ECR = {c: np.asarray(L(c + '_ec')).astype(np.float64) - cr for c in CODES}
Qa = np.asarray(h5py.File(f'/home/joe/code/vexact-data/{name}/{name}.hdf5', 'r')['test']); Q = unit(Qa[np.linalg.norm(Qa, axis=1) > 0][:NQ])
# c as a function of the union count U (events = U x LV, one-sided), tabulated
lg = np.arange(0, 10.01, 0.05); ctab = np.array([c_exact(n, EPS / (LV * 10 ** x)) for x in lg])
def cU(U): return float(np.interp(math.log10(max(U, 1.0)), lg, ctab))
M0 = 64; FACG = 2 + 1 / M0 + 4 / M0
def cap(S):                                   # first cap of the grid M0 * 2^j that is >= S
    M = M0
    while M < S: M *= 2
    return M
c_N = cU(N); c_1e9 = cU(1e9); c_in = cU(2 * N)       # certify-in side: all items, half of eps
log(f'{name}: N={N} n={n} queries={len(Q)}; c(all N)={c_N:.2f} c(N=1e9)={c_1e9:.2f} c_in={c_in:.2f} grid factor {FACG:.2f}')
SCH = ['allN', 'selfbound', 'capgrid', 'oracle']
acc = {K: {s: np.zeros(8) for s in SCH} for K in KS}      # c1,c2,c3,s1,s2,s3,wrongly excluded,certified-in matches (mixed rule)
bad_in = 0; PQ = Q @ Rot.T
for qi in range(len(Q)):
    q = Q[qi]; S = (X @ q).astype(np.float64); qc = (C @ q).astype(np.float64)
    al = (qc / cn2)[a]; w = np.sqrt(np.maximum(1 - qc * qc / cn2, 0))[a]; base = qc[a] + al * cr; R = rsafe * w
    est = {c: (CD[c] @ PQ[qi]).astype(np.float64) * FAC[c] + al * ECR[c] for c in CODES}
    Ss = np.sort(S)[::-1]
    for K in KS:
        if K > N // 5: continue
        t = Ss[K - 1]; g = t - base; match = S >= t; nS = int(match.sum())
        den = np.sqrt(np.maximum(R * R - g * g, 0))
        crit = []; crin = []
        for c in CODES:
            with np.errstate(divide='ignore', invalid='ignore'):
                x = (g - est[c]) * rsafe / (SD[c] * den); y = (est[c] - g) * rsafe / (SD[c] * den)
            x = np.where(g > R, np.inf, np.where(est[c] >= g, 0.0, x)); x[np.isnan(x)] = np.inf     # g > R: excluded with certainty
            y = np.where(g < -R, np.inf, np.where(est[c] <= g, -np.inf, y)); y[np.isnan(y)] = -np.inf; y[g > R] = -np.inf
            crit.append(x); crin.append(y)
        cm = np.maximum.accumulate(np.stack(crit), axis=0)             # pass through level l iff cm[l] <= c
        srt = [np.sort(cm[l]) for l in range(3)]
        def run(cs):
            p = [cm[l] <= cs[l] for l in range(3)]
            p[1] &= p[0]; p[2] &= p[1]
            cin = (crin[2] >= c_in) & p[1] | (crin[1] >= c_in) & p[0]          # certified in at level 2 or 3 at c_in
            return [cs[0], cs[1], cs[2], p[0].sum(), p[1].sum(), p[2].sum(), int((match & ~p[2]).sum()), int((cin & p[2] & match).sum())], int((cin & ~match).sum())
        # self-bounding: per level, U <- #{cm_l <= c(U)} from the previous level's count down to its fixed point
        cs = []; U = N
        for l in range(3):
            while True:
                c = cU(U); U2 = int(np.searchsorted(srt[l], c, side='right'))
                if U2 >= U: break
                U = U2
            cs.append(cU(U))
        cg = cU(FACG * cap(nS)); co = cU(nS)
        for s, cc in (('allN', [c_N] * 3), ('selfbound', cs), ('capgrid', [cg] * 3), ('oracle', [co] * 3)):
            r, b = run(cc); acc[K][s] += np.array(r, float); bad_in += b
    if (qi + 1) % 10 == 0: log(f'{qi + 1} queries')
nq = len(Q)
print(f'\n{name}: N={N}, eps={EPS}, {LV} levels, {nq} queries. Mean per query. bytes = N x {BYTES[0]} + s1 x {BYTES[1]} + s2 x {BYTES[2]} + full x {BYTES[3]} (MB)')
print('non-matches certified in at c_in (must be 0):', bad_in)
for K in KS:
    if K > N // 5: continue
    print(f'\nanswer size {K}')
    print(f'{"scheme":10} {"c1":>5} {"c2":>5} {"c3":>5} {"pass 1":>9} {"pass 2":>9} {"pass 4":>9} {"lost":>5} {"cert-in":>8} {"MB verified":>12} {"MB mixed":>9}')
    for s in SCH:
        v = acc[K][s] / nq
        mb = (N * BYTES[0] + v[3] * BYTES[1] + v[4] * BYTES[2] + v[5] * BYTES[3]) / 1e6
        print(f'{s:10} {v[0]:5.2f} {v[1]:5.2f} {v[2]:5.2f} {v[3]:9.0f} {v[4]:9.0f} {v[5]:9.0f} {acc[K][s][6]:5.0f} {v[7]:8.0f} {mb:12.2f} {mb - v[7] * BYTES[3] / 1e6:9.2f}')
