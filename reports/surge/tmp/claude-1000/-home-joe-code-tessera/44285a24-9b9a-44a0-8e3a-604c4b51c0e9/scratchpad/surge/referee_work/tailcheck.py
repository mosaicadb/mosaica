# Independent check of the exact sphere tail and the constants table.
# u uniform on S^{m-1} in R^m:  P[u1 >= s] = 0.5 * I_{1-s^2}((m-1)/2, 1/2).
# Evaluated by the hypergeometric series I_x(a,b) = x^a (1-x)^b / (a B(a,b)) * 2F1(a+b, 1; a+1; x).
import math, sys, importlib.util, os
def log_tail(m, s):                       # log of the ONE-sided tail
    a, b, x = (m - 1) / 2, 0.5, 1 - s * s
    term, tot, j = 1.0, 1.0, 0
    while True:
        term *= (a + b + j) / (a + 1 + j) * x; j += 1; tot += term
        if term < 1e-17 * tot: break
        if j > 10_000_000: raise RuntimeError('no convergence')
    lB = math.lgamma(a) + math.lgamma(b) - math.lgamma(a + b)
    return math.log(0.5) + a * math.log(x) + b * math.log(1 - x) - math.log(a) - lB + math.log(tot)
def c_exact(m, d):                        # half-width in sd units (sd = 1/sqrt(m)) for one-sided tail d
    lo, hi = 1e-9, 1 - 1e-12; ld = math.log(d)
    for _ in range(200):
        mid = (lo + hi) / 2
        if log_tail(m, mid) > ld: lo = mid
        else: hi = mid
    return hi * math.sqrt(m)
def c_subg(d): return math.sqrt(2 * math.log(1 / d))
if __name__ == '__main__':
    # sanity: closed forms. m=3: u1 uniform on [-1,1] -> tail (1-s)/2.  m=2: arccos(s)/pi.  m=4: density (2/pi) sqrt(1-t^2)
    for s in (0.1, 0.5, 0.9):
        print('m=3 s=%.1f series %.12f closed %.12f | m=2 series %.12f closed %.12f' % (s, math.exp(log_tail(3, s)), (1 - s) / 2, math.exp(log_tail(2, s)), math.acos(s) / math.pi))
        cl4 = 0.5 - (s * math.sqrt(1 - s * s) + math.asin(s)) / math.pi
        print('     m=4 series %.12f closed %.12f' % (math.exp(log_tail(4, s)), cl4))
    # against the report's own integrator
    p = '/tmp/claude-1000/-home-joe-code-tessera/44285a24-9b9a-44a0-8e3a-604c4b51c0e9/scratchpad/surge/level0/consts_tail.py'
    spec = importlib.util.spec_from_file_location('ct', p); ct = importlib.util.module_from_spec(spec); spec.loader.exec_module(ct)
    for m, s in ((99, 0.3), (99, 0.6), (255, 0.4), (767, 0.25), (99, 0.75)):
        print('m=%d s=%.2f  series %.6e  report trapezoid %.6e  Prop10 bound (1-s^2)^(m/2) %.6e  subG exp(-m s^2/2) %.6e' % (m, s, math.exp(log_tail(m, s)), ct.tail(m, s), (1 - s * s) ** (m / 2), math.exp(-m * s * s / 2)))
    ns = (100, 256, 768, 1536)
    rows = [('report: N*5 two-sided (1e-19)', 1e-19), ('report: N one-sided (1e-18)', 1e-18),
            ('report: k=10 x3 two-sided', 1e-9 / 60), ('report: k=100 x3 two-sided', 1e-9 / 600), ('report: k=1000 x3 two-sided', 1e-9 / 6000),
            ('report: k=10 x3 two-sided eps 1e-6', 1e-6 / 60),
            ('k=10 x3 ONE-sided', 1e-9 / 30), ('k=10 x4 two-sided (2 directions at level 1)', 1e-9 / 80), ('k=10 x5 one-sided (clusters report)', 1e-9 / 50),
            ('N=1e9 x3 one-sided (all-items event)', 1e-9 / 3e9), ('N=1e9 x3 two-sided', 1e-9 / 6e9),
            ('N=1.18e6 x3 one-sided', 1e-9 / (3 * 1.18e6)),
            ('k=10 x3 two-sided, x13 scale grid', 1e-9 / (60 * 13)),
            ('100k-query run at total 1e-9: k=10 x3 two-sided', 1e-14 / 60), ('1e9-query lifetime at total 1e-9', 1e-18 / 60)]
    print(f'{"events":50} {"delta1":>9} {"subG":>6} ' + ' '.join(f'{"n=" + str(n):>7}' for n in ns))
    for name, d in rows: print(f'{name:50} {d:9.1e} {c_subg(d):6.2f} ' + ' '.join(f'{c_exact(n - 1, d):7.2f}' for n in ns))
    print('m = n (wrong dimension) for comparison, k=10x3 two-sided:', ' '.join(f'{c_exact(n, 1e-9/60) * math.sqrt((n-1)/n):7.3f}' for n in ns), ' vs m=n-1:', ' '.join(f'{c_exact(n - 1, 1e-9/60):7.3f}' for n in ns))
    print('deterministic cap sqrt(n-1):', [round(math.sqrt(n - 1), 2) for n in ns])
    print('Prop 10 closed bound c (one-sided (1-s^2)^(m/2) = d), k=10x3 two-sided:', ' '.join('%7.2f' % (math.sqrt((1 - (1e-9 / 60) ** (2 / (n - 1))) * (n - 1))) for n in ns))
    # approximate mode: recall bound 1 - L*delta(m)
    for n in (100, 256):
        for mm in (1.5, 2.0, 2.5, 3.0, 3.5):
            d1 = math.exp(log_tail(n - 1, mm / math.sqrt(n - 1)))
            print(f'n={n} dial {mm}: one-sided delta {d1:.5f}; recall bound L=1 {1 - d1:.4f}, L=3 {1 - 3 * d1:.4f}; two-sided L=3 {1 - 6 * d1:.4f}')
    print('Gaussian one-sided at 1.5, 2.5:', 0.5 * math.erfc(1.5 / math.sqrt(2)), 0.5 * math.erfc(2.5 / math.sqrt(2)))
