# NOT RUN. The test of the rate-margin law (DESIGN-CLAIMS, C4.4) on the 50k verify dump.
# usage: python3 c4_dump_test.py dump.bin c0 c1 c2 c3 b0 b1 b2 b3 [out.json]
#   c_l: the build's constant at randomised level l (study_res.out or meta.json); b_l: bytes a survivor reads on entering level l
#   (24+5.5, then the three stored sizes of the build log). One pass in chunks of 400,000 records (38 MB); the join table of the
#   first-level scales is the only other large array (12 bytes per level-0 record, about 46 MB at 3.8 M records).
# Record: 24 float32. 0 tag (uint32: bit 0 index plan, bits 1-5 answer size, bits 6+ query, queries >= NQ_ITEM are text), 1 row (uint32),
#   2 level, 3 t, 4 truth - t, 5 estimate - t, 6 deterministic part of the half-width, 7 sd, then per block: coded flag, a_j, R_j, e_j.
import sys, json, math, numpy as np
path = sys.argv[1]; C = [float(x) for x in sys.argv[2:6]]; BY = [float(x) for x in sys.argv[6:10]]; out = sys.argv[10] if len(sys.argv) > 10 else "c4_dump_test.json"
NQ_ITEM = 10; D = np.array([95, 127, 255, 287.0]); CH = 400_000; FULL = 1536.0
raw = np.memmap(path, dtype=np.float32, mode="r").reshape(-1, 24); n = len(raw)
UB = np.exp(np.linspace(math.log(1e-3), math.log(3.0), 61))            # bins of the normalised margin u
def chunks():
    for s in range(0, n, CH):
        x = np.array(raw[s:s + CH]); tag = x[:, 0].copy().view(np.uint32); row = x[:, 1].copy().view(np.uint32)
        k = (tag & 1) == 1
        yield x[k].astype(np.float64), tag[k], row[k]
# pass 1: the scale of every item at its first randomised level. eta = sum_j R_j when R_j is filled for coded blocks, else the level-0 half-width.
keys, etas, hw0 = [], [], []; filled = True
for x, tag, row in chunks():
    m = x[:, 2] == 0; cod = x[m][:, 8::4] > 0; R = x[m][:, 10::4]
    if (R[cod] <= 0).any(): filled = False
    keys.append((tag[m].astype(np.uint64) << np.uint64(32)) | row[m].astype(np.uint64))
    etas.append(R.sum(1).astype(np.float32)); hw0.append((C[0] * x[m][:, 7] + x[m][:, 6]).astype(np.float32))
keys = np.concatenate(keys); scale = np.concatenate(etas if filled else hw0); o = np.argsort(keys); keys = keys[o]; scale = scale[o]
# pass 2: per (class, answer size, level, u bin): entrants, survivors, sums of w = h/scale and w^2, of tan phi per block; bytes per (query, size)
acc = {}; byq = {}
for x, tag, row in chunks():
    key = (tag.astype(np.uint64) << np.uint64(32)) | row.astype(np.uint64); i = np.searchsorted(keys, key); ok = (i < len(keys)); ok[ok] &= keys[i[ok]] == key[ok]
    x, tag, i = x[ok], tag[ok], i[ok]; lv = x[:, 2].astype(int); eta = scale[i].astype(np.float64)
    h = np.array(C)[lv] * x[:, 7] + x[:, 6]; mu = np.abs(x[:, 4]); u = mu / eta; w = h / eta; surv = np.abs(x[:, 5]) <= h
    cod = x[:, 8::4] > 0; a = x[:, 9::4]; R = x[:, 10::4]; tanp = np.where(cod & (R > 0), a / np.maximum(R, 1e-30), np.nan)
    cls = (tag >> 6) >= NQ_ITEM; size = (tag >> 1) & 31; ub = np.clip(np.searchsorted(UB, u), 0, len(UB))
    for c_ in (0, 1):
        for l in range(4):
            m = (cls == c_) & (lv == l)
            if not m.any(): continue
            A = acc.setdefault((c_, l), {"ent": np.zeros(len(UB) + 1), "sur": np.zeros(len(UB) + 1), "w1": 0.0, "w2": 0.0, "lw1": 0.0, "lw2": 0.0, "n": 0, "tan": np.zeros(4), "ntan": np.zeros(4)})
            A["ent"] += np.bincount(ub[m], minlength=len(UB) + 1); A["sur"] += np.bincount(ub[m], weights=surv[m], minlength=len(UB) + 1)
            A["w1"] += w[m].sum(); A["w2"] += (w[m] ** 2).sum(); A["lw1"] += np.log(w[m]).sum(); A["lw2"] += (np.log(w[m]) ** 2).sum(); A["n"] += int(m.sum())
            A["tan"] += np.nansum(tanp[m] ** 2, 0); A["ntan"] += np.isfinite(tanp[m]).sum(0)
    qs = (tag >> 1).astype(np.int64)                                       # query and size together
    actual = np.array(BY)[lv] + FULL * ((lv == 3) & surv)                  # bytes this record adds: entering its level, and the full vector if undecided at the last
    for q_ in np.unique(qs):
        m = qs == q_; B = byq.setdefault(int(q_), {"actual": 0.0, "u": [np.zeros(len(UB) + 1) for _ in range(1)]})
        B["actual"] += float(actual[m].sum()); B["u"][0] += np.bincount(ub[m & (lv == 0)], minlength=len(UB) + 1)
# results
res = {"scale": "sum of block Cauchy-Schwarz terms" if filled else "level-0 half-width", "levels": {}, "queries": {}}
uc = np.concatenate([[UB[0]], np.sqrt(UB[1:] * UB[:-1]), [UB[-1]]]); wmed = {}
for (c_, l), A in sorted(acc.items()):
    S = A["sur"] / np.maximum(A["ent"], 1); mean = A["w1"] / A["n"]; cv = math.sqrt(max(A["w2"] / A["n"] - mean ** 2, 0)) / mean
    gm = math.exp(A["lw1"] / A["n"]); sdlog = math.sqrt(max(A["lw2"] / A["n"] - (A["lw1"] / A["n"]) ** 2, 0))
    good = A["ent"] >= 200; cross = float(np.interp(0.5, S[good][::-1], uc[good][::-1])) if good.sum() > 3 else float("nan")   # u at which half the entrants survive
    u10 = float(np.interp(0.1, S[good][::-1], uc[good][::-1])) if good.sum() > 3 else float("nan"); u90 = float(np.interp(0.9, S[good][::-1], uc[good][::-1])) if good.sum() > 3 else float("nan")
    wmed[(c_, l)] = gm
    res["levels"][f"{'text' if c_ else 'item'} level {l}"] = {"records": A["n"], "w geometric mean": gm, "w cv": cv, "sd of ln w": sdlog, "u at 50% survival": cross, "u at 90% / 10% survival": [u90, u10],
        "predicted 10-90 ratio": (1 + 1.2816 / C[l]) / (1 - 1.2816 / C[l]), "rms tan phi by block": np.sqrt(A["tan"] / np.maximum(A["ntan"], 1)).tolist(), "survival by u": S.tolist(), "entrants by u": A["ent"].tolist()}
    print(f"{'text' if c_ else 'item'} level {l}: n {A['n']}  w = h/scale: geometric mean {gm:.4f}, cv {cv:.3f}, sd ln {sdlog:.3f} | half survive at u = {cross:.4f} (law: {gm:.4f}) | 90%/10% at {u90:.4f}/{u10:.4f}")
# T3: bytes of the randomised levels predicted from the level-0 margins alone, per (query, size)
from math import erf
def Phi(z): return 0.5 * (1 + np.vectorize(erf)(z / math.sqrt(2)))
tot_a = tot_p = 0.0; rel = []
for q_, B in sorted(byq.items()):
    c_ = int((q_ >> 5) >= NQ_ITEM); hist = B["u"][0]; cur = np.ones(len(uc)); pred = 0.0
    for l in range(4):
        pred += BY[l] * float((hist * cur).sum()); w = wmed.get((c_, l)); s = w / C[l]
        cur = np.minimum(cur, Phi((w - uc) / s) - Phi((-w - uc) / s))
    pred += FULL * float((hist * cur).sum()); tot_a += B["actual"]; tot_p += pred; rel.append(pred / max(B["actual"], 1e-9) - 1)
    res["queries"][str(q_)] = {"actual": B["actual"], "predicted": pred}
print(f"T3: bytes of the randomised levels and full vectors, predicted / actual pooled {tot_p / tot_a:.3f}; per (query, size) rms relative error {float(np.sqrt(np.mean(np.square(rel)))):.3f}, worst {float(np.max(np.abs(rel))):.3f}")
res["T3"] = {"pooled predicted over actual": tot_p / tot_a, "rms relative error": float(np.sqrt(np.mean(np.square(rel))))}
json.dump(res, open(out, "w"))
