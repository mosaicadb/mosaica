# Toy check of the band rule for the mixed (two-sided) threshold decision, one level:
# exclude when the upper bound at c is below t, certify in when the lower bound at c reaches t, else read exactly.
# The constant is set from D = number left undecided. "raw" uses delta(c) = eps / D with no floor and no factor
# (conjectured sound); "proved" uses max(floor, c(12 D)), capped by the all-items constant.
import sys, os, math, numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from consts2 import tail
n = 64; m = n - 1; N = 2000; t = 0.5; EPS = float(sys.argv[1]); TR = int(sys.argv[2]); B = 500; GAM = 1.4; J = 8
rng = np.random.default_rng(5)
lg = np.arange(0.3, 8.01, 0.1)
def sq(d):
    lo, hi = 0.0, 1.0
    for _ in range(50):
        mid = (lo + hi) / 2
        if tail(m, mid) > d: lo = mid
        else: hi = mid
    return hi * math.sqrt(m)
ctab = np.array([sq(10 ** -x) for x in lg])
def cd(delta): return float(np.interp(-math.log10(min(delta, 0.49)), lg, ctab))
def corpus(mode):
    q = np.zeros(n); q[0] = 1; sig = 0.75 * math.sqrt(1 - t * t) / math.sqrt(m)       # typical error sd
    V = rng.standard_normal((N, n)); V[:, 0] = 0
    if mode == 'indep': sc = t + rng.uniform(-4, 4, N) * sig
    elif mode == 'atthr': sc = np.where(np.arange(N) < 1000, t - 1e-9, t + 1e-9) + 0 * V[:, 0]      # everything sits at the threshold
    elif mode == 'sparse': sc = np.where(np.arange(N) < 1900, rng.uniform(-0.9, t - 5 * sig, N), np.where(np.arange(N) % 2 == 0, t - 1e-9, t + 1e-9))   # 100 items at the threshold, the rest far below
    elif mode == 'sparsedup':
        sc = np.where(np.arange(N) < 1900, rng.uniform(-0.9, t - 5 * sig, N), np.where(np.arange(N) % 2 == 0, t - 1e-9, t + 1e-9)); V[1900:1950] = V[1900] + 1e-3 * V[1900:1950]
    elif mode == 'clusters':
        cl = rng.integers(0, 20, N); sc = (t + rng.uniform(-4, 4, 20) * sig)[cl]; base = rng.standard_normal((20, n)); V = base[cl] + 1e-3 * V
    elif mode == 'bigdup':                       # 1,500 duplicates 2.5 sd below t, 500 independent items just below t
        sc = np.where(np.arange(N) < 1500, t - 2.5 * sig, t - 1e-9); V[:1500] = V[0] + 1e-3 * V[:1500]
    V[:, 0] = 0; V /= np.linalg.norm(V, axis=1, keepdims=True)
    return q, sc[:, None] * q[None, :] + np.sqrt(1 - sc * sc)[:, None] * V, sc
c_all = cd(EPS / N); c_far = cd(EPS / 6 / N); c_acc = cd(EPS / 6 / (4 * J)); c_floor = (GAM * c_far + c_acc) / (1 + GAM)
print(f'n={n} N={N} eps={EPS} trials={TR}; all-items harmful-side c={c_all:.2f}; proved rule: c_far={c_far:.2f} floor={c_floor:.2f}')
for mode in sys.argv[3].split(','):
    q, X, sc = corpus(mode); match = sc >= t
    err = dict(allN=0, raw=0, proved=0); cm = dict(allN=0.0, raw=0.0, proved=0.0); dm = dict(allN=0.0, raw=0.0, proved=0.0)
    for b0 in range(0, TR, B):
        G = rng.standard_normal((B, n, n)); Qm, Rm = np.linalg.qr(G); P = Qm * np.sign(np.einsum('bii->bi', Rm))[:, None, :]
        Z = np.einsum('bij,xj->bxi', P, X); Pq = P[:, :, 0]
        code = np.sign(Z) / math.sqrt(n); cos = (code * Z).sum(2); est = np.einsum('bxi,bi->bx', code, Pq) / cos
        tan = np.sqrt(np.maximum(1 - cos * cos, 0)) / cos
        z = (est - t) / (tan * math.sqrt(1 - t * t)) * math.sqrt(m)       # certify in iff z >= c, exclude iff z < -c
        az = np.sort(np.abs(z), axis=1); bad = np.where(match[None, :], -z, z)   # item wrongly decided at c iff bad > c (strictly: >= on the in side)
        worst = bad.max(1)
        for i in range(B):
            def D(c): return int(np.searchsorted(az[i], c, side='right'))
            def fp(rule):
                c = c_all
                while True:
                    c2 = min(c_all, rule(max(D(c), 1)))
                    if c2 >= c - 1e-9: return c
                    c = c2
            for nm, c in (('allN', c_all), ('raw', fp(lambda d: cd(EPS / d))), ('proved', fp(lambda d: max(c_floor, cd(EPS / (12 * d)))))):
                err[nm] += worst[i] > c; cm[nm] += c; dm[nm] += D(c)
    print(f'{mode:9s} ' + '  '.join(f'{nm}: fail {err[nm] / TR:.4f} c {cm[nm] / TR:.2f} reads {dm[nm] / TR:.0f}' for nm in err), flush=True)
