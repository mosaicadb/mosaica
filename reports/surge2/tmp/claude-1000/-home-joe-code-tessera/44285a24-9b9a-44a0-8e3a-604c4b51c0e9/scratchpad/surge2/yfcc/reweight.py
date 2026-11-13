import sys, json
recs = json.load(open(sys.argv[1]))['records']
edges = [10, 100, 1000, 10000, 50000, 1 << 40]
pop = {1: [5179, 5350, 27414, 8252, 12312], 2: [x * 38374 / 3000 for x in (1490, 1228, 98, 0, 0)]}
def b(v): return max(i for i in range(5) if v >= edges[i])
n = {}
for r in recs: n[(r['ntags'], b(r['V']))] = n.get((r['ntags'], b(r['V'])), 0) + 1
w = [pop[r['ntags']][b(r['V'])] / n[(r['ntags'], b(r['V']))] for r in recs]
def S(k): return sum(wi * r[k] for wi, r in zip(w, recs))
print('weighted queries', round(sum(w)), 'weighted pairs %.3e' % S('V'), 'share of pairs from one-tag V>=10k %.4f' % (sum(wi * r['V'] for wi, r in zip(w, recs) if r['ntags'] == 1 and r['V'] >= 10000) / S('V')))
ps = sorted({int(k.split('_')[1]) for k in recs[0] if k.startswith('both_')})
for tn in [x for x in ('true', 'seed', 'true1', 'seed1') if ('i1_' + x) in recs[0]]:
    print(tn, 'cluster cut %.4f f_c %.4f' % (S('cl_' + tn) / S('V'), S('i1_' + tn) / S('V')),
          ' '.join('p=%d: s %.4f (all pairs %.4f)' % (p, S(f'both_{p}_{tn}') / S('i1_' + tn), S(f'i2_{p}_{tn}') / S('V')) for p in ps))
    for m in [x for x in ('2.0', '2.5', '3.0') if f'f{x}_{tn}' in recs[0]]:
        print('  1-bit dial', m, 'survivors/query %.0f share of pairs %.5f lost %.5f of true top-k rows' % (S(f'f{m}_{tn}') / sum(w), S(f'f{m}_{tn}') / S('V'), S(f'flost{m}_{tn}') / S('n_within')))
