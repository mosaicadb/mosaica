# c4_dump_test.py with the entry replayed at the given constants: a record counts only if the item survived every earlier
# level at those constants (the dump keeps an item while the 9.33 bound does, so any constants <= 9.33 replay exactly).
# At c = 9.33 it reproduces c4_dump_test.py. usage: python3 c4_dump_test_replay.py c0 c1 c2 c3 out.json
import sys, json, math, numpy as np, dt
from scipy.stats import norm
C = [float(x) for x in sys.argv[1:5]]; out = sys.argv[5]
r = dt.load(); lv = r["lv"].astype(int)
h = dt.halfwidth(r, C); ent, surv = dt.replay(r, h)
cod = (r["cod"][:, None] >> np.arange(4)) & 1; R = r["R"].astype(np.float64)
eta0 = R.sum(1); filled = bool((R[cod == 1] > 0).all())
eta = eta0[r["i0"]]; mu = np.abs(r["tr"].astype(np.float64)); u = mu / eta; w = h / eta
UB = np.exp(np.linspace(math.log(1e-3), math.log(3.0), 61)); uc = np.concatenate([[UB[0]], np.sqrt(UB[1:] * UB[:-1]), [UB[-1]]])
ub = np.clip(np.searchsorted(UB, u), 0, len(UB))
res = {"constants": C, "scale": "sum of block Cauchy-Schwarz terms" if filled else "?", "levels": {}, "queries": {}, "by class and size": {}}
wbar = {}
for c_ in (0, 1):
    for l in range(4):
        m = ent & (r["text"] == c_) & (lv == l); n = int(m.sum()); wl = w[m]; lw = np.log(wl)
        E = np.bincount(ub[m], minlength=len(UB) + 1).astype(float); S_ = np.bincount(ub[m], weights=surv[m], minlength=len(UB) + 1) / np.maximum(E, 1)
        gm = float(np.exp(lw.mean())); sdl = float(lw.std()); cv = float(wl.std() / wl.mean()); wbar[(c_, l)] = gm
        good = E >= 200
        f = lambda p: float(np.interp(p, S_[good][::-1], uc[good][::-1]))
        nm = f"{'text' if c_ else 'item'} level {l}"
        res["levels"][nm] = {"records": n, "w geometric mean": gm, "w cv": cv, "sd of ln w": sdl, "u at 50% survival": f(0.5), "u at 90% / 10% survival": [f(0.9), f(0.1)],
                             "survival by u": S_.tolist(), "entrants by u": E.tolist()}
        print(f"{nm}: n {n}  w: geometric mean {gm:.4f}, cv {cv:.3f}, sd ln {sdl:.3f} | half survive at u = {f(0.5):.4f} | 90%/10% at {f(0.9):.4f}/{f(0.1):.4f}")
# T3
add = dt.BY[lv] * ent + dt.FULL * (ent & surv & (lv == 3))
qs = r["qs"]; uq = np.unique(qs); qi = np.searchsorted(uq, qs)
actual = np.bincount(qi, weights=add, minlength=len(uq))
m0 = lv == 0
H = np.zeros((len(uq), len(UB) + 1)); np.add.at(H, (qi[m0], ub[m0]), 1.0)
pred = np.zeros(len(uq))
for k, q_ in enumerate(uq):
    c_ = int((q_ >> 5) >= dt.NQ_ITEM); cur = np.ones(len(uc)); p = 0.0
    for l in range(4):
        p += dt.BY[l] * float((H[k] * cur).sum()); wl = wbar[(c_, l)]; s = wl / C[l]
        cur = np.minimum(cur, norm.cdf((wl - uc) / s) - norm.cdf((-wl - uc) / s))
    pred[k] = p + dt.FULL * float((H[k] * cur).sum())
    res["queries"][str(int(q_))] = {"actual": actual[k], "predicted": pred[k]}
rel = pred / actual - 1
print(f"T3: predicted / actual pooled {pred.sum() / actual.sum():.3f}; per (query, size) rms relative error {np.sqrt(np.mean(rel ** 2)):.3f}, worst {np.abs(rel).max():.3f}")
res["T3"] = {"pooled predicted over actual": pred.sum() / actual.sum(), "rms relative error": float(np.sqrt(np.mean(rel ** 2)))}
print("T3 by class and answer size: actual B/item of N (randomised levels + full vectors), predicted / actual, rms rel. error over the 10 queries")
for c_ in (0, 1):
    for s in range(7):
        m = ((uq >> 5) >= dt.NQ_ITEM) == c_
        m &= (uq & 31) == s
        a = actual[m].sum() / dt.NROWS / m.sum(); pr = pred[m].sum() / actual[m].sum(); e = float(np.sqrt(np.mean(rel[m] ** 2)))
        res["by class and size"][f"{'text' if c_ else 'item'} {dt.SIZES[s]}"] = {"actual B/item": a, "pred/actual": pr, "rms": e}
        print(f"  {'text' if c_ else 'item'} {dt.SIZES[s]:7.2%}: actual {a:7.2f} B/item, predicted/actual {pr:.3f}, rms {e:.3f}")
json.dump(res, open(out, "w"))
