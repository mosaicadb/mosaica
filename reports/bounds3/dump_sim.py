# Effect of the constants on a verify dump (50k BioCLIP index, four randomised levels, n = 768; records exist while
# the 9.33 bound keeps an item, so any bound no wider than that is simulated exactly).
# For each variant: items entering each randomised level, exact (full-vector) reads, and bytes read from the first
# randomised level on (level increments 29.5 / 41.0 / 67.7 / 101.1 B, full vector 1,536 B).
import numpy as np, math, sys
from scipy.interpolate import RegularGridInterpolator
from law import c1
p = sys.argv[1]
raw = np.fromfile(p, dtype=np.float32).reshape(-1, 24)
tag = raw[:, 0].view(np.uint32); row = raw[:, 1].view(np.uint32)
keep = (tag & 1) == 1
raw, tag, row = raw[keep], tag[keep], row[keep]
lv = raw[:, 2].astype(np.int64); est = raw[:, 5].astype(np.float64); hn = raw[:, 6].astype(np.float64); sd = raw[:, 7].astype(np.float64)
tr = raw[:, 4].astype(np.float64)
cod = raw[:, 8::4] > 0; a = raw[:, 9::4].astype(np.float64); R = raw[:, 10::4].astype(np.float64); ej = raw[:, 11::4].astype(np.float64)
D = np.array([95, 127, 255, 287], float)
qi = tag >> 6; size = (tag >> 1) & 31; text = qi >= 10
key = tag.astype(np.int64) << 32 | row.astype(np.int64)
order = np.lexsort((lv, key))
k_ = key[order]; l_ = lv[order]
first = np.r_[True, k_[1:] != k_[:-1]]
gid = np.cumsum(first) - 1; ng = gid[-1] + 1
start = l_[first]; t_ = text[order][first]; sz_ = size[order][first]
truth_first = tr[order][first]
LB = np.array([29.5, 41.0, 67.7, 101.1]); FULL = 1536.0

def sim(hl, hu):
    dec = ((est - hl >= 0) | (est + hu < 0))[order]
    fl = np.full(ng, 9, np.int64)
    np.minimum.at(fl, gid[dec], l_[dec])
    ent = np.stack([(start <= x) & (fl >= x) for x in range(4)], 1)
    full = fl == 9
    # wrong decisions (never expected): decided in with truth < 0 or out with truth >= 0
    din = (est - hl >= 0)[order]; dout = (est + hu < 0)[order]
    trs = tr[order]
    wrong = int(((din & (trs < 0)) | (dout & (trs >= 0)))[dec & (l_ == fl[gid])].sum())
    return ent, full, wrong

def report(name, hl, hu, base=None, split=True):
    ent, full, wrong = sim(hl, hu)
    out = {}
    for cls, m in (("item", ~t_), ("text", t_)):
        e = ent[m].sum(0); f = int(full[m].sum())
        out[cls] = (e, f, float((e * LB).sum() + f * FULL))
    s = f"{name:44s}"
    for cls in ("item", "text"):
        e, f, b = out[cls]
        if base is None:
            s += f" | {cls}: enter {e.tolist()} full {f} bytes {b/1e6:.1f}M"
        else:
            be, bf, bb = base[cls]
            s += f" | {cls}: enter L1-3 {100*(e[1:].sum()/be[1:].sum()-1):+5.1f}% full {100*(f/bf-1):+6.1f}% bytes {100*(b/bb-1):+5.1f}%"
    print(s + (f"  WRONG {wrong}" if wrong else ""), flush=True)
    return out

LN = math.log(1e-9 / 1e9)
tabz = np.load("ctab_40.npz"); keys = list(tabz["keys"]); TAB = tabz["tab"]; K = int(tabz["K"])
g = np.arange(K + 1) / K
def c_item(keyname):
    """Exact per-record constant from the table (records with four coded blocks); nan elsewhere."""
    f = RegularGridInterpolator((g, g, g), TAB[..., keys.index(keyname)])
    v = a ** 2 / D * cod; w = v / np.maximum(v.sum(1, keepdims=True), 1e-300)
    out = np.full(len(a), np.nan)
    m = cod.all(1)
    out[m] = f(np.clip(w[m, :3], 0, 1))
    return out
def c_sup(keyname):
    return float(TAB[..., keys.index(keyname)].max())
single = cod.sum(1) == 1
four = cod.all(1)
assert (single | four).all()
def widths(c_single, c_four):
    """Half-width with constants per record (arrays or scalars)."""
    c = np.where(single, c_single, c_four)
    return (c * sd + hn) * (1 + 1e-6)
CSG = math.sqrt(2 * 43.525973215572655)
h0 = widths(CSG, CSG)
print(f"records {len(raw)}; items x queries {ng}; item {int((~t_).sum())} text {int(t_.sum())}")
base = report("A current: 9.330 everywhere", h0, h0)
hB = widths(7.413, 9.086); bB = report("B commit c25333fa9: 7.413 / 9.086", hB, hB)
print("--- relative to B ---")
report("A current 9.330", h0, h0, bB)
c1_two = c1(95, -43.525973215572655); c1_one = c1(95, LN - math.log(4))
hC = widths(c1_two, 8.775); report(f"C exact worst-case, two-sided: {c1_two:.3f} / 8.775", hC, hC, bB)
c4_one = c_sup("one") + 0.009
hD = widths(c1_one, c4_one); bD = report(f"D C + harmful side only: {c1_one:.3f} / {c4_one:.3f}", hD, hD, bB)
ci = c_item("one")
print("   per-item constant (one-sided) by level: " + "; ".join(f"L{l}: mean {np.nanmean(ci[lv==l]):.3f} p10 {np.nanpercentile(ci[lv==l],10):.3f} p90 {np.nanpercentile(ci[lv==l],90):.3f}" for l in (1, 2, 3)))
hE = widths(c1_one, np.where(four, ci, 0)); report("E D with the item's own exact constant", hE, hE, bB)
# level splits: (pi_0, pi_1, pi_2, pi_3) of eps / N, one-sided
def split(pis, per_item=False, name=None):
    cs1 = c1(95, LN + math.log(pis[0]))
    c4 = np.zeros(len(a))
    for l in (1, 2, 3):
        kn = {0.9: "p90", 0.7: "p70", 0.5: "p50", 0.25: "one", 0.1: "p10", 1 / 30: "p033", 0.05: "p05", 0.02: "p02", 0.01: "p01", 1.0: "L1"}[pis[l]]
        c4[lv == l] = (c_item(kn)[lv == l] if per_item else c_sup(kn) + 0.009)
    h = widths(cs1, c4)
    return report(name or f"split {pis}{' per-item' if per_item else ''}", h, h, bB)
split((0.1, 0.1, 0.1, 0.7))
split((0.05, 0.05, 0.9 - 0.1 + 0.1 - 0.9 + 0.5 - 0.1 + 0.0, 0.5)) if False else None
split((1 / 30, 1 / 30, 1 / 30, 0.9))
split((0.25, 0.1, 0.1, 0.5)) if False else None
split((0.1, 0.1, 0.1, 0.7), per_item=True)
# Theorem B (self-normalised, one-sided events) with the worst-case constants
def thmB(c_single, c_four):
    c = np.where(single, c_single, c_four)
    E = (ej * cod).sum(1); V = (a ** 2 / D * cod).sum(1)
    W = np.where(cod, D * R ** 2 / np.maximum(a, 1e-30) ** 2, 0).sum(1)
    k2 = c * c / np.maximum(W, 1e-30)
    disc = c * c * V * (1 + k2) - k2 * E * E
    sq = np.sqrt(np.maximum(disc, 0))
    hu = (sq - k2 * E) / (1 + k2); hl = (sq + k2 * E) / (1 + k2)
    Rsum = (R * cod).sum(1)
    bad = E * E > V * W
    hu = np.where(bad, np.maximum(Rsum - E, 0), hu); hl = np.where(bad, np.maximum(Rsum + E, 0), hl)
    plain = c * np.sqrt(V)
    return (np.minimum(hl, np.inf) + hn) * (1 + 1e-6), (hu + hn) * (1 + 1e-6), k2, E, V, W, plain
HL, HU, k2, E, V, W, plain = thmB(c1_one, c4_one)
for l in range(4):
    m = lv == l
    print(f"   level {l}: k = c^2/W median {np.median(k2[m]):.4f}; |E|/sqrt(VW) median {np.median(np.abs(E[m])/np.sqrt(V[m]*W[m])):.3f}; shift k|E|/(1+k) over plain width: median {np.median(k2[m]*np.abs(E[m])/(1+k2[m])/plain[m]):.3f}; hu/plain median {np.median((HU[m]-hn[m])/plain[m]):.3f} hl/plain {np.median((HL[m]-hn[m])/plain[m]):.3f}")
report("F D with Theorem B's interval", HL, HU, bB)
report("F' min(D, Theorem B) [two events per level]", np.minimum(HL, hD), np.minimum(HU, hD), bB)
# references that are not sound for the default path
c1_s = c1(95, math.log(1e-9 / 4)); c4_s = c_sup("single")
hS = widths(c1_s, c4_s); report(f"X single-item constant {c1_s:.2f} / {c4_s:.2f} (not sound)", hS, hS, bB)
hL = widths(c1(95, LN), c_sup("L1") + 0.009); report("Y one level's worth (L = 1) at every level (not sound)", hL, hL, bB)
# by answer size, variant D against B (bytes and full reads)
print("--- by answer size (index 0..6), D and E against B: full reads / bytes ---")
entB, fullB, _ = sim(hB, hB); entD, fullD, _ = sim(hD, hD); entE, fullE, _ = sim(hE, hE)
for cls, m0 in (("item", ~t_), ("text", t_)):
    for s in range(7):
        m = m0 & (sz_ == s)
        bb = (entB[m].sum(0) * LB).sum() + fullB[m].sum() * FULL
        bd = (entD[m].sum(0) * LB).sum() + fullD[m].sum() * FULL
        be = (entE[m].sum(0) * LB).sum() + fullE[m].sum() * FULL
        nmatch = int((truth_first[m] >= 0).sum())
        print(f"   {cls} size {s}: items reaching rand {int(m.sum())}; full B {int(fullB[m].sum())} D {100*(fullD[m].sum()/max(fullB[m].sum(),1)-1):+.1f}% E {100*(fullE[m].sum()/max(fullB[m].sum(),1)-1):+.1f}%; bytes D {100*(bd/bb-1):+.1f}% E {100*(be/bb-1):+.1f}%")
