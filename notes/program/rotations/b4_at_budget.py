"""B4, second table: the structured-round bound evaluated at the block's own certified constant, the one-coordinate
sphere quantile at delta = 2.5e-19 per side (N = 1e9, L = 4), in place of the round figure 7.8. A failure of the
certificate is then Pr[|error| > c sd] above 5e-19 = 2^-60.8.
"""
import numpy as np
import b3_structured as b3
from f_constant import sphere_c

lnd = np.log(4e18)
tables = {}
for (n, t, k, total, chunk) in [(256, 64, 2, 400_000, 4000), (256, 32, 2, 400_000, 4000), (256, 16, 2, 200_000, 4000),
                                (256, 16, 3, 200_000, 4000), (256, 32, 3, 400_000, 4000),
                                (256, 32, 4, 400_000, 4000), (256, 16, 4, 200_000, 4000),
                                (512, 32, 2, 60_000, 2000), (512, 32, 3, 60_000, 2000), (512, 64, 2, 60_000, 2000),
                                (1024, 64, 2, 40_000, 2000), (1024, 32, 2, 40_000, 2000)]:
    c = sphere_c(n - 1, lnd)
    b3.C = round(c, 3)
    if n not in tables:
        tables[n] = b3.chars_table(n)
    print(f"[certified one-block constant at d = {n-1}: {c:.3f}]", end=" ")
    b3.run(n, t, k, total=total, chunk=chunk, chars=tables[n])
