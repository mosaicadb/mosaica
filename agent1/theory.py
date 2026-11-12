# Numerical evaluation of the proved floors (Theorems A, B, C of REPORT.md), the conjectured floor of FINDINGS section 5,
# and the measured mean half-width of two codes on Gaussian data with the measured spectra. Then the sharper constant c.
import numpy as np, json, math, sys
rng = np.random.default_rng(1)
spec = json.load(open(sys.argv[1] if len(sys.argv) > 1 else "spec_v1.json"))
def kappa(k):                                   # E|g_k| sqrt(2 pi e) / vol(B_k)^(1/k)
    Eg = math.sqrt(2) * math.exp(math.lgamma((k + 1) / 2) - math.lgamma(k / 2))
    return Eg * math.sqrt(2 * math.pi * math.e) / math.exp((k / 2 * math.log(math.pi) - math.lgamma(k / 2 + 1)) / k)
assert all(kappa(k) >= k for k in range(1, 6000))
def floors(gam, B):                             # gam sorted descending, B in bits
    n = len(gam); s = np.sqrt(gam); L = np.cumsum(np.log(s)); ks = np.arange(1, n + 1); kap = np.array([kappa(k) for k in ks])
    head = np.exp(L / ks - B * math.log(2) / ks)                  # GM_k(sqrt gamma) 2^(-B/k)
    A = float((kap * head).max())
    suf1 = np.concatenate([np.cumsum(s[::-1])[::-1], [0]]); suf2 = np.concatenate([np.cumsum(gam[::-1])[::-1], [0]])   # sums over i >= j
    tail = lambda bits: suf1 - (math.sqrt(2 * bits * math.log(2)) + 1) * np.sqrt(suf2)
    Bb = float(tail(B).max())
    tC = tail(B + 1); best_tail_from = np.maximum.accumulate(tC[::-1])[::-1]            # best suffix starting at >= j
    C = float(max(Bb, (kap / math.sqrt(math.e) * np.exp(L / ks - (B + 1) * math.log(2) / ks) + np.maximum(best_tail_from[1:], 0)).max()))
    lo, hi = 1e-30, gam[0]
    for _ in range(200):
        th = math.sqrt(lo * hi)
        if 0.5 * np.log2(np.maximum(gam / th, 1)).sum() > B: lo = th
        else: hi = th
    return A, Bb, C, float(np.sqrt(np.minimum(gam, th)).sum())

# ---- the two codes, as in h.py ----
STEP = {2: 0.9957, 3: 0.5860, 4: 0.3352, 5: 0.1881, 6: 0.1041, 7: 0.0569, 8: 0.0308}
GT = np.array([1, .3634, .1188, .03744, .01154, .00349, .00104, .0003043, .0000876])
def quant_uni(Z, bits):
    out = np.zeros_like(Z)
    for b in np.unique(bits):
        if b == 0: continue
        cols = np.where(bits == b)[0]; z = Z[:, cols]
        if b == 1: out[:, cols] = np.where(z >= 0, 0.7979, -0.7979)
        else:
            s = STEP[int(b)]; Lc = 2 ** int(b); out[:, cols] = (np.clip(np.floor(z / s), -Lc // 2, Lc // 2 - 1) + 0.5) * s
    return out
W = 0.6745; EMAXD = 16
gE = np.array([1.0, 0.3634, 0.03653] + [W * W / 12 * 4.0 ** (2 - j) for j in range(3, EMAXD + 2)]); cE = np.ones(EMAXD + 1); cE[1] = 1.728
def recon_esc(U, dep):
    a = np.abs(U); wj = W * np.exp2(2.0 - np.maximum(dep, 2)); val = (np.floor(a / wj) + 0.5) * wj
    val[dep == 1] = 0.7979; val[dep == 0] = 0
    return np.sign(U) * val
def steps(lam, Qe, g, cost, total, maxd):
    b = np.zeros(len(lam), int); a = np.sqrt(Qe * lam); sg = np.sqrt(g); seq = []; spent = 0.0
    gain = a * (sg[0] - sg[1]) / cost[0]
    while spent < total and gain.max() > 0:
        i = int(gain.argmax()); seq.append(i); spent += cost[b[i]]; b[i] += 1
        gain[i] = a[i] * (sg[b[i]] - sg[b[i] + 1]) / cost[b[i]] if b[i] < maxd else -1
    return np.array(seq)
def measure(lam, Qe, BS, NR=1500, ND=300):
    n = len(lam); sig = np.sqrt(lam); u = rng.standard_normal((NR, n)); z = u * sig; D2 = (rng.standard_normal((ND, n)) ** 2) * Qe
    dn = np.sqrt(D2.sum(1)); out = {}
    su = steps(lam, Qe, GT, np.ones(9), max(BS), 8); se = steps(lam, Qe, gE, cE, 1.5 * max(BS), EMAXD)
    dep_at = np.zeros((len(se) + 1, n), np.int8)
    for s_, c_ in enumerate(se): dep_at[s_ + 1] = dep_at[s_]; dep_at[s_ + 1, c_] += 1
    sdep = dep_at[np.arange(1, len(se) + 1), se]; expc = np.cumsum(cE[sdep - 1])
    C = np.ones((NR, len(se)), np.int32); s2 = np.where(sdep == 2)[0]; C[:, s2] = np.floor(np.abs(u[:, se[s2]]) / W).astype(int) + 1
    cum = np.cumsum(C, 1)
    for B in BS:
        row = []
        for k in ("uni", "esc"):
            if B == 0: c = np.zeros_like(z); ev = lam
            elif k == "uni": bits = np.bincount(su[:B], minlength=n); c = quant_uni(u, bits) * sig; ev = lam * GT[bits]
            else: c = recon_esc(u, dep_at[(cum <= B).sum(1)]) * sig; ev = lam * gE[dep_at[int((expc <= B).sum())]]
            gm = (z * c).sum(1) / ((c * c).sum(1) + 1e-30); e = z - gm[:, None] * c
            Dv2 = np.sqrt((ev + 1e-12) / (Qe + 1e-12)); dw = np.sqrt((D2 * Dv2).sum(1))
            h = np.minimum(np.outer(np.linalg.norm(e, axis=1), dn), np.outer(np.sqrt((e * e / Dv2).sum(1)), dw))
            row.append(float(h.mean()))
        out[B] = row
    return out
BS = [0, 32, 64, 128, 256, 768, 1536, 3072, 4608]; SCAL = 48          # three stored scalars at 16 bits each
for g in sorted(spec, key=lambda g: -spec[g]["items"])[:2]:
    lam = np.array(spec[g]["lam"]); Qe = np.array(spec[g]["Qe"]); gam = np.sort(lam * Qe)[::-1]; n = len(gam)
    m = measure(lam, Qe, BS)
    print(f"group {g}: {spec[g]['items']} items, n = {n}; sum sqrt(gamma) = {np.sqrt(gam).sum():.4f}; mean half-width on Gaussian data (independent offsets)")
    print(" code bits | proved A (volume) | proved B (concentration) | proved C (both) | best proved | conjectured sum sqrt(min(gamma,theta)) | uniform | escape | uniform/proved | escape/proved | escape/conjectured")
    for B in BS:
        A, Bb, C, cj = floors(gam, B + SCAL); best = max(A, Bb, C); mu, me = m[B]
        assert mu >= best and me >= best, "a code beat a proved floor"
        print(f" {B:5d}     | {A:.4f} | {Bb:.4f} | {C:.4f} | {best:.4f} | {cj:.4f} | {mu:.4f} | {me:.4f} | {mu/best:.2f} | {me/best:.2f} | {me/cj:.2f}")
# dimension: the same floors for a power-law spectrum gamma_i = i^-alpha scaled to the same total, at bits = n and 4n
print("power-law spectra gamma_i ~ i^-1.5: proved/conjectured ratio at B = n bits and B = 4n bits")
for n in (100, 256, 768, 1024, 5000):
    gam = np.arange(1, n + 1) ** -1.5; r = []
    for B in (n, 4 * n):
        A, Bb, C, cj = floors(gam, B); r.append(max(A, Bb, C) / cj)
    print(f"  n = {n:5d}: {r[0]:.3f} {r[1]:.3f}")

# ---- sharper constant for the randomised certificate ----
def logtail(m, s, npts=400_001):                # log P(|u_1| > s), u uniform on the unit sphere of R^m
    x = np.linspace(s, 1, npts)[:-1] + (1 - s) / (2 * (npts - 1)); lf = (m - 3) / 2 * np.log1p(-x * x)
    lc = math.lgamma(m / 2) - 0.5 * math.log(math.pi) - math.lgamma((m - 1) / 2)
    M = lf.max(); return math.log(2) + lc + M + math.log(np.exp(lf - M).sum() * (1 - s) / (npts - 1))
def logmgf(m, lam):                             # log E exp(lam u_1) = log 0F1(; m/2; lam^2/4)
    j = np.arange(0, 400); lt = j * math.log(lam * lam / 4 + 1e-300) - np.array([math.lgamma(i + 1) for i in j]) - np.array([math.lgamma(m / 2 + i) - math.lgamma(m / 2) for i in j])
    M = lt.max(); return M + math.log(np.exp(lt - M).sum())
def chernoff_x(blocks, logp):                   # smallest x with 2 inf_l exp(-l x + sum log M(l a_b)) <= p
    def lb(x): return math.log(2) + min(-l * x + sum(logmgf(m, l * a) for m, a in blocks) for l in np.linspace(0.5, 400, 500))
    lo, hi = 0.0, sum(a for _, a in blocks)
    for _ in range(40):
        mid = (lo + hi) / 2
        if lb(mid) > logp: lo = mid
        else: hi = mid
    return hi
print("one rotated block of m dimensions: c' such that the exact tail at c'/sqrt(m) equals 2 exp(-c^2/2); ratio c'/c")
for m in (23, 30, 40, 68, 128, 250, 767):
    out = []
    for c in (6.0, 8.0, 10.0):
        lp = math.log(2) - c * c / 2; lo, hi = 0.0, 1.0
        for _ in range(60):
            mid = (lo + hi) / 2
            if logtail(m, mid) > lp: lo = mid
            else: hi = mid
        out.append(hi * math.sqrt(m) / c)
    print(f"  m = {m:4d}: " + " ".join(f"{v:.3f}" for v in out))
print("several independent blocks with equal variance (exact moment generating function, Chernoff): ratio of half-widths to the sub-Gaussian one")
for name, ms in (("96 B: 30,39,31,28", (29, 38, 30, 27)), ("192 B: 32,43,46,41,41,32,25,68", (31, 42, 45, 40, 40, 31, 24, 67))):
    blocks = [(m, math.sqrt(m)) for m in ms]; sd = math.sqrt(len(ms)); out = []
    for c in (6.0, 8.0, 10.0): out.append(chernoff_x(blocks, math.log(2) - c * c / 2) / (c * sd))
    print(f"  {name}: " + " ".join(f"{v:.3f}" for v in out))
