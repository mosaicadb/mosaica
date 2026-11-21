# Shared loading and replay for the dump-test scripts. Records as prep.py writes them, sorted by (query, size, row, level).
import numpy as np
NQ_ITEM = 10; D = np.array([95, 127, 255, 287.0]); NB = D + 1
SIZES = [1e-4, 1e-3, 1e-2, 0.1, 0.3, 0.5, 0.75]; NROWS = 49500
BY = np.array([29.5, 41.0, 67.7, 101.1]); FULL = 1536.0          # bytes a survivor reads on entering each randomised level; full vector
C933 = 9.330163258547264
def load(cols=("key", "lv", "tr", "es", "hn", "sd", "cod", "a", "R")):
    r = {k: np.load(f"work/rec_{k}.npy") for k in cols}
    k = r["key"]; r["qs"] = (k >> np.uint64(32)).astype(np.int64); r["q"] = r["qs"] >> 5; r["size"] = r["qs"] & 31
    r["text"] = r["q"] >= NQ_ITEM
    first = np.ones(len(k), bool); first[1:] = k[1:] != k[:-1]
    r["i0"] = np.maximum.accumulate(np.where(first, np.arange(len(k)), 0))   # index of the item's level-0 record
    return r
def halfwidth(r, C, parts=("rand", "unc", "det")):
    """h = c_l sd + hn, the binary's half-width without the 1e-6 factor; parts may drop terms (unc: uncoded blocks' Cauchy-Schwarz)."""
    C = np.asarray(C, float); lv = r["lv"].astype(int); h = np.zeros(len(lv))
    unc = uncoded_cs(r)
    if "rand" in parts: h += C[lv] * r["sd"]
    if "unc" in parts: h += unc
    if "det" in parts: h += r["hn"] - unc
    return h
def uncoded_cs(r):
    cod = (r["cod"][:, None] >> np.arange(4)) & 1
    return (r["R"].astype(np.float64) * (1 - cod)).sum(1)
def replay(r, h, err=True):
    """Which records an item reaches with half-widths h: it enters level l when it survived every earlier level.
    err=True survives on the estimate (the index); err=False on the truth (the law without estimate error)."""
    lv = r["lv"]; x = r["es"] if err else r["tr"]
    surv = np.abs(x.astype(np.float64)) <= h * (1 + 1e-6)
    ent = lv == 0
    for l in (1, 2, 3):
        i = np.nonzero(lv == l)[0]; ent[i] = ent[i - 1] & surv[i - 1]
    return ent, surv
def bytes_by_qs(r, ent, surv):
    """bytes per item of N of the randomised levels and full vectors, per (query, size)."""
    lv = r["lv"]; add = BY[lv] * ent + FULL * (ent & surv & (lv == 3))
    qs = r["qs"]; u = np.unique(qs); out = {}
    tot = np.bincount(np.searchsorted(u, qs), weights=add, minlength=len(u))
    for k, v in zip(u, tot): out[int(k)] = v / NROWS
    return out
