# E3: exact optimum of the deterministic bound over (quantiser box) ∩ (stored-norm ellipsoid), against weighted Cauchy-Schwarz.
# E4: proved deterministic floor against the achieved bound, from the measured spectra.
# One-group (global residual basis) version of the deterministic scheme, 200k subset.
import numpy as np, time, math
rng = np.random.default_rng(2)
DATA = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/data/"
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = unit(np.concatenate([np.load(DATA + f"e{i:02d}.npy") for i in (0, 5, 10, 15)]))
N, d = X.shape; K = 200
def kmeans(S, K, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for it in range(iters):
        a = np.concatenate([(S[i:i+50_000] @ C.T).argmax(1) for i in range(0, len(S), 50_000)])
        M = np.zeros_like(C); np.add.at(M, a, S)
        cnt = np.bincount(a, minlength=K); empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C
STEP = {2: 0.9957, 3: 0.5860, 4: 0.3352, 5: 0.1881, 6: 0.1041, 7: 0.0569, 8: 0.0308}
GT = np.array([1, .3634, .1188, .03744, .01154, .00349, .00104, .0003043, .0000876])
def alloc(lam, Qe, B):
    n = len(lam); b = np.zeros(n, int); f = lambda bb: np.sqrt(Qe * lam * GT[bb]); val = f(b)
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - f(nb); gain[b >= 8] = -1
        i = int(gain.argmax()); b[i] += 1; val = f(b)
    return b
Chat = kmeans(X, K); assign = (X @ Chat.T).argmax(1)
Msum = np.zeros((K, d)); np.add.at(Msum, assign, X)
Mean = (Msum / np.maximum(np.bincount(assign, minlength=K), 1)[:, None]).astype(np.float32)
Rs = X - Mean[assign]
lam, V = np.linalg.eigh((Rs.T @ Rs).astype(np.float64) / N); lam = np.maximum(lam[::-1], 1e-12).copy(); V = V[:, ::-1].astype(np.float32).copy()
sig = np.sqrt(lam).astype(np.float32)
pqi = rng.choice(N, 2000, replace=False); pq = X[pqi]; pk = np.argsort(-(pq @ Chat.T), axis=1)[:, :4]
Doff = ((pq[:, None, :] - Mean[pk]).reshape(-1, d) @ V); Qe = (Doff ** 2).mean(0).astype(np.float64)
gam = np.sort(lam * Qe)[::-1]
# ---- E4: floor.  E h >= max_k  E||g_k|| * omega_k^(-1/k) * sqrt(2 pi e) * GM_k(sqrt gamma) * 2^(-B/k)
def floor(B):
    best = 0; kb = 0; ls = np.cumsum(0.5 * np.log(gam))
    for k in range(1, d + 1):
        lg = (0.5 * math.log(2) + math.lgamma((k + 1) / 2) - math.lgamma(k / 2)) - (0.5 * math.log(math.pi) - math.lgamma(k / 2 + 1) / k) \
             + 0.5 * math.log(2 * math.pi * math.e) + ls[k - 1] / k - B / k * math.log(2)
        if math.exp(lg) > best: best, kb = math.exp(lg), k
    return best, kb
print("E4  model = Gaussian residual and Gaussian query offset with the measured variances; all numbers are expected half-widths")
print("    bits | proved floor (k*) | floor with 48 scalar bits counted | achieved l1 = sum sqrt(gamma g(b)) | achieved/floor | sqrt(sum gamma g) (l2) | PR=(l1/l2)^2 | zero-bit l1")
for B in [32, 128, 256, 768, 1536, 3072]:
    b = alloc(lam, Qe, B); c = np.sqrt(lam * Qe * GT[b]); f, kb = floor(B); f2, _ = floor(B + 48)
    print(f"   {B:5d} | {f:.5f} ({kb:3d}) | {f2:.5f} | {c.sum():.5f} | {c.sum() / f:5.2f} | {np.sqrt((c ** 2).sum()):.5f} | {c.sum() ** 2 / (c ** 2).sum():6.1f} | {np.sqrt(lam * Qe).sum():.5f}")
# ---- E3: box ∩ ellipsoid
def quant_box(Zs, bits):
    code = np.zeros_like(Zs); lo = np.full_like(Zs, -np.inf); hi = np.full_like(Zs, np.inf)      # feasible interval for z/sig
    for b in np.unique(bits):
        if b == 0: continue
        cols = np.where(bits == b)[0]; z = Zs[:, cols]
        if b == 1:
            code[:, cols] = np.where(z >= 0, 0.7979, -0.7979); lo[:, cols] = np.where(z >= 0, 0, -np.inf); hi[:, cols] = np.where(z >= 0, np.inf, 0)
        else:
            s = STEP[int(b)]; L = 2 ** int(b); q = np.clip(np.floor(z / s), -L // 2, L // 2 - 1)
            code[:, cols] = (q + 0.5) * s
            lo[:, cols] = np.where(q == -L // 2, -np.inf, q * s); hi[:, cols] = np.where(q == L // 2 - 1, np.inf, (q + 1) * s)
    return code, lo, hi
def boxball(w, l, u, rho, iters=50):
    # upper bound on max w.f over l<=f<=u, ||f||<=rho, by the Lagrangian dual g(mu) = max_box w.f - mu(||f||^2 - rho^2), any mu>=0 is valid
    wn = np.linalg.norm(w, axis=1); tlo = np.log(rho / wn); thi = tlo + np.log(1e4)
    for _ in range(iters):
        t = 0.5 * (tlo + thi); f = np.clip(np.exp(t)[:, None] * w, l, u); big = np.linalg.norm(f, axis=1) > rho
        thi = np.where(big, t, thi); tlo = np.where(big, tlo, t)
    tau = np.exp(thi); f = np.clip(tau[:, None] * w, l, u); mu = 1 / (2 * tau)
    return (w * f).sum(1) - mu * ((f * f).sum(1) - rho ** 2)
qs = np.repeat(np.arange(400), 25); cl = pk[qs, 0]
it = np.array([rng.choice(np.where(assign == c)[0]) for c in cl])
z = (Rs[it] @ V).astype(np.float64); Dl = ((pq[qs] - Mean[cl]) @ V).astype(np.float64)
print("\nE3  10,000 (query, item in the query's nearest cluster) pairs")
print("    bits | mean weighted-CS bound (as used) | mean box∩ellipsoid bound | ratio of means | mean of ratios | mean actual |err| / CS | violations (CS, box∩ell)")
for B in [256, 768, 1536]:
    bits = alloc(lam, Qe, B); Dv = ((lam * GT[bits] + 1e-10) / (Qe + 1e-10)) ** 0.25
    code, lo, hi = quant_box(z / sig, bits); e = z - code * sig; lo = lo * sig - code * sig; hi = hi * sig - code * sig
    act = np.abs((Dl * e).sum(1))
    r1 = np.linalg.norm(e, axis=1); r2 = np.linalg.norm(e / Dv, axis=1)
    cs = np.minimum(np.linalg.norm(Dl, axis=1) * r1, np.linalg.norm(Dl * Dv, axis=1) * r2)
    bb = np.full(len(z), np.inf)
    for sgn in (1, -1):
        b1 = boxball(sgn * Dl, lo, hi, r1); b2 = boxball(sgn * Dl * Dv, lo / Dv, hi / Dv, r2)
        bb = np.minimum(bb, np.minimum(b1, b2)) if sgn == 1 else np.maximum(bbp, np.minimum(b1, b2))
        bbp = bb.copy()
    print(f"   {B:5d} | {cs.mean():.5f} | {bb.mean():.5f} | {bb.mean() / cs.mean():.3f} | {(bb / cs).mean():.3f} | {(act / cs).mean():.3f} | {int((act > cs * 1.0001).sum())}, {int((act > bb * 1.0001).sum())}")
