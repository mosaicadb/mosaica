# Referee check 3: the margins of q.rs 1402-1411 and rtab's `up` (kern.rs 467) in f64 against exact rationals.
# Sound means the integer margin is at least the real quotient: Rin >= (x + e - b)/sg, Rout >= (b + e + d x)/sg.
# The regime attacked is Rout with b very negative (b is the sum of the tables' minima, about -2^20 steps) and the
# quotient next to a small integer, where `up` adds only (|R| + 1) 1e-12 while the f64 sum's rounding is relative to |b|.
import math
from fractions import Fraction as F

import numpy as np

rng = np.random.default_rng(5)


def up(x):
    return math.ceil(x + (abs(x) + 1.0) * 1e-12)


def f32(x):
    return float(np.float32(x))


n = short_in = short_out = 0
worst = 0.0
errs = []
cross = []   # (|k|, k - computed, slack `up` adds) where the exact quotient is above k and the computed one below
signs = {"exact>k, computed>=k": 0, "exact>k, computed<k": 0, "exact<=k, computed>=k": 0, "exact<=k, computed<k": 0}
for trial in range(300000):
    sg = f32(10 ** rng.uniform(-9, -3))
    units = int(rng.choice([8, 16, 32, 64]))
    b = -sg * rng.uniform(0.2, 1.0) * 2 ** 20 * (units / 64)
    e = float(F(units) * F(sg) / 2) * (1 + 1e-6) * (1 + 1e-6) * rng.uniform(1, 30)
    d = f32(rng.uniform(0.2, 1.0)) if trial % 2 else 1.0
    k = int(rng.integers(-2, 3)) if trial % 3 else int(rng.integers(-100000, 100000))
    x = (k * sg - b - e) / d + rng.normal() * sg * 3e-10
    if x <= 0:
        continue
    q_in = (x + e - b) / sg
    q_out = (b + e + d * x) / sg
    ex_in = (F(x) + F(e) - F(b)) / F(sg)
    ex_out = (F(b) + F(e) + F(d) * F(x)) / F(sg)
    n += 1
    errs.append(float(F(q_out) - ex_out))
    signs[("exact>k" if ex_out > k else "exact<=k") + ", " + ("computed>=k" if q_out >= k else "computed<k")] += 1
    if ex_out > k and q_out < k:
        cross.append((abs(k), k - q_out, (abs(q_out) + 1.0) * 1e-12))
    if F(up(q_in)) < ex_in:
        short_in += 1
    if F(up(q_out)) < ex_out:
        short_out += 1
        worst = max(worst, float(ex_out - up(q_out)))
errs = np.array(errs)
print(f"{n} cases; f64 error of the OUT quotient: {errs.min():.3e} to {errs.max():.3e} steps")
print(f"Rin short of the exact quotient: {short_in}; Rout short: {short_out} (worst {worst:.3e} steps)")
print("OUT quotient against the integer it was placed at:", signs)
print(f"slack e carries beyond half a step per unit (two factors of 1 + 1e-6): {8 * 0.5 * 2e-6:.1e} steps at 8 units, {64 * 0.5 * 2e-6:.1e} at 64")
cross = np.array(cross)
if len(cross):
    print(f"crossings {len(cross)}: smallest |k| {int(cross[:, 0].min())}; computed below k by at most {cross[:, 1].max():.3e}; `up` adds at least {cross[:, 2].min():.3e} there")
