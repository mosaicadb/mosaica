# One-sided exact tail of a coordinate of a uniform unit vector in R^m: P[u1 >= s] = C_m * int_s^1 (1-t^2)^((m-3)/2) dt.
# c = s*sqrt(m) is the half-width in units of sd = A/sqrt(m) for one-sided failure delta.
import numpy as np, math
def tail(m, s):
    t = np.linspace(s, 1.0, 400001); y = (1 - t * t) ** ((m - 3) / 2)
    return math.exp(math.lgamma(m / 2) - math.lgamma((m - 1) / 2)) / math.sqrt(math.pi) * float(((y[1:] + y[:-1]) / 2).sum() * (t[1] - t[0]))
def c_exact(m, d):
    lo, hi = 0.0, 1.0
    for _ in range(60):
        mid = (lo + hi) / 2
        if tail(m, mid) > d: lo = mid
        else: hi = mid
    return hi * math.sqrt(m)
def c_subg(d): return math.sqrt(2 * math.log(1 / d))
assert abs(tail(99, 0.0) - 0.5) < 1e-6
rows = [('now: N*5 events two-sided, eps 1e-9', 1e-19), ('N one-sided events', 1e-18), ('k=10 x 3 levels, two-sided', 1e-9 / 60), ('k=10 x 1 level, one-sided', 1e-10), ('k=100 x 3 levels, two-sided', 1e-9 / 600), ('k=1000 x 3, two-sided', 1e-9 / 6000), ('k=10 x 3, two-sided, eps 1e-6', 1e-6 / 60)]
ns = (100, 256, 768, 1536)
print(f'{"events":36} {"delta1":>8} {"subG":>6} ' + ' '.join(f'{"n=" + str(n):>7}' for n in ns))
for name, d in rows: print(f'{name:36} {d:8.1e} {c_subg(d):6.2f} ' + ' '.join(f'{c_exact(n - 1, d):7.2f}' for n in ns))
print('deterministic cap sqrt(n-1):', [round(math.sqrt(n - 1), 2) for n in ns])
