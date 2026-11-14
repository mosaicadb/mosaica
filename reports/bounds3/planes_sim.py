# Per-block constants: h^2 = sum_j kappa_j a_j^2 / d_j with kappa the gradient of F(v) = H(sqrt(v d))^2 at an anchor
# share vector (F is homogeneous of degree 1, so the tangent plane at w0 is grad F(w0) . v). Measured on the dump:
# anchors at each level's mean shares (one plane), or k-means centres of the shares (several planes, minimum taken).
import numpy as np, math, sys
from scipy.interpolate import RegularGridInterpolator
from ctable import quantiles
from law import c1
p = sys.argv[1]
raw = np.fromfile(p, dtype=np.float32).reshape(-1, 24)
tag = raw[:, 0].view(np.uint32); row = raw[:, 1].view(np.uint32)
keep = (tag & 1) == 1
raw, tag, row = raw[keep], tag[keep], row[keep]
lv = raw[:, 2].astype(np.int64); est = raw[:, 5].astype(np.float64); hn = raw[:, 6].astype(np.float64); sd = raw[:, 7].astype(np.float64)
tr = raw[:, 4].astype(np.float64)
cod = raw[:, 8::4] > 0; a = raw[:, 9::4].astype(np.float64)
D = np.array([95, 127, 255, 287], float); ds = [95, 127, 255, 287]
qi = tag >> 6; text = qi >= 10
key = tag.astype(np.int64) << 32 | row.astype(np.int64)
order = np.lexsort((lv, key)); k_ = key[order]; l_ = lv[order]
first = np.r_[True, k_[1:] != k_[:-1]]; gid = np.cumsum(first) - 1; ng = gid[-1] + 1
start = l_[first]; t_ = text[order][first]
LB = np.array([29.5, 41.0, 67.7, 101.1]); FULL = 1536.0
def sim(h):
    dec = ((est - h >= 0) | (est + h < 0))[order]
    fl = np.full(ng, 9, np.int64); np.minimum.at(fl, gid[dec], l_[dec])
    ent = np.stack([(start <= x) & (fl >= x) for x in range(4)], 1); full = fl == 9
    return {cls: (ent[m].sum(0), int(full[m].sum()), float((ent[m].sum(0) * LB).sum() + full[m].sum() * FULL)) for cls, m in (("item", ~t_), ("text", t_))}
def rel(name, o, b):
    print(f"{name:52s}" + "".join(f" | {c}: full {100*(o[c][1]/b[c][1]-1):+6.2f}% bytes {100*(o[c][2]/b[c][2]-1):+6.2f}%" for c in ("item", "text")), flush=True)
LN = math.log(1e-9 / 1e9); lnd = LN - math.log(4)
single = cod.sum(1) == 1; four = cod.all(1)
v = a ** 2 / D * cod; V = v.sum(1); w = v / np.maximum(V[:, None], 1e-300)
c1_one = c1(95, lnd)
def widths(c4):
    return (np.where(single, c1_one, c4) * sd + hn) * (1 + 1e-6)
bB = sim(widths(9.086) * 0 + (np.where(single, 7.413, 9.086) * sd + hn) * (1 + 1e-6))
tabz = np.load("ctab_40.npz"); keys = list(tabz["keys"]); TAB = tabz["tab"]; K = int(tabz["K"]); g = np.arange(K + 1) / K
f = RegularGridInterpolator((g, g, g), TAB[..., keys.index("one")])
cex = np.full(len(a), np.nan); cex[four] = f(np.clip(w[four, :3], 0, 1))
csup = float(TAB[..., keys.index("one")].max())
rel("D worst-case one-sided", sim(widths(csup + 0.009)), bB)
rel("E exact per-item (ideal)", sim(widths(np.where(four, cex, 0))), bB)
def grad_F(w0, eps=1e-4):
    """F(v) = (quantile of sum sqrt(v_j d_j) U_j)^2; gradient by central differences."""
    pts = [w0] + [w0 + s * eps * np.eye(4)[j] for j in range(4) for s in (1, -1)]
    q = quantiles(np.array(pts), ds, [lnd])[:, 0] ** 2
    return q[0], np.array([(q[1 + 2 * j] - q[2 + 2 * j]) / (2 * eps) for j in range(4)])
def kmeans(X, k, it=30, seed=0):
    rng = np.random.default_rng(seed); C = X[rng.choice(len(X), k, replace=False)]
    for _ in range(it):
        d2 = ((X[:, None, :] - C[None]) ** 2).sum(-1); lab = d2.argmin(1)
        C = np.array([X[lab == i].mean(0) if (lab == i).any() else C[i] for i in range(k)])
    return C
rng = np.random.default_rng(1)
for nk in (1, 2, 4, 8):
    c4 = np.zeros(len(a)); viol = 0.0
    for l in (1, 2, 3):
        m = (lv == l) & four
        sub = w[m][rng.choice(m.sum(), 20000, replace=False)]
        anchors = [sub.mean(0)] if nk == 1 else list(kmeans(sub, nk))
        best = np.full(m.sum(), np.inf)
        for w0 in anchors:
            F0, gk = grad_F(np.asarray(w0))
            best = np.minimum(best, w[m] @ gk)
            if nk == 1:
                print(f"   level {l}: anchor {np.round(w0, 3)} c(w0) {math.sqrt(F0):.4f}; kappa_j (as constants sqrt) {np.round(np.sqrt(gk), 3)}; check grad.w0 = F: {gk @ w0:.4f} vs {F0:.4f}")
        c4[m] = np.sqrt(best)
        viol = max(viol, float(np.nanmax(cex[m] - c4[m])))
        print(f"   level {l}, {nk} plane(s): mean constant {c4[m].mean():.4f} (exact per-item {np.nanmean(cex[m]):.4f}, worst-case {csup:.4f}); max of exact - plane {np.nanmax(cex[m] - c4[m]):+.5f}")
    rel(f"P{nk}: {nk} tangent plane(s) per level", sim(widths(np.where(four, c4, 0) + 0.0)), bB)
