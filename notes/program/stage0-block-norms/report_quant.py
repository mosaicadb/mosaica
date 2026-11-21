# Stage 0, block norms: tables from quant.py, and the cost of J norms against the bytes they save.
# Saving at equal undecided share: for each bound at budget B, the budget B' at which one norm (a) leaves the same
# undecided share, by linear interpolation of log(undecided) against bytes between the measured budgets; saving = B' - B.
# Cost: (J - 1) extra 4-bit norms per item at that level = (J - 1)/2 bytes.
# usage: report_quant.py WORKDIR
import numpy as np, sys, glob
W = sys.argv[1]
s0 = np.load(f"{W}/setup.npz"); kind = s0["kind"]; N = len(s0["a"])
R = [np.load(f) for f in sorted(glob.glob(f"{W}/q[0-9][0-9].npz"))]
NAMES = list(R[0]["names"]); BY = list(R[0]["by"])
tot = lambda key: sum(r[key] for r in R)
npair = tot("npair"); sh, und, viol = tot("sum_h"), tot("und"), tot("viol")
exc = np.max(np.stack([r["excess"] for r in R]), 0)
print(f"groups {len(R)} of 12; violations {int(viol.sum())}; largest (actual - bound) {exc.max():.2e}")
KN = ["text (held-out prompts)", "text (in-sample species)", "item"]; FRN = ["0.01%", "0.1%", "1%", "5%"]
SHOW = ["a", "ccnt4", "ccnt16", "cvar2", "cvar4", "cvar8", "cvar16", "d", "b2", "b2d", "ar", "cvar4r", "cvar16r"]
for kd in range(3):
    m = kind == kd; P = npair[0] * m.sum(); ia = NAMES.index("a")
    print(f"\n=== {KN[kd]}: {m.sum()} queries x {npair[0]} items; budgets {BY} bytes")
    print("Mean half-width (ratio to a)")
    for nm in SHOW:
        i = NAMES.index(nm); h = sh[i][:, m].sum(1) / P; r = sh[ia][:, m].sum(1) / P
        print(f"  {nm:8s} " + " ".join(f"{x:.4f}({x / y:4.2f})" for x, y in zip(h, r)))
    U = 100 * und[:, :, m].sum(2) / P                                          # bound x budget x answer
    for it, fn in enumerate(FRN):
        print(f"Undecided %, answer {fn}; in [] the saving in bytes at equal undecided share against a, and the norms' cost")
        la = np.log(np.maximum(U[ia, :, it], 1e-9))
        for nm in SHOW:
            i = NAMES.index(nm); cells = []
            for il, b in enumerate(BY):
                u = np.log(max(U[i, il, it], 1e-9))
                # a's budget giving the same share (a's share falls with bytes); extrapolation is not attempted
                if u > la[0] or u < la[-1]: sv = "  n/a"
                else:
                    b2 = np.interp(-u, -la, BY); sv = f"{b2 - b:+5.1f}"
                J = int(nm.rstrip("r")[4:]) if nm.startswith("c") else 1
                cells.append(f"{U[i, il, it]:6.2f}[{sv}/{(J - 1) / 2:.1f}]" if nm.startswith("c") else f"{U[i, il, it]:6.2f}[{sv}]")
            print(f"  {nm:8s} " + " ".join(cells))
