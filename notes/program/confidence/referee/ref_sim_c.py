"""Referee simulation of Theorem C, Claim E and Lemma C2 at small sizes."""
import numpy as np, sys
from scipy.special import betainc, betaincinv
def T(d, x):
    x = np.asarray(x, float); ax = np.clip(np.abs(x), 0, 1)
    t = 0.5 * betainc((d - 1) / 2, 0.5, 1 - ax * ax); return np.where(x >= 0, t, 1 - t)
def qd(d, delta): return float(np.sqrt(1 - betaincinv((d - 1) / 2, 0.5, 2 * delta)))
rng = np.random.default_rng(7)
def unit(a): return a / np.linalg.norm(a, axis=-1, keepdims=True)
def haar(batch, n):
    Q, R = np.linalg.qr(rng.standard_normal((batch, n, n)))
    return Q * np.sign(np.einsum('bii->bi', R))[:, None, :]
def perp_unit(batch, N, ref):
    """N uniform unit vectors orthogonal to ref[b] for each b. ref: (batch, n) unit."""
    w = rng.standard_normal((batch, N, ref.shape[-1]))
    w -= (w * ref[:, None, :]).sum(-1, keepdims=True) * ref[:, None, :]
    return unit(w)
def nearest(z, C):                       # z (..., n), C (M, n) -> codeword, cos
    dots = z @ C.T; idx = dots.argmax(-1); return C[idx], np.take_along_axis(dots, idx[..., None], -1)[..., 0]
def signcode(z): 
    c = np.sign(z) / np.sqrt(z.shape[-1]); return c, (z * c).sum(-1)

def fail_plain(z, Dp, s, q, code):
    """one block, unit item z, unit rotated query Dp (batch, n) ; true score s; plain upper end < s  => fail."""
    x, cosf = code(z)
    ok = cosf > 1e-12; cs = np.where(ok, cosf, 1.0); tanf = np.sqrt(np.clip(1 - cs ** 2, 0, 1)) / cs
    est = (x * Dp[:, None, :]).sum(-1) / cs
    return ok & (s - est > q * tanf)
def fail_a4(z, Dp, s, q, code):
    x, cosf = code(z)
    ok = cosf > 1e-12; cs = np.where(ok, cosf, 1.0); tanf = np.sqrt(np.clip(1 - cs ** 2, 0, 1)) / cs
    k = q * q * tanf ** 2
    est = (x * Dp[:, None, :]).sum(-1) / cs / (1 + k)
    return ok & (s - est > q * tanf / np.sqrt(1 + k))

def stats(K, N):
    p = K.mean() / N; pi2 = (K * (K - 1)).mean() / (N * (N - 1))
    return p, pi2, (K >= 1).mean(), (K >= 2).mean()

def partA():
    n, N, delta, trials, batch = 13, 20, 0.01, 40000, 500
    d = n - 1; q = qd(d, delta)
    print(f"PART A one block n={n} N={N} delta={delta} q={q:.4f}; 'real' = fixed query, corpus on its latitude, one Haar matrix (QR) per trial applied to all items;")
    print("        'model' = Delta' uniform, items i.i.d. on the latitude of Delta'.  se of Pr[K>=1] about 0.002")
    print("  code  rule  s    | real: pbar   pi2       Pr[K>=1] Pr[K>=2] | model: pbar  pi2       Pr[K>=1] Pr[K>=2] | N pbar  N pbar-C(N,2)pi2  eff.count  Pr[F2|F1]")
    for kind in ('M1', 'M8', 'M64', 'M512', 'sign'):
        if kind == 'sign': code = signcode
        else:
            C = unit(rng.standard_normal((int(kind[1:]), n))); code = (lambda C: (lambda z: nearest(z, C)))(C)
        for rule, fn in (('plain', fail_plain), ('A4', fail_a4)):
            for s in (0.0, 0.3):
                if rule == 'A4' and kind in ('M1', 'M8'): continue
                Kr = []; Km = []
                for _ in range(trials // batch):
                    Q = haar(batch, n)
                    e1 = np.zeros((batch, n)); e1[:, 0] = 1
                    y = s * e1[:, None, :] + np.sqrt(1 - s * s) * perp_unit(batch, N, e1)
                    z = np.einsum('bij,bkj->bki', Q, y); Dp = Q[:, :, 0]
                    Kr.append(fn(z, Dp, s, q, code).sum(1))
                    Dp = unit(rng.standard_normal((batch, n)))
                    z = s * Dp[:, None, :] + np.sqrt(1 - s * s) * perp_unit(batch, N, Dp)
                    Km.append(fn(z, Dp, s, q, code).sum(1))
                Kr = np.concatenate(Kr).astype(float); Km = np.concatenate(Km).astype(float)
                a = stats(Kr, N); b = stats(Km, N)
                print(f"  {kind:5s} {rule:5s} {s:.1f}  | {a[0]:.5f} {a[1]:.2e}  {a[2]:.4f}   {a[3]:.4f}   | {b[0]:.5f} {b[1]:.2e}  {b[2]:.4f}   {b[3]:.4f}   | {N*a[0]:.4f}  {N*a[0]-N*(N-1)/2*a[1]:.4f}           {a[2]/a[0]:.1f}     {a[1]/a[0]:.4f}")

def partB():
    n, delta, trials = 13, 0.01, 2000000
    d = n - 1; q = qd(d, delta)
    print(f"\nPART B literal statement 'independent given Delta'' with a codebook that is random and shared (part of the seed). Delta' fixed = e1, equator, plain rule.")
    for M in (1, 2, 8):
        f1 = f2 = f12 = 0
        for _ in range(trials // 100000):
            b = 100000
            Dp = np.zeros((b, n)); Dp[:, 0] = 1
            z = perp_unit(b, 2, Dp)
            C = unit(rng.standard_normal((b, M, n)))
            dots = np.einsum('bkn,bmn->bkm', z, C); idx = dots.argmax(-1)
            cosf = np.take_along_axis(dots, idx[..., None], -1)[..., 0]
            x = np.take_along_axis(C, idx[..., None].repeat(n, -1), 1)
            ok = cosf > 1e-12; cs = np.where(ok, cosf, 1.0); sinf = np.sqrt(np.clip(1 - cs ** 2, 0, 1))
            F = ok & (-(x[..., 0]) > q * sinf)
            f1 += F[:, 0].sum(); f2 += F[:, 1].sum(); f12 += (F[:, 0] & F[:, 1]).sum()
        p1 = f1 / trials; p12 = f12 / trials
        print(f"  M={M}: Pr[F1|D']={p1:.5f}  Pr[F1 and F2|D']={p12:.6f}  product={p1*f2/trials:.6f}  ratio={p12/(p1*f2/trials):.1f}   (1/4 T_(d+1)(q)={0.25*float(T(d+1,q)):.6f})")

def partC():
    # two blocks, two levels, separate Haar rotations shared by all items; plain rule with sd over blocks
    n1, n2, N, trials, batch = 6, 8, 20, 200000, 5000
    d1, d2 = n1 - 1, n2 - 1
    D1, D2 = 0.95, 0.3            # ||Delta_1||, ||Delta_2||
    r1, r2 = 0.87, 0.5            # block norms
    s1, s2 = 0.10, -0.05         # block scores (latitudes)
    cst = 2.0                    # confidence constant in sd units (no exact delta needed for this test)
    lev = np.array([-1.5, -0.5, 0.5, 1.5])
    def grid(z):
        g = lev / np.sqrt(z.shape[-1]); c = g[np.abs(z[..., None] - g).argmin(-1)]; c = unit(c); return c, (z * c).sum(-1)
    def one(z, Dp, code):        # z unit direction (b,N,n), Dp rotated query direction (b,n): estimate of dir.dir and tan
        x, cosf = code(z); return (x * Dp[:, None, :]).sum(-1) / cosf, np.sqrt(1 - cosf ** 2) / cosf
    def fails(z1, z2, Dp1, Dp2):
        t = s1 + s2
        # level 1: block 1 by the sign code, block 2 uncoded (Cauchy-Schwarz)
        e1, t1 = one(z1, Dp1, signcode)
        up1 = r1 * D1 * e1 + 0.9 * np.sqrt((r1 * D1 * t1) ** 2 / d1) + r2 * D2
        # level 2: block 1 by the 2-bit grid, block 2 by the sign code
        g1, tg1 = one(z1, Dp1, grid); e2, t2 = one(z2, Dp2, signcode)
        up2 = r1 * D1 * g1 + r2 * D2 * e2 + cst * np.sqrt((r1 * D1 * tg1) ** 2 / d1 + (r2 * D2 * t2) ** 2 / d2)
        return (up1 < t) | (up2 < t), (up1 < t), (up2 < t)
    c1, c2 = s1 / (r1 * D1), s2 / (r2 * D2)   # cosines to the query in each block
    Kr = []; Km = []; l1 = l2 = 0
    for _ in range(trials // batch):
        Q1 = haar(batch, n1); Q2 = haar(batch, n2)
        e = np.zeros((batch, n1)); e[:, 0] = 1; y1 = c1 * e[:, None, :] + np.sqrt(1 - c1 ** 2) * perp_unit(batch, N, e)
        f = np.zeros((batch, n2)); f[:, 0] = 1; y2 = c2 * f[:, None, :] + np.sqrt(1 - c2 ** 2) * perp_unit(batch, N, f)
        F, a, b = fails(np.einsum('bij,bkj->bki', Q1, y1), np.einsum('bij,bkj->bki', Q2, y2), Q1[:, :, 0], Q2[:, :, 0])
        Kr.append(F.sum(1)); l1 += a.sum(); l2 += b.sum()
        Dp1 = unit(rng.standard_normal((batch, n1))); Dp2 = unit(rng.standard_normal((batch, n2)))
        z1 = c1 * Dp1[:, None, :] + np.sqrt(1 - c1 ** 2) * perp_unit(batch, N, Dp1)
        z2 = c2 * Dp2[:, None, :] + np.sqrt(1 - c2 ** 2) * perp_unit(batch, N, Dp2)
        Km.append(fails(z1, z2, Dp1, Dp2)[0].sum(1))
    Kr = np.concatenate(Kr).astype(float); Km = np.concatenate(Km).astype(float)
    a = stats(Kr, N); b = stats(Km, N)
    print(f"\nPART C two blocks (n={n1},{n2}), two levels, plain rule c={cst} sd, N={N}, latitudes s=({s1},{s2}); per-level failure rates {l1/(trials*N):.5f} {l2/(trials*N):.5f}")
    print(f"  real : pbar={a[0]:.5f} pi2={a[1]:.3e} Pr[K>=1]={a[2]:.4f} Pr[K>=2]={a[3]:.4f}   hist K=0..4: {[round(float((Kr==k).mean()),4) for k in range(5)]}")
    print(f"  model: pbar={b[0]:.5f} pi2={b[1]:.3e} Pr[K>=1]={b[2]:.4f} Pr[K>=2]={b[3]:.4f}   hist K=0..4: {[round(float((Km==k).mean()),4) for k in range(5)]}")
    print(f"  N pbar={N*a[0]:.4f}  N pbar - C(N,2) pi2={N*a[0]-N*(N-1)/2*a[1]:.4f}  independent items would give {1-(1-a[0])**N:.4f}")

def partD():
    # random shared codebook, fresh per seed: pi2 and the excess Pr[F2|F1] - pbar, against Lemma C2's bound and the referee's refinement
    from scipy.integrate import quad
    from scipy.special import gammaln
    n = 13; d = n - 1
    print("\nPART D random shared codebook (fresh per seed), equator, plain rule, n=13: excess e = Pr[F2|F1] - pbar")
    print("  delta  M     pbar     Pr[F2|F1]  e(measured)  se     | C2 bound on e (best theta)  refined bound on e (best theta) | 1/M")
    for delta, seeds, K in ((0.01, 6000, 600), (0.002, 8000, 600)):
        q = qd(d, delta)
        for M in (8, 64, 512, 4096):
            if M == 4096: sds, KK = seeds // 4, K // 2
            else: sds, KK = seeds, K
            S = []
            b = max(1, int(5e6 // (KK * M)))
            for _ in range(sds // b):
                Dp = unit(rng.standard_normal((b, n))); z = perp_unit(b, KK, Dp)
                C = unit(rng.standard_normal((b, M, n)))
                dots = z @ C.transpose(0, 2, 1); idx = dots.argmax(-1)
                cosf = np.take_along_axis(dots, idx[..., None], -1)[..., 0]
                tau = -np.einsum('bmn,bn->bm', C, Dp)
                tsel = np.take_along_axis(tau, idx, 1)
                F = (cosf > 1e-12) & (tsel > q * np.sqrt(np.clip(1 - cosf ** 2, 0, 1)))
                S.append(F.sum(1))
            S = np.concatenate(S).astype(float)
            pb = S.mean() / KK; pi2s = S * (S - 1) / (KK * (KK - 1)); pi2 = pi2s.mean()
            cond = pi2 / pb; e = cond - pb; se = pi2s.std() / np.sqrt(len(S)) / pb
            # bounds, optimised over theta
            cap = lambda dd, th: 0.5 * betainc(dd / 2, 0.5, np.sin(th) ** 2)
            lk = gammaln((d + 1) / 2) - gammaln(d / 2) - 0.5 * np.log(np.pi)
            best1 = best2 = 1e9
            for th in np.linspace(0.05, np.pi / 2 - 0.01, 300):
                Lam = M * cap(d, th); first = 2 * (1 - cap(d, th)) ** M
                b1 = (first + Lam ** 2 * delta ** 2 + Lam * delta * cap(d - 1, th)) / delta - delta
                integ, _ = quad(lambda ph: np.exp(lk) * np.sin(ph) ** (d - 1) * float(T(d, np.cos(th) / np.sqrt(1 - q * q * np.sin(ph) ** 2))), 0, th)
                b2 = (first + Lam ** 2 * delta ** 2 + M * delta * integ) / delta - delta
                best1 = min(best1, b1); best2 = min(best2, b2)
            print(f"  {delta:<6} {M:<5d} {pb:.5f}  {cond:.5f}    {e:+.5f}   {se:.5f} |  {best1:.4f}                      {best2:.4f}                      | {1/M:.5f}")

for p in sys.argv[1:]:
    {'A': partA, 'B': partB, 'C': partC, 'D': partD}[p]()
