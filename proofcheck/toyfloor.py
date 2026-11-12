# Toy checks of the volume floor (Theorem A / T5), with exact mean half-widths.
import numpy as np, math
from scipy import stats, optimize
from scipy.spatial import ConvexHull
rng = np.random.default_rng(0)
def Eg(k): return math.sqrt(2) * math.exp(math.lgamma((k + 1) / 2) - math.lgamma(k / 2))
def lvol(k): return k / 2 * math.log(math.pi) - math.lgamma(k / 2 + 1)
def bound(hS, H, k): return Eg(k) * math.exp(-lvol(k) / k + (hS - H) / k)      # nats
# (1) Gaussian, entropy-coded infinite uniform scalar quantisers per coordinate, box certificate (exact support fn of a box).
def ecsq(sig, w, off):
    j = np.arange(-4000, 4001); e = (j + off) * w; p = np.diff(stats.norm.cdf(np.append(e, e[-1] + w) / sig)); p = p[p > 0]
    return -(p * np.log(p)).sum()
print("(1) Gaussian product ECSQ: min over tried codes of achieved/bound (max_k)")
worst = 9
for trial in range(4000):
    n = rng.integers(1, 5); sig = np.sort(np.exp(rng.normal(0, 1.5, n)))[::-1]; w = sig * np.exp(rng.uniform(-4, 2.5, n)); off = rng.uniform(0, 1, n)
    H = sum(ecsq(s, ww, o) for s, ww, o in zip(sig, w, off)); ach = Eg(1) * w.sum() / 2
    b = max(bound(0.5 * k * math.log(2 * math.pi * math.e) + np.log(sig[:k]).sum(), H, k) for k in range(1, n + 1))
    worst = min(worst, ach / b)
print("    worst ratio over 4000 random codes: %.7f (>=1 required)" % worst)
# 1-D high-rate tightness
for w in (1, .3, .05): print("    1-D w=%.2f: achieved/bound = %.4f" % (w, Eg(1) * w / 2 / bound(0.5 * math.log(2 * math.pi * math.e), ecsq(1, w, .5), 1)))
# (2) 1-D Gaussian, random-search over interval partitions with finitely many *bounded* cells on a truncated Gaussian: try to beat the bound.
print("(2) 1-D truncated Gaussian on [-L,L], arbitrary interval partitions, optimised to minimise achieved/bound")
L = 4.0; Z = stats.norm.cdf(L) - stats.norm.cdf(-L)
xs = np.linspace(-L, L, 200001); f = stats.norm.pdf(xs) / Z; hX = -np.trapezoid(f * np.log(f), xs)
def ratio(par):
    cuts = np.sort(np.clip(par, -L, L)); e = np.concatenate([[-L], cuts, [L]]); p = np.diff(stats.norm.cdf(e)) / Z; ln = np.diff(e)
    ok = p > 1e-300; H = -(p[ok] * np.log(p[ok])).sum(); return (Eg(1) * (p * ln).sum() / 2) / bound(hX, H, 1)
best = 9
for m in (1, 2, 3, 5, 8, 15):
    for t in range(15):
        res = optimize.minimize(ratio, rng.uniform(-L, L, m), method="Nelder-Mead", options={"maxiter": 4000, "xatol": 1e-6, "fatol": 1e-9}); best = min(best, res.fun)
    print("    %2d cuts: best ratio so far %.4f" % (m, best))
# (3) 2-D: uniform on a rectangle [0,a]x[0,1], Gaussian g, cells = Voronoi regions of random sites and non-convex unions of them; exact via hull perimeter.
print("(3) 2-D uniform on rectangle, Voronoi cells (and random merges -> non-convex cells); k=2 and k=1 bounds")
G = 300
worst2 = 9
for trial in range(300):
    a = math.exp(rng.uniform(-2, 2)); m = rng.integers(2, 40); P = np.stack(np.meshgrid((np.arange(G) + .5) / G * a, (np.arange(G) + .5) / G), -1).reshape(-1, 2)
    sites = rng.uniform(0, 1, (m, 2)) * [a, 1]; scale = np.exp(rng.uniform(-1, 1, 2))
    lab = (((P[:, None, :] - sites[None]) * scale) ** 2).sum(2).argmin(1)
    if trial % 2: lab = rng.integers(0, max(2, m // 3), m)[lab]              # merge cells at random: non-convex / disconnected
    ach = 0; H = 0
    for c in np.unique(lab):
        pts = P[lab == c]; p = len(pts) / len(P); H -= p * math.log(p)
        # pixel corners so the hull contains the cell
        d = np.array([[-.5 * a / G, -.5 / G], [.5 * a / G, -.5 / G], [-.5 * a / G, .5 / G], [.5 * a / G, .5 / G]]); allp = (pts[:, None] + d[None]).reshape(-1, 2)
        hull = ConvexHull(allp); per = hull.area                                  # in 2-D .area is the perimeter
        ach += p * Eg(2) * per / (2 * math.pi)
    b2 = bound(math.log(a), H, 2); b1 = max(bound(math.log(a), H, 1), bound(0.0, H, 1))
    worst2 = min(worst2, ach / max(b1, b2))
print("    worst achieved/bound over 300 partitions: %.4f (>=1 required)" % worst2)
# hexagonal-ish sanity: a disc cell of the same area would give ratio 1
# (4) Concentration bound (Theorem B) per-cell inequality on balls: rho*E|Gam^.5 g| >= sum sqrt(gam) - sqrt(sum gam)(sqrt(2 ln 1/P)+1)
print("(4) Theorem B per-cell inequality on centred / off-centre balls in whitened coordinates (isotropic gamma=1)")
w4 = 9e9
for n in (5, 50, 500):
    for rho in np.linspace(0.2, 2.0, 30) * math.sqrt(n):
        for shift in (0, 1.0, 3.0):
            Pm = stats.ncx2.cdf(rho * rho, n, shift * shift)
            if Pm <= 0: continue
            lhs = rho * Eg(n); rhs = n - math.sqrt(n) * (math.sqrt(2 * math.log(1 / Pm)) + 1); w4 = min(w4, lhs - rhs)
print("    min(lhs - rhs) = %.4f (>=0 required)" % w4)
