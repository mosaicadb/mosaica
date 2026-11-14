# Extra rows for the dump simulation: the constant following the index's own size (N = 1e6 instead of 1e9), and the
# stored scale without its rounding (sd / 1.0245), each against the committed constants (B).
import numpy as np, math, sys
from law import c1
raw = np.fromfile(sys.argv[1], dtype=np.float32).reshape(-1, 24)
tag = raw[:, 0].view(np.uint32); row = raw[:, 1].view(np.uint32)
keep = (tag & 1) == 1; raw, tag, row = raw[keep], tag[keep], row[keep]
lv = raw[:, 2].astype(np.int64); est = raw[:, 5].astype(np.float64); hn = raw[:, 6].astype(np.float64); sd = raw[:, 7].astype(np.float64)
cod = raw[:, 8::4] > 0
qi = tag >> 6; text = qi >= 10
key = tag.astype(np.int64) << 32 | row.astype(np.int64)
order = np.lexsort((lv, key)); k_ = key[order]; l_ = lv[order]
first = np.r_[True, k_[1:] != k_[:-1]]; gid = np.cumsum(first) - 1; ng = gid[-1] + 1
start = l_[first]; t_ = text[order][first]
LB = np.array([29.5, 41.0, 67.7, 101.1]); FULL = 1536.0
single = cod.sum(1) == 1
def sim(c_s, c_f, shrink=1.0):
    h = (np.where(single, c_s, c_f) * sd / shrink + hn) * (1 + 1e-6)
    dec = ((est - h >= 0) | (est + h < 0))[order]
    fl = np.full(ng, 9, np.int64); np.minimum.at(fl, gid[dec], l_[dec])
    ent = np.stack([(start <= x) & (fl >= x) for x in range(4)], 1); full = fl == 9
    return {cls: (ent[m].sum(0), int(full[m].sum()), float((ent[m].sum(0) * LB).sum() + full[m].sum() * FULL)) for cls, m in (("item", ~t_), ("text", t_))}
def rel(name, o, b):
    print(f"{name:66s}" + "".join(f" | {c}: enter L1-3 {100*(o[c][0][1:].sum()/b[c][0][1:].sum()-1):+6.2f}% full {100*(o[c][1]/b[c][1]-1):+6.2f}% bytes {100*(o[c][2]/b[c][2]-1):+6.2f}%" for c in ("item", "text")), flush=True)
B = sim(7.413, 9.086)
rel("A 9.330 everywhere", sim(9.330, 9.330), B)
rel("D certified, one-sided, N = 1e9: 7.373 / 8.704", sim(7.373, 8.704), B)
rel("C1 closed form (convex order), one-sided: 7.373 / 8.815", sim(7.373, 8.815), B)
rel("N6 as D with the index's own size N = 1e6: 6.927 / 7.957", sim(6.927, 7.957), B)
rel("B at N = 1e6 (committed method): 6.976 / 8.372", sim(6.976, 8.372), B)
rel("R as D, stored scale without its 2.45% rounding (not available)", sim(7.373, 8.704, 1.0245), B)
