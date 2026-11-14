# Aggregates run.py outputs for one dataset into tables. usage: summarise.py <name> <out.npz>... (pieces in query order)
import sys, math, os, numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import consts_tail as ct
import h5py
name = sys.argv[1]; parts = [np.load(p) for p in sorted(sys.argv[2:])]
f = h5py.File(f'/home/joe/code/vexact-data/{name}/{name}.hdf5', 'r'); N, n = f['train'].shape
qzero = np.linalg.norm(np.asarray(f['test']), axis=1) == 0
F = list(parts[0]['fields']); keys = [kk[2:] for kk in parts[0].files if kk.startswith('r_')]
nq = sum(int(p['nq']) for p in parts)
print(f'queries {nq}; brute-force top-10 overlap with the dataset ground truth: {sum(int(p["gt_overlap"]) for p in parts) / (10 * nq):.5f}')
TB = n / 8; f16 = 2 * n
def bytes_per_query(cfg, r):
    indep = '_indep' in cfg; plain = cfg.startswith('plain')
    s1, s2, s3 = (3, 1, 1) if plain else (4, 3, 3)
    l3 = 3 * n / 8 if indep else n / 4
    return r[:, F.index('nvis')] * (TB + s1) + r[:, 0] * (TB + s2) + r[:, 1] * (l3 + s3) + r[:, 3] * f16
print(f'\n{"configuration":26} {"differ":>6} {"nbfail":>6} {"tau0=sk":>7} | {"L1 mean":>9} {"p90":>9} | {"L2 mean":>8} {"p90":>8} | {"L3 mean":>7} {"p90":>6} | {"full mean":>9} {"p90":>6} | {"MB mean":>7} {"p90":>6} | {"visible":>9}')
rows = []
for cfg in keys:
    r = np.concatenate([p['r_' + cfg] for p in parts]); assert len(r) == len(qzero)
    rows.append((cfg, r, '')); rows.append((cfg, r[~qzero], ' (zero queries left out)'))
for cfg, r, note in rows:
    if note and not qzero.any(): continue
    bq = bytes_per_query(cfg, r) / 1e6
    row = f'{cfg:26} {r[:, 4].sum():6d} {r[:, 5].sum():6d} {r[:, 9].mean():7.4f} |'
    for col in (0, 1, 2, 3):
        row += f' {r[:, col].mean():9.1f} {np.percentile(r[:, col], 90):9.0f} |'
    row += f' {bq.mean():7.2f} {np.percentile(bq, 90):6.2f} | {r[:, 10].mean():9.0f}'
    print(row)
    print(f'{"":26} {len(r)} queries{note}; neighbour bound failures per level: {r[:, 6:9].sum(0).tolist()}; worst query full reads {r[:, 3].max()}')
print('\nz = (true - estimate)/(sd |D_perp|), centre-corrected, first 1000 queries, all items')
law = {s: ct.tail(n - 1, s / math.sqrt(n - 1)) for s in (3, 4, 5)}
for c in ('c1', 'c2', 'c2t', 'c4n', 'c4i'):
    z = sum(p['z_' + c] for p in parts if 'z_' + c in p.files); zm = max(float(p['zmax_' + c]) for p in parts)
    if z[0] == 0: continue
    print(f'{c:4} pairs {z[0]:.3e} max|z| {zm:.2f}  ' + '  '.join(
        f'|z|>{s}: {z[1 + i]} ({z[1 + i] / z[0]:.3e}, law {2 * law[s]:.3e}, expected {2 * law[s] * z[0]:.1f}); z>{s}: {z[4 + i]} (law {law[s] * z[0]:.1f})'
        for i, s in enumerate((3, 4, 5))))
