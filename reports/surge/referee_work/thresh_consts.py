import math, sys
sys.path.insert(0, __file__.rsplit('/', 1)[0])
from tailcheck import log_tail, c_exact, c_subg
ns = (512, 768, 1024); eps = 1e-9
rows = [('today: N=1e9 x 5 levels, two-sided', eps / (1e9 * 5 * 2)),
        ('N=1e9 x 5, one harmful side per item', eps / (1e9 * 5)),
        ('N=1e9 x 3, one harmful side per item', eps / (1e9 * 3)),
        ('undecided-after-deterministic U=1e7 x 3', eps / (1e7 * 3)),
        ('exclude side: matches |S|=1e7 x 3 (1% answer), eps/2', eps / 2 / (1e7 * 3)),
        ('exclude side: |S|=1e5 x 3 (0.01% answer), eps/2', eps / 2 / (1e5 * 3)),
        ('exclude side: |S|=1e3 x 3, eps/2', eps / 2 / (1e3 * 3)),
        ('certify-in side: non-matches 1e9 x 3, eps/2', eps / 2 / (1e9 * 3)),
        ('k-NN k=10 x 3 two-sided (Theorem A)', eps / 60)]
print(f'{"events":52} {"delta1":>9} {"subG":>6} ' + ' '.join(f'{"n=" + str(n):>7}' for n in ns))
for name, d in rows: print(f'{name:52} {d:9.1e} {c_subg(d):6.2f} ' + ' '.join(f'{c_exact(n - 1, d):7.2f}' for n in ns))
print('deterministic cap sqrt(n-1):', [round(math.sqrt(n - 1), 2) for n in ns])
