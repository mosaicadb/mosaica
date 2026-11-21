import numpy as np, dt
r = dt.load()
for C in ([dt.C933] * 4, [6.724, 7.639, 7.639, 7.639], [8.0] * 4):
    h = dt.halfwidth(r, C); ent, surv = dt.replay(r, h)
    print("c =", C, "records entered", int(ent.sum()), "of", len(ent), "by level", [int((ent & (r['lv'] == l)).sum()) for l in range(4)],
          "full vectors", int((ent & surv & (r['lv'] == 3)).sum()))
