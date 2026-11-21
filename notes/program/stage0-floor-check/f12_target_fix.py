# Rerun, with the corrected tail_norm, the three f12_target rows whose ratio to W was invalid (empty tail), plus
# F2 targets for m = 2, N = 400 and m = 3, N = 99 that the restart cut off. Same codes and optimiser as f12_target.py.
import sys, math, numpy as np
from scipy import optimize
src = open("f12_target.py").read().split('print("spectrum')[0]
exec(src)
from common import Eg
cases = [(1, 2000, 1e-4, "W"), (2, 20, 1e-6, "W"), (2, 99, 1e-6, "W"),
         (2, 400, 1e-4, "F2(S=head)"), (2, 400, 1e-4, "F2(max S)"), (2, 400, 1e-4, "W"),
         (3, 99, 1e-4, "F2(S=head)"), (3, 99, 1e-4, "F2(max S)"), (3, 99, 1e-4, "W")]
want = [int(a) for a in sys.argv[1:]] or list(range(len(cases)))
print("m  N     eps    target      E h/target   H (nats)   E h        F2(S=head)  F2(max S, k)   W        code parameters")
for ci in want:
    m, N, eps, tname = cases[ci]
    gam = np.array([1.0] * m + [eps] * N); B = F2B(gam); Etu = math.sqrt(eps) * Eg(N)
    def code(p):
        w = math.exp(p[0]); D = math.exp(p[1]); o = p[2]
        if not (2e-3 < w < 20 and 0.01 < D < 50 and 0 <= o < 2 * math.sqrt(N) + 10): return None
        Er, Ht = tail_norm(N, D, o)
        return m * w / SQ2PI + Er * Etu, m * ecsq(w) + Ht
    def obj(p):
        c = code(p)
        if c is None: return 1e9
        Eh, H = c
        b = B.k(H, m) if tname == "F2(S=head)" else (B.best(H)[0] if tname == "F2(max S)" else B.W(H))
        return Eh / b
    best = (1e9, None)
    for x0 in ([math.log(0.5), math.log(2 / math.sqrt(N)), math.sqrt(N) - 1.5], [math.log(0.1), math.log(0.5 / math.sqrt(N)), math.sqrt(N) - 2],
               [math.log(2.0), math.log(1.0), math.sqrt(N) - 1], [math.log(0.02), math.log(4 / math.sqrt(N)), math.sqrt(N) - 3]):
        r = optimize.minimize(obj, x0, method="Nelder-Mead", options={"xatol": 1e-7, "fatol": 1e-11, "maxiter": 600})
        if r.fun < best[0]: best = (r.fun, r.x)
    Eh, H = code(best[1]); f2b, kb = B.best(H)
    print("%d %5d %7.0e  %-10s  %.6f    %8.3f   %.6g  %.6g  %.6g (%d)  %.6g  w=%.4g D=%.4g o=%.4g" % (
        m, N, eps, tname, best[0], H, Eh, B.k(H, m), f2b, kb, B.W(H), math.exp(best[1][0]), math.exp(best[1][1]), best[1][2]), flush=True)
