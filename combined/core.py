# Shared pieces for the combined design: quantisers (uniform clipped, nested tree, escape), greedy orders,
# k-means, grids for stored scalars, and the exact log-MGF of a sphere coordinate for the sharp randomised tail.
import numpy as np, math, time
from scipy.special import gammaln

T0 = time.time()
def log(*a): print(f"[{time.time()-T0:6.0f}s]", *a, flush=True)
def unit(a):
    a = np.asarray(a, dtype=np.float32); return a / np.linalg.norm(a, axis=1, keepdims=True)

def kmeans(S, K, rng, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for _ in range(iters):
        a = np.concatenate([(S[i:i+50_000] @ C.T).argmax(1) for i in range(0, len(S), 50_000)])
        M = np.zeros_like(C); np.add.at(M, a, S)
        cnt = np.bincount(a, minlength=K); empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C

# ---------- uniform clipped quantiser of FINDINGS section 4 (independent allocation per budget) ----------
STEP = {2: 0.9957, 3: 0.5860, 4: 0.3352, 5: 0.1881, 6: 0.1041, 7: 0.0569, 8: 0.0308}
GT = np.array([1, .3634, .1188, .03744, .01154, .00349, .00104, .0003043, .0000876])
def quant_uni(Z, bits):
    out = np.zeros_like(Z)
    for b in np.unique(bits):
        if b == 0: continue
        cols = np.where(bits == b)[0]; z = Z[:, cols]
        if b == 1: out[:, cols] = np.where(z >= 0, 0.7979, -0.7979)
        else:
            s = STEP[int(b)]; L = 2 ** int(b)
            out[:, cols] = (np.clip(np.floor(z / s), -L // 2, L // 2 - 1) + 0.5) * s
    return out
def alloc_bound(lam, Qe, B):
    """FINDINGS 4.4: greedy bits minimising sum sqrt(Q lam g(b))."""
    b = np.zeros(len(lam), int); a = np.sqrt(Qe * lam); sg = np.sqrt(GT)
    gain = a * (sg[0] - sg[1])
    for _ in range(B):
        i = int(gain.argmax())
        if gain[i] <= 0: break
        b[i] += 1; gain[i] = a[i] * (sg[b[i]] - sg[b[i] + 1]) if b[i] < 8 else -1
    return b

# ---------- Gaussian cell helpers ----------
_xs = np.linspace(-9, 9, 360001); _pg = np.exp(-_xs * _xs / 2); _pg /= _pg.sum()
_cp = np.concatenate([[0], np.cumsum(_pg)]); _cxp = np.concatenate([[0], np.cumsum(_xs * _pg)])
def _cell(lo, hi):
    a, b = np.searchsorted(_xs, lo), np.searchsorted(_xs, hi); m = _cp[b] - _cp[a]
    return m, (_cxp[b] - _cxp[a]) / max(m, 1e-300)

# ---------- nested tree-structured quantiser (agent1), used inside rotated randomised blocks ----------
def _tree_tables():
    T = {}; g = [1.0]; edges = [-np.inf, np.inf]
    for b in range(1, 9):
        new = [edges[0]]
        for lo, hi in zip(edges[:-1], edges[1:]):
            t = _cell(lo, hi)[1]
            for _ in range(60): t = 0.5 * (_cell(lo, t)[1] + _cell(t, hi)[1])
            new += [t, hi]
        edges = new
        mc = [_cell(lo, hi) for lo, hi in zip(edges[:-1], edges[1:])]
        T[b] = (np.array(edges[1:-1], np.float32), np.array([c for _, c in mc], np.float32))
        g.append(1 - sum(m * c * c for m, c in mc))
    return T, np.array(g)
TREE, GTREE = _tree_tables()

# ---------- escape code (agent1): depth 1 sign, depth 2 unary index on grid W, deeper halves the cell ----------
W = 0.6745; EMAXD = 14
_kk = np.arange(64); _mc2 = [_cell(k * W, (k + 1) * W) for k in _kk]
CENT2 = np.array([c if m > 1e-12 else (k + 0.5) * W for k, (m, c) in zip(_kk, _mc2)], np.float32)
P2 = np.array([2 * m for m, _ in _mc2])
gE = [1.0, 0.3634, 1 - float((P2 * CENT2.astype(np.float64) ** 2).sum())]
for j in range(3, EMAXD + 1):
    wj = W * 2.0 ** (2 - j); idx = np.floor(np.abs(_xs) / wj); gE.append(float((_pg * (np.abs(_xs) - (idx + 0.5) * wj) ** 2).sum()))
gE = np.array(gE); cE = np.ones(EMAXD + 1); cE[1] = float((P2 * (_kk + 1)).sum())
def recon_esc(U, dep):
    a = np.abs(U); wj = (W * np.exp2(2.0 - np.maximum(dep, 2))).astype(np.float32); idx = np.floor(a / wj)
    val = (idx + 0.5) * wj
    m2 = dep == 2; val[m2] = CENT2[np.minimum(idx[m2], 63).astype(int)]
    val[dep == 1] = 0.7979; val[dep == 0] = 0
    return np.sign(U) * val
def esc_steps(lam, Qe, total_bits):
    """Greedy step order (one coordinate one depth deeper per step) by bound gain per expected bit."""
    n = len(lam); b = np.zeros(n, int); a = np.sqrt(Qe * lam); sg = np.sqrt(gE); seq = []; spent = 0.0
    gain = a * (sg[0] - sg[1]) / cE[0]
    while spent < total_bits and gain.max() > 0:
        i = int(gain.argmax()); seq.append(i); spent += cE[b[i]]; b[i] += 1
        gain[i] = a[i] * (sg[b[i]] - sg[b[i] + 1]) / cE[b[i]] if b[i] < EMAXD else -1
    se = np.array(seq)
    dep_at = np.zeros((len(se) + 1, n), np.int8)
    for s_, c_ in enumerate(se): dep_at[s_ + 1] = dep_at[s_]; dep_at[s_ + 1, c_] += 1
    sdep = dep_at[np.arange(1, len(se) + 1), se].astype(int)
    expc = np.cumsum(cE[sdep - 1])
    return se, dep_at, sdep, expc

# ---------- grids for stored scalars ----------
def roundup16(v):   # one byte, ratio 2^(1/16), rounded up
    return np.exp2(np.ceil(np.log2(np.maximum(v, 1e-30)) * 16) / 16)
RHO_Y = 2.0 ** (1 / 4096)    # block norm, two bytes, rounded up
def roundup_y(v): return np.exp2(np.ceil(np.log2(np.maximum(v, 1e-30)) * 4096) / 4096)
RHO_T = 2.0 ** (1 / 20)      # tan(phi), one byte, rounded up
def roundup_t(v): return np.exp2(np.ceil(np.log2(np.maximum(v, 2.0 ** -12.8)) * 20) / 20)

# ---------- exact sphere-coordinate log-MGF, tabulated; linear interpolation of a convex function is an upper bound ----------
class LogMGF:
    def __init__(self, m, xmax=640.0, dx=0.1, kmax=2500):
        self.m = m; self.dx = dx; self.x = np.arange(0, xmax + dx / 2, dx)
        b = m / 2.0; k = np.arange(kmax)
        lt = -(gammaln(b + k) - gammaln(b)) - gammaln(k + 1)
        out = np.zeros(len(self.x))
        for i0 in range(1, len(self.x), 500):
            z = (self.x[i0:i0 + 500] ** 2 / 4)[:, None]
            t = k[None] * np.log(z) + lt[None]; mx = t.max(1, keepdims=True)
            out[i0:i0 + 500] = (mx + np.log(np.exp(t - mx).sum(1, keepdims=True)))[:, 0]
        self.f = out * (1 + 1e-9) + 1e-12
        assert np.all(self.f <= self.x ** 2 / (2 * m) + 1e-9), "sphere log-MGF above its sub-Gaussian bound"
        assert np.all(np.diff(self.f, 2) >= -1e-9), "tabulated log-MGF not convex"
    def __call__(self, x):
        i = np.minimum((x / self.dx).astype(np.int64), len(self.x) - 2); fr = x / self.dx - i
        v = self.f[i] * (1 - fr) + self.f[i + 1] * fr
        return np.where(x >= self.x[-1], x * x / (2 * self.m), v)
LAMS = np.array([1.0, 1.08, 1.17, 1.27, 1.4, 1.55, 1.75, 2.0, 2.4])
def chernoff_halfwidth(A, ms, tabs, L1):
    """A: pairs x blocks of scale bounds a_b (sum a_b u_b, u_b a coordinate of a uniform unit vector in R^{m_b});
    one-sided tail <= exp(-L1) at the returned h (two-sided: twice). Min over a fixed lambda grid of the Chernoff bound."""
    sig = np.sqrt((A * A / ms[None]).sum(1)); lam0 = np.sqrt(2 * L1) / np.maximum(sig, 1e-30); best = np.full(len(A), np.inf)
    for f in LAMS:
        lam = lam0 * f; s = np.full(len(A), L1)
        for b in range(A.shape[1]): s += tabs[b](lam * A[:, b])
        best = np.minimum(best, s / lam)
    return np.where(sig > 0, best, 0.0)
