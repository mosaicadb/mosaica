"""Per (cluster, level), how far a cluster's rms half-width, as a multiple of |D| or of its code term dwa rms(r), varies
across queries (study_w_unit's lines): the coefficient of variation over the queries reading the cluster, p10 / median /
p90 over the (cluster, level) units read by at least 5 queries, and the median of each unit's p90 / p10.
usage: python3 -I u_check.py FILE..."""
import sys

import numpy as np

for f in sys.argv[1:]:
    a = np.loadtxt(f, comments="#")
    c, l = a[:, 1].astype(int), a[:, 2].astype(int)
    rh, dn, dwa, rr = a[:, 3], a[:, 4], a[:, 5], a[:, 6]
    for name, x in (("rms(h) / |D|", rh / dn), ("rms(h) / (dwa rms(r))", rh / (dwa * rr)), ("dwa / |D|", dwa / dn)):
        cv, sp = [], []
        for key in set(zip(c, l)):
            m = (c == key[0]) & (l == key[1])
            if m.sum() < 5:
                continue
            v = x[m]
            cv.append(v.std() / v.mean())
            sp.append(np.quantile(v, 0.9) / np.quantile(v, 0.1))
        q = np.quantile(cv, [0.1, 0.5, 0.9])
        print(f"{f.rsplit('/', 1)[-1]:<12} {name:<24} units {len(cv)}: CV across queries p10 / median / p90 "
              f"{q[0]:.4f} / {q[1]:.4f} / {q[2]:.4f}; p90 / p10 within a unit, median {np.median(sp):.3f}; "
              f"level of the ratio, median {np.median(x):.3f}")
