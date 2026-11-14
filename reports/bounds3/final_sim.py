# The report's table of measured effects, on a verify dump of the 50k index (regenerate with the command in REPORT).
# Each row: items entering randomised levels 2-4 (96 / 160 / 256 B), exact (full-vector) reads and bytes read from the
# first randomised level on, relative to the committed constants (B: 7.413 where one block is coded, 9.086 where four).
import numpy as np, math, sys
from scipy.interpolate import RegularGridInterpolator
from law import c1
raw = np.fromfile(sys.argv[1], dtype=np.float32).reshape(-1, 24)
tag = raw[:, 0].view(np.uint32); row = raw[:, 1].view(np.uint32)
keep = (tag & 1) == 1; raw, tag, row = raw[keep], tag[keep], row[keep]
lv = raw[:, 2].astype(np.int64); est = raw[:, 5].astype(np.float64); hn = raw[:, 6].astype(np.float64); sd = raw[:, 7].astype(np.float64)
tr = raw[:, 4].astype(np.float64)
cod = raw[:, 8::4] > 0; a = raw[:, 9::4].astype(np.float64); R = raw[:, 10::4].astype(np.float64); ej = raw[:, 11::4].astype(np.float64)
D = np.array([95, 127, 255, 287], float)
qi = tag >> 6; text = qi >= 10
key = tag.astype(np.int64) << 32 | row.astype(np.int64)
order = np.lexsort((lv, key)); k_ = key[order]; l_ = lv[order]
first = np.r_[True, k_[1:] != k_[:-1]]; gid = np.cumsum(first) - 1; ng = gid[-1] + 1
start = l_[first]; t_ = text[order][first]
LB = np.array([29.5, 41.0, 67.7, 101.1]); FULL = 1536.0
single = cod.sum(1) == 1; four = cod.all(1)
v = a ** 2 / D * cod; V = v.sum(1); w = v / np.maximum(V[:, None], 1e-300)
def sim(hl, hu=None):
    hu = hl if hu is None else hu
    din = (est - hl >= 0); dout = (est + hu < 0)
    dec = (din | dout)[order]
    fl = np.full(ng, 9, np.int64); np.minimum.at(fl, gid[dec], l_[dec])
    ent = np.stack([(start <= x) & (fl >= x) for x in range(4)], 1); full = fl == 9
    wrong = int((((din & (tr < 0)) | (dout & (tr >= 0)))[order])[dec & (l_ == fl[gid])].sum())
    return {cls: (ent[m].sum(0), int(full[m].sum()), float((ent[m].sum(0) * LB).sum() + full[m].sum() * FULL)) for cls, m in (("item", ~t_), ("text", t_))}, wrong
def width(c_s, c_f, shrink=1.0):
    return (np.where(single, c_s, c_f) * sd / shrink + hn) * (1 + 1e-6)
B, _ = sim(width(7.413, 9.086))
print(f"records {len(raw)}; (item, query) pairs reaching the randomised levels: item {int((~t_).sum())}, text {int(t_.sum())}")
for cls in ("item", "text"):
    e, f, b = B[cls]; print(f"B committed 7.413 / 9.086, {cls}: enter {e.tolist()}, full {f}, bytes {b/1e6:.1f}M")
def rel(name, hl, hu=None):
    o, wrong = sim(hl, hu)
    s = f"{name:64s}"
    for c in ("item", "text"):
        s += f" | {c}: L2-4 {100*(o[c][0][1:].sum()/B[c][0][1:].sum()-1):+6.2f}% full {100*(o[c][1]/B[c][1]-1):+6.2f}% bytes {100*(o[c][2]/B[c][2]-1):+6.2f}%"
    print(s + (f"  WRONG {wrong}" if wrong else ""), flush=True)
LN = math.log(1e-9 / 1e9)
tabz = np.load("ctab_40.npz"); keys = list(tabz["keys"]); TAB = tabz["tab"]; K = int(tabz["K"]); g = np.arange(K + 1) / K
def c_item(kn):
    f = RegularGridInterpolator((g, g, g), TAB[..., keys.index(kn)])
    out = np.zeros(len(a)); out[four] = f(np.clip(w[four, :3], 0, 1)); return out
def c_sup(kn): return float(TAB[..., keys.index(kn)].max())
rel("A  sub-Gaussian 9.330 everywhere (before c25333fa9)", width(9.330, 9.330))
rel("S  harmful side only, committed method: 7.373 / 9.018", width(7.373, 9.018))
rel("C  exact worst case, two-sided, certified: 7.413 / 8.774", width(7.413, 8.774))
rel("C1 closed form (convex order), one-sided: 7.373 / 8.815", width(7.373, 8.815))
rel("D  exact worst case, one-sided, certified: 7.373 / 8.704", width(7.373, 8.704))
kap = {1: np.array([8.4261, 8.3234, 8.8603, 9.3163]) ** 2, 2: np.array([8.3785, 8.6097, 8.6598, 9.2168]) ** 2, 3: np.array([8.8614, 8.7891, 8.4486, 9.0793]) ** 2}
cP = np.zeros(len(a))
for l in (1, 2, 3):
    m = (lv == l) & four
    cP[m] = np.sqrt(np.minimum(w[m] @ kap[l], 8.704 ** 2))
    print(f"      level {l + 1}: per-block constants give mean {cP[m].mean():.3f} (p10 {np.percentile(cP[m], 10):.3f}, p90 {np.percentile(cP[m], 90):.3f}); the item's own exact constant mean {c_item('one')[m].mean():.3f}")
rel("P  D with certified per-block constants (one plane a level)", width(7.373, cP))
rel("E  D with each item's own exact constant (ideal per item)", width(7.373, c_item("one")))
def split(pis, per_item=False):
    names = {0.7: "p70", 0.1: "p10", 0.25: "one", 0.5: "p50", 0.9: "p90", 1 / 30: "p033"}
    c4 = np.zeros(len(a))
    for l in (1, 2, 3):
        c4[lv == l] = (c_item(names[pis[l]])[lv == l] if per_item else c_sup(names[pis[l]]) + 0.009)
    return width(c1(95, LN + math.log(pis[0])), c4)
rel("L  D with eps split 0.1 / 0.1 / 0.1 / 0.7 over the levels", split((0.1, 0.1, 0.1, 0.7)))
rel("L' D with eps split 1/30 x 3 / 0.9", split((1 / 30, 1 / 30, 1 / 30, 0.9)))
rel("EL E with the split 0.1 / 0.1 / 0.1 / 0.7 (best sound found)", split((0.1, 0.1, 0.1, 0.7), per_item=True))
# Theorem B with the worst-case constants (one-sided events)
c = np.where(single, 7.373, 8.704)
E = (ej * cod).sum(1); W = np.where(cod, D * R ** 2 / np.maximum(a, 1e-30) ** 2, 0).sum(1)
k2 = c * c / np.maximum(W, 1e-30); disc = c * c * V * (1 + k2) - k2 * E * E; sq = np.sqrt(np.maximum(disc, 0))
hu = (sq - k2 * E) / (1 + k2); hl = (sq + k2 * E) / (1 + k2); Rs = (R * cod).sum(1); bad = E * E > V * W
hu = np.where(bad, np.maximum(Rs - E, 0), hu); hl = np.where(bad, np.maximum(Rs + E, 0), hl)
rel("T  D with the self-normalised interval (Theorem B)", (hl + hn) * (1 + 1e-6), (hu + hn) * (1 + 1e-6))
print("      Theorem B: k = c^2 / W median by level " + ", ".join(f"{np.median(k2[lv == l]):.4f}" for l in range(4)) + f"; records with E^2 > VW: {int(bad.sum())}")
rel("N  D for the index's own size, N = 1e6: 6.927 / 7.957", width(6.927, 7.957))
rel("R  D with the stored scale's 2.45% rounding removed (n/a)", width(7.373, 8.704, 1.0245))
rel("X  single-item constant 5.67 / 6.15 (not sound)", width(c1(95, math.log(1e-9 / 4)), c_sup("single")))
