import numpy as np, sys
sys.argv = ["x", "1"]
src = open("knn_adv.py").read().split("for mode in (\"two\", \"harmful\"):")[0]
exec(src)
shown = 0
for _ in range(20000):
    s, iv, k, top, cands = trial("two")
    out, vals = select(s, cands, iv, k, "others", False)
    if set(out) != set(top) and len(s) <= 8:
        print("k", k, "scores", np.round(s, 3).tolist(), "top", top, "out", out, vals)
        print("  cands", [(c[0], round(c[1], 3), round(c[2], 3), c[3], c[4]) for c in cands])
        print("  iv", [[(round(a, 3), round(b, 3)) for a, b in r] for r in iv])
        shown += 1
        if shown >= 3: break
