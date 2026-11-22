# analyse.py OUT [OUT2 ...]: from study_w_principled's OUT.nb and OUT.cl, per (set, mode, psi):
# - true neighbours in index-path clusters: count, dropped at each level, and the median / p90 of their height h,
#   the threshold height zeta, and the error over tau at level 0 (z0), for those found and those dropped;
# - index-path clusters: median and p10 / p90 of tau_built / tau_corr at levels 0 and 1, the same over the clusters
#   holding a true neighbour, and tau0 / sigma (sqrt(1 - f)).
import sys
import numpy as np
from collections import defaultdict


def q(v, p):
    return float(np.quantile(v, p)) if len(v) else float("nan")


for out in sys.argv[1:]:
    nb = defaultdict(list)
    for i, line in enumerate(open(out + ".nb")):
        if i == 0:
            continue
        x = line.split()
        nb[(x[0], x[1], float(x[2]))].append((x[6], *map(float, x[7:])))
    cl = defaultdict(list)
    for i, line in enumerate(open(out + ".cl")):
        if i == 0:
            continue
        x = line.split()
        cl[(x[0], x[1], float(x[2]))].append(tuple(map(float, x[5:])))
    print(f"== {out}")
    for key in sorted(nb, key=lambda k: (k[0], k[1], k[2])):
        v = nb[key]
        fate = [r[0] for r in v]
        a = np.array([r[1:] for r in v])
        h, zeta, z0 = a[:, 0], a[:, 1], a[:, 2]
        found = np.array([f == "found" for f in fate])
        drops = {f: fate.count(f) for f in sorted(set(fate)) if f != "found"}
        c = np.array(cl[key]) if cl[key] else np.zeros((0, 9))
        hold = c[:, 8] > 0 if len(c) else np.zeros(0, bool)
        print(f"{key[0]:<10} {key[1]:<12} psi={key[2]:g}: neighbours on the index path {len(v)}, {drops};"
              f" found h {q(h[found], .5):.2f}/{q(h[found], .9):.2f} zeta {q(zeta[found], .5):.2f} z0 {q(z0[found], .5):.2f}/{q(z0[found], .9):.2f};"
              f" dropped h {q(h[~found], .5):.2f} zeta {q(zeta[~found], .5):.2f} z0 {q(z0[~found], .5):.2f}"
              f" | clusters {len(c)}: built/corr L0 {q(c[:, 3], .1):.3f}/{q(c[:, 3], .5):.3f}/{q(c[:, 3], .9):.3f} L1 {q(c[:, 4], .5):.3f};"
              f" holding {hold.sum()}: L0 {q(c[hold, 3], .5):.3f} tau0/sig {q(c[hold, 1], .5):.3f} zeta {q(c[hold, 0], .5):.2f};"
              f" others L0 {q(c[~hold, 3], .5):.3f} tau0/sig {q(c[~hold, 1], .5):.3f} zeta {q(c[~hold, 0], .5):.2f}")
