import math, numpy as np, mpmath as mp, time
from mytail import *
ds = [95, 127, 255, 287]
# single-block check against the incomplete beta tail
for d in (95, 287):
    a = [math.sqrt(d)]
    for x in (7.0, 8.0):
        s = x / math.sqrt(d)
        ex = mp.log(mp.betainc(mp.mpf(d - 1) / 2, mp.mpf(1) / 2, 0, 1 - mp.mpf(s) ** 2, regularized=True) / 2)
        xs, lt = lntails_fft(a, [d], 8.3)
        i = int(round(x / 0.002))
        t0 = time.time(); m = lntail_mp(a, [d], x); t1 = time.time() - t0
        print(d, x, "beta", mp.nstr(ex, 12), "fft", lt[i], "mp inversion", mp.nstr(m, 12), f"{t1:.1f}s")
w = [0.1254, 0.167, 0.333, 0.3746]
a = [math.sqrt(x * d) for x, d in zip(w, ds)]
xs, lt = lntails_fft(a, ds, 8.3)
for x in (8.036, 8.704, 8.774):
    i = int(round(x / 0.002))
    for h in (0.25, 0.125):
        print(x, "fft", lt[i], "mp", mp.nstr(lntail_mp(a, ds, x, h=h), 14))
xs2, lt2 = lntails_fft(a, ds, 8.3, eta=0.001, M=1 << 17)
print("eta 0.001:", [lt2[int(round(x / 0.001))] for x in (8.036, 8.704, 8.774)])
