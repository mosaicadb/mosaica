"""The rank-space model's local exponent of the rows read under the build rule K ~ sqrt N (claim A1.4, [model]):
rows(M) = the exact leaf floor of claim A1.2 with f = A(1/2) held fixed, L = 10 targets; d ln rows / d ln M at M = 1e6."""
import math
from scipy.stats import norm
from scipy import integrate, optimize

def rows(M, f, r, L=10):
    zT = norm.isf(L / M); s = math.sqrt(1 - f)
    joint = lambda a: integrate.quad(lambda x: norm.pdf(x / math.sqrt(f)) / math.sqrt(f) * norm.sf((zT - x) / s), a, a + 12, epsabs=0, epsrel=1e-9, limit=200)[0]
    a_r = optimize.brentq(lambda a: joint(a) - r * norm.sf(zT), -6, zT + 2, xtol=1e-10)
    return M * norm.sf(a_r / math.sqrt(f))

print("f = A(1/2)   r     rows at M = 1e6   share   local exponent (M 1e6 to 1.6e7)   asymptotic 1 - f")
for name, f in (("yahoo", 0.455), ("laion", 0.480), ("glove", 0.380)):
    for r in (0.9, 0.95, 0.99):
        a, b = rows(1e6, f, r), rows(1.6e7, f, r)
        print(f"{name} {f:.3f}  {r:.2f}  {a:10.0f}  {a / 1e6:.4f}  {math.log(b / a) / math.log(16):.3f}   {1 - f:.3f}")
