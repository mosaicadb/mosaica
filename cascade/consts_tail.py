# One-sided exact tail of a coordinate of a uniform unit vector in R^m: P[u1 >= s] = C_m * int_s^1 (1-t^2)^((m-3)/2) dt.
# c = s*sqrt(m) is the half-width in units of sd = A/sqrt(m) for one-sided failure delta.
import numpy as np, math
def tail(m, s):
    t = np.linspace(s, 1.0, 400001); y = (1 - t * t) ** ((m - 3) / 2)
    return math.exp(math.lgamma(m / 2) - math.lgamma((m - 1) / 2)) / math.sqrt(math.pi) * float(((y[1:] + y[:-1]) / 2).sum() * (t[1] - t[0]))
