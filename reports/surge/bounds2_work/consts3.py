# Constants for threshold queries at N = 1e9, eps = 1e-9, L randomised levels, exact one-sided sphere tail (one rotation of all n dimensions).
import sys, os, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from consts2 import c_exact, c_subg
eps, N, J, gam = 1e-9, 1e9, 30, 1.4
ns = (512, 768, 1536)
for L in (5, 3):
    rows = [
     ('today: N x 5, two-sided', eps / (2 * N * 5)),
     ('all N x L, harmful side only (whole eps)', eps / (N * L)),
     ('mixed: certify-in, N x L at eps/2', eps / (2 * N * L)),
    ] + [(f'mixed: exclude, cap grid, |S| = {S:.0e} (eps/2)', eps / (2 * 4.2 * S * L)) for S in (1e3, 1e5, 1e6, 1e7, 1e8)] + \
        [(f'both sides, exposed set |U0| = {U:.0e}', eps / (U * L)) for U in (1e6, 1e7, 1e8)] + \
        [(f'band rule: undecided after level D = {D:.0e}', eps / (12 * L * D)) for D in (1e4, 1e6, 1e7, 1e8)] + \
        [('band rule: c_far', eps / (6 * N * L)), ('band rule: c_acc', eps / (6 * 2 * J * L * (L + 1))),
         ('single item x L (not attainable)', eps / L)]
    print(f'\nL = {L} levels')
    print(f'{"union":48} {"delta1":>8} {"subG":>6} ' + ' '.join(f'{"n=" + str(n):>7}' for n in ns))
    out = {}
    for name, d in rows:
        v = [c_exact(n, d) for n in ns]; out[name] = v
        print(f'{name:48} {d:8.1e} {c_subg(d):6.2f} ' + ' '.join(f'{x:7.2f}' for x in v))
    fl = [(gam * a + b) / (1 + gam) for a, b in zip(out['band rule: c_far'], out['band rule: c_acc'])]
    print(f'{"band rule: floor (gamma 1.4)":48} {"":8} {(gam * c_subg(eps / (6 * N * L)) + c_subg(eps / (6 * 2 * J * L * (L + 1)))) / (1 + gam):6.2f} ' + ' '.join(f'{x:7.2f}' for x in fl))
print('deterministic cap sqrt(n-1):', [round(math.sqrt(n - 1), 2) for n in ns])
