# One-sided exact tail of a coordinate of a uniform unit vector in R^m (m = n-1):
#   P[u1 >= s] = C_m * int_s^1 (1-t^2)^((m-3)/2) dt.   c = s*sqrt(m) is the half-width in units of sd.
import numpy as np, math
def tail(m, s):
    a = (m - 3) / 2
    # integrate to where the integrand has fallen by e^-60 relative to its value at s
    hi = 1.0
    if a > 0:
        l0 = a * math.log1p(-s * s)
        lo_, hi_ = s, 1.0
        for _ in range(60):
            mid = (lo_ + hi_) / 2
            if a * math.log1p(-mid * mid) > l0 - 60: lo_ = mid
            else: hi_ = mid
        hi = hi_
    t = np.linspace(s, hi, 200001)
    y = np.exp(a * np.log1p(-np.minimum(t * t, 1 - 1e-300)))
    h = t[1] - t[0]
    integ = h / 3 * (y[0] + y[-1] + 4 * y[1:-1:2].sum() + 2 * y[2:-1:2].sum())
    return math.exp(math.lgamma(m / 2) - math.lgamma((m - 1) / 2)) / math.sqrt(math.pi) * float(integ)
def c_exact(n, d):
    m = n - 1; lo, hi = 0.0, 1.0
    for _ in range(70):
        mid = (lo + hi) / 2
        if tail(m, mid) > d: lo = mid
        else: hi = mid
    return hi * math.sqrt(m)
def c_subg(d): return math.sqrt(2 * math.log(1 / d))
if __name__ == '__main__':
    assert abs(tail(99, 0.0) - 0.5) < 1e-6 and abs(tail(255, 0.0) - 0.5) < 1e-6
    eps, N = 1e-9, 1e9
    ns = (100, 256, 768)
    rows = [
     ('A0 today: all N x 5 levels, two-sided', eps / (2 * N * 5)),
     ('A1 all N x 5 levels, harmful side only', eps / (N * 5)),
     ('A2 all N x 3 levels, harmful side only', eps / (N * 3)),
     ('A3 visible 1e6 x 3 levels, harmful side', eps / (1e6 * 3)),
     ('B0 kNN k=10 x 3, two-sided (Theorem A)', eps / (2 * 10 * 3)),
     ('B1 kNN k=10 x 3, one-sided', eps / (10 * 3)),
     ('B2 kNN k=10, level 1 holds 0.9 eps', 0.9 * eps / 10),
     ('B2 ..., level 2 holds 0.09 eps', 0.09 * eps / 10),
     ('B2 ..., level 3 holds 0.01 eps', 0.01 * eps / 10),
     ('B3 kNN k=100 x 3, one-sided', eps / (100 * 3)),
     ('B4 kNN k=1000 x 3, one-sided', eps / (1000 * 3)),
     ('B5 single item, 3 levels (gap floor)', eps / 3),
     ('C  thresholds, cap grid, |S|=10', eps / (4.2 * 64 * 3)),
     ('C  thresholds, cap grid, |S|=1e3', eps / (4.2 * 1e3 * 3)),
     ('C  thresholds, cap grid, |S|=1e5', eps / (4.2 * 1e5 * 3)),
     ('C  thresholds, cap grid, |S|=1e7', eps / (4.2 * 1e7 * 3)),
     ('C* oracle |S|=1e3 (count known)', eps / (1e3 * 3)),
     ('C* oracle |S|=1e5', eps / (1e5 * 3)),
     ('D  per item, eps_item=1e-9, 3 levels', eps / 3),
     ('D  per item, eps_item=1e-6, 3 levels', 1e-6 / 3),
     ('E  kNN k=10 x 3 one-sided, eps=1e-6', 1e-6 / 30),
     ('E  kNN k=10 x 3 one-sided, eps=1e-3', 1e-3 / 30),
    ]
    print(f'{"events":42} {"delta1":>8} {"subG":>6} ' + ' '.join(f'{"n=" + str(n):>7}' for n in ns))
    for name, d in rows: print(f'{name:42} {d:8.1e} {c_subg(d):6.2f} ' + ' '.join(f'{c_exact(n, d):7.2f}' for n in ns))
    print('deterministic cap sqrt(n-1):', [round(math.sqrt(n - 1), 2) for n in ns])
