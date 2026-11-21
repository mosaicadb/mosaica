# Stage 0, block norms: tables from the per-group results of measure.py.
# usage: report.py WORKDIR
import numpy as np, sys, glob
W = sys.argv[1]
s0 = np.load(f"{W}/setup.npz"); kind = s0["kind"]; FR = s0["FR"]; N = len(s0["a"])
fs = sorted(glob.glob(f"{W}/g[0-9][0-9].npz")); R = [np.load(f) for f in fs]
NAMES = list(R[0]["names"]); KS = list(R[0]["ks"])
tot = lambda key: sum(r[key] for r in R)
npair = tot("npair"); sh, sa, sb, sra, und = tot("sum_h"), tot("sum_act"), tot("sum_rb"), tot("sum_ra"), tot("und")
viol = tot("viol"); exc = np.max(np.stack([r["excess"] for r in R]), 0)
hb, ha, hA = tot("hist_best"), tot("hist_act"), tot("hist_a"); LB = np.linspace(-1, 13, 281)
KN = ["text (held-out prompts)", "text (in-sample species)", "item"]
NQ = [int((kind == k).sum()) for k in range(3)]
def med(h):
    c = np.cumsum(h); i = np.searchsorted(c, c[-1] / 2); i = min(max(i, 1), len(LB) - 1); return 2 ** (0.5 * (LB[i - 1] + LB[i]))
print(f"items {N}, groups {len(R)} of 12, pairs per query {npair[0]}; queries: " + ", ".join(f"{k} {q}" for k, q in zip(KN, NQ)))
print(f"violations over every pair, bound and k: {int(viol.sum())}; largest (actual - bound): {exc.max():.2e}")
SHOW = ["a", "b1", "b2", "ccnt2", "ccnt4", "ccnt8", "ccnt16", "cvar2", "cvar4", "cvar8", "cvar16", "d", "b2c4", "b2c16", "b2d",
        "ar", "b1r", "b2r", "cvar4r", "cvar16r", "ccnt4r", "ccnt16r"]
for kd in range(3):
    m = kind == kd; P = npair[0] * m.sum()
    print(f"\n=== {KN[kd]}: {m.sum()} queries x {npair[0]} items")
    print("Mean half-width h (ratio to a), per k = " + " / ".join(map(str, KS)))
    for nm in SHOW:
        i = NAMES.index(nm); h = sh[i][:, m].sum(1) / P; ref = sh[NAMES.index("a")][:, m].sum(1) / P
        print(f"  {nm:8s} " + " ".join(f"{x:.4f}({x / y:4.2f})" for x, y in zip(h, ref)))
    print("Mean over pairs of h / best weighted bound for the pair (d in the data-only basis; b2d for the mixed basis); median in []")
    for nm in SHOW:
        i = NAMES.index(nm); print(f"  {nm:8s} " + " ".join(f"{sb[i, ik][m].sum() / P:5.2f}[{med(hb[i, ik, kd]):4.2f}]" for ik in range(len(KS))))
    print("h / |actual|: ratio of means, median per pair in []")
    for nm in SHOW:
        i = NAMES.index(nm); print(f"  {nm:8s} " + " ".join(f"{sh[i, ik][m].sum() / sa[i, ik][m].sum():6.1f}[{med(ha[i, ik, kd]):5.1f}]" for ik in range(len(KS))))
    print("Undecided share, % of items, answers 0.01 / 0.1 / 1 / 5 %")
    for ik, k in enumerate(KS):
        print(f" k = {k}")
        for nm in SHOW:
            i = NAMES.index(nm); u = 100 * und[i, ik][m].sum(0) / P
            print(f"   {nm:8s} " + " / ".join(f"{x:6.2f}" for x in u))
