# Study of tighter randomised-level bounds on a verify dump (VEXACT_DUMP, 50k index, four randomised levels).
# Record: tag (qi*64 + size*2 + plan), row, level, t, truth-t, est-t, hn (all but the random term), sd, then per block
# j: coded, a_j = A_j nd_j (eps * nd), R_j = |y_j| nd_j, est_j (kappa * a).
# For each alternative half-width the first randomised level that decides each item is found; records exist while
# the current bound keeps an item, so a bound no wider than the current one is simulated exactly.
import sys, numpy as np, math
from scipy.special import betainc, ive, gammaln
from scipy.optimize import brentq
p = sys.argv[1]
raw = np.fromfile(p, dtype=np.float32).reshape(-1, 24)
tag = raw[:, 0].view(np.uint32); row = raw[:, 1].view(np.uint32)
keep = (tag & 1) == 1          # the index plan only (no duplicates)
raw, tag, row = raw[keep], tag[keep], row[keep]
lv = raw[:, 2].astype(np.int64); est = raw[:, 5].astype(np.float64); hn = raw[:, 6].astype(np.float64); sd = raw[:, 7].astype(np.float64)
tr = raw[:, 4].astype(np.float64)
cod = raw[:, 8::4] > 0; a = raw[:, 9::4].astype(np.float64); R = raw[:, 10::4].astype(np.float64); ej = raw[:, 11::4].astype(np.float64)
D = np.array([95, 127, 255, 287], float)
L1 = 43.525973215572655; CSG = math.sqrt(2 * L1)
lnd1 = -(L1 - math.log(2))       # one-sided per (item, level): delta = eps/(N L)
c1 = math.sqrt(-2 * lnd1)
qi = tag >> 6; size = (tag >> 1) & 31; text = qi >= 10
print(f"records {len(raw)}; c two-sided {CSG:.3f}, one-sided {c1:.3f}")
# key per item run
key = tag.astype(np.int64) << 32 | row.astype(np.int64)
order = np.lexsort((lv, key)); 
def sim(hl, hu, name):
    """hl, hu: lower and upper half-widths (inf: undecided). Items entering each randomised level and reaching full."""
    dec = (est - hl >= 0) | (est + hu < 0)
    # first deciding level per item
    k = key[order]; d = dec[order]; l = lv[order]
    first = np.r_[True, k[1:] != k[:-1]]
    gid = np.cumsum(first) - 1
    ng = gid[-1] + 1
    fl = np.full(ng, 9, np.int64)
    np.minimum.at(fl, gid[d], l[d])
    start = l[first]                # level the item entered the randomised part at (0 normally)
    ent = [(start <= x) & (fl >= x) for x in range(4)]
    full = fl == 9
    t_ = text[order][first]
    out = {}
    for cls, m in (("item", ~t_), ("text", t_)):
        e = [int((x & m).sum()) for x in ent]; f = int((full & m).sum())
        out[cls] = (e, f)
    print(f"{name:34s} " + "  ".join(f"{c}: enter {out[c][0]} full {out[c][1]}" for c in out))
    return out
hcur = (CSG * sd + hn) * (1 + 1e-6)
base = sim(hcur, hcur, "current")
# check that the simulation reproduces the decisions: every record but an item's last is undecided under hcur
# components
for l in range(4):
    m = lv == l
    rnd = CSG * sd[m]; hd = (R[m] * ~cod[m]).sum(1)
    print(f"level {l}: mean hh {hcur[m].mean():.4f}; random {rnd.mean():.4f}; uncoded-block CS {hd.mean():.4f}; other det {(hn[m]-hd).mean():.4f}; "
          f"share of var by block {np.round((a[m]**2/D).mean(0)/ (a[m]**2/D).mean(0).sum(),3)}")
sim((c1 * sd + hn) * (1 + 1e-6), (c1 * sd + hn) * (1 + 1e-6), "one-sided c")
# exact tail, one block coded: s* with P(U >= s) = delta (U a coordinate of uniform on S^{d-1})
from scipy.special import betaincinv
def sstar(d, lnd):
    return math.sqrt(1 - betaincinv((d - 1) / 2, 0.5, 2 * math.exp(lnd)))
for d in D:
    print(f"d {d:.0f}: exact s* {sstar(d, lnd1):.4f} vs c/sqrt(d) {c1/math.sqrt(d):.4f} (two-sided c {CSG/math.sqrt(d):.4f}); ratio {sstar(d, lnd1)*math.sqrt(d)/CSG:.3f}")
ncod = cod.sum(1)
single = ncod == 1
hx = hcur.copy()
jj = np.argmax(cod, 1)
ss = np.array([sstar(d, lnd1) for d in D])
hx[single] = (ss[jj[single]] * a[single, jj[single]] + hn[single]) * (1 + 1e-6)
print(f"single-block records {single.sum()} of {len(single)}")
sim(hx, hx, "exact tail where one block")
# Chernoff with the exact MGF of each coordinate, lambda on a grid around c/sigma (sample of records)
def lnmgf(d, x):
    nu = d / 2 - 1
    x = np.maximum(x, 1e-12)
    return gammaln(nu + 1) - nu * np.log(x / 2) + np.log(ive(nu, x)) + x
def chern(av, lnd):
    s = np.sqrt((av ** 2 / D).sum(1)); best = np.full(len(av), np.inf)
    lam0 = math.sqrt(-2 * lnd) / np.maximum(s, 1e-30)
    for g in np.geomspace(0.6, 12, 80):
        lam = lam0 * g
        v = -lnd + sum(lnmgf(D[j], lam * av[:, j]) * (av[:, j] > 0) for j in range(4))
        best = np.minimum(best, v / lam)
    return best
hc = hcur.copy()
# all records (vectorised in chunks)
for i in range(0, len(a), 2_000_000):
    sl = slice(i, i + 2_000_000)
    hc[sl] = np.minimum((chern(a[sl], lnd1) + hn[sl]) * (1 + 1e-6), hcur[sl])
r = (hc - hn) / np.maximum(c1 * sd, 1e-30)
for l in range(4):
    print(f"level {l}: Chernoff exact-MGF random term / one-sided sub-Gaussian: mean {r[lv==l].mean():.3f}, p10 {np.percentile(r[lv==l],10):.3f} p90 {np.percentile(r[lv==l],90):.3f}")
sim(hc, hc, "Chernoff exact MGF (one-sided)")
# Theorem B, self-normalised: (S-E)^2 <= c^2 (V - S^2/W)
def thmB(c, use_exact=False):
    E = (ej * cod).sum(1); V = (a ** 2 / D * cod).sum(1)
    W = np.where(cod, D * R ** 2 / np.maximum(a, 1e-30) ** 2, 0).sum(1)
    k2 = c * c / np.maximum(W, 1e-30)
    disc = c * c * V * (1 + k2) - k2 * E * E
    sq = np.sqrt(np.maximum(disc, 0))
    hu = (sq - k2 * E) / (1 + k2); hl = (sq + k2 * E) / (1 + k2)
    bad = disc < 0
    hu = np.where(bad, np.inf, hu); hl = np.where(bad, np.inf, hl)
    return hl, hu, k2
hl, hu, k2 = thmB(c1)
for l in range(4):
    m = lv == l
    print(f"level {l}: kappa^2 median {np.median(k2[m]):.4f} p90 {np.percentile(k2[m],90):.4f}; E/sqrt(VW) median {np.median(np.abs((ej*cod).sum(1)[m])/np.sqrt((a**2/D*cod).sum(1)[m]*np.where(cod, D*R**2/np.maximum(a,1e-30)**2,0).sum(1)[m])):.4f}")
HL = np.minimum((hl + hn) * (1 + 1e-6), hcur); HU = np.minimum((hu + hn) * (1 + 1e-6), hcur)
sim(HL, HU, "Theorem B (one-sided c)")
sim(np.minimum(HL, hc), np.minimum(HU, hc), "min(Theorem B, Chernoff) [needs 2x delta]")
# tolerance d (score units): in when lb >= t - d
for dd in (0.001, 0.003, 0.01):
    hl2 = np.where(est - hcur >= -dd, 0.0, hcur)   # an item whose lower bound clears t - d is taken in
    sim(np.where(est - hcur >= -dd, -np.inf, hcur), hcur, f"tolerance d={dd}") if False else None
    dec_in = est - hcur >= -dd
    sim(np.where(dec_in, -1e9, 1e9) * 0 + np.where(dec_in, est, hcur), hcur, f"tolerance d={dd}")
