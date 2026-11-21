# Referee check of the inner inequalities of F1's proof on planar partitions that are not products.
# The law of z is replaced by the piecewise-constant law zt on a fine grid (so every quantity is exact); the proof's
# chain for a general mixture then reads, cell by cell and on average,
#   (a) E_g s_c(g) >= sum_i sqrt(gam_i) C_i(nu_c)                      (coupling, step 2)
#   (b) h(nu_c) <= sum_i ln(sqrt(2 pi e) C_i(nu_c))                     (L2, step 3)
#   (c) sum_i ln(sqrt(2 pi e) Cbar_i) >= h(zt) - H(c)                   (steps 3 and 4)
#   (d) Cbar_i <= sqrt(E zt_i^2)                                        (step 5)
# Both coordinate orders are tested.
import math
import sys
import numpy as np
from scipy.special import ndtr
from core_l2_2d import gap2d
from core_opt2d import start_labels, perimeter, MW

L, NF = 5.0, 100
e = np.linspace(-L, L, NF + 1)
pm = np.diff(ndtr(e))
mass = np.outer(pm, pm)
mass = mass / mass.sum()
x = 0.5 * (e[:-1] + e[1:])
area = np.outer(np.diff(e), np.diff(e))
h_zt = -(mass * np.log(mass / area)).sum()
# second moments of the piecewise-constant law per coordinate
m2 = float((pm / pm.sum() * (e[:-1] ** 2 + e[:-1] * e[1:] + e[1:] ** 2) / 3).sum())
rng = np.random.default_rng(5)
worst = dict(a=np.inf, b=np.inf, c=np.inf, d=np.inf)
for eps in (1.0, 0.3, 0.05):
    for kind, p1, p2 in [("grid", 0.6, 0.6), ("hex", 0.6, 0.52), ("hex", 1.2, 1.0), ("voronoi", 25, 1.0),
                         ("voronoi", 60, 0.2), ("voronoi", 6, 1.0), ("strips", 0.4, 2.0)]:
        lab = start_labels(kind, x, x, p1, p2, rng)
        _, inv = np.unique(lab, return_inverse=True)
        lab = inv.reshape(lab.shape)
        K = lab.max() + 1
        for order in (0, 1):
            Hc = 0.0
            Cb = np.zeros(2)
            la = lb = np.inf
            for c in range(K):
                M = np.where(lab == c, mass, 0.0)
                P = M.sum()
                if P <= 0:
                    continue
                Hc -= P * math.log(P)
                if order == 0:
                    gap, h, C1, C2 = gap2d(e, e, M)
                else:
                    gap, h, C2, C1 = gap2d(e, e, M.T)
                width = MW * perimeter(lab == c, 1.0 * e, eps * e)
                la = min(la, width / (C1 + eps * C2))
                lb = min(lb, gap)
                Cb += P * np.array([C1, C2])
            cc = math.log(2 * math.pi * math.e) + math.log(Cb[0]) + math.log(Cb[1]) - (h_zt - Hc)
            dd = math.sqrt(m2) - Cb.max()
            worst["a"] = min(worst["a"], la); worst["b"] = min(worst["b"], lb)
            worst["c"] = min(worst["c"], cc); worst["d"] = min(worst["d"], dd)
            print("eps=%.2f %-8s(%g,%g) order=%d cells=%d H=%.3f  (a) min width/corr=%.4f  (b) min L2 gap=%.4f  "
                  "(c) slack=%.4f  Cbar=(%.4f, %.4f)  sum ln(1/Cbar)=%.4f <= H"
                  % (eps, kind, p1, p2, order, K, Hc, la, lb, cc, Cb[0], Cb[1], -math.log(Cb[0]) - math.log(Cb[1])))
print("worst: (a) %.4f (needs >= 1)   (b) %.4f (>= 0)   (c) %.4f (>= 0)   (d) %.5f (>= 0)"
      % (worst["a"], worst["b"], worst["c"], worst["d"]))
