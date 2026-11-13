# Tables and plots from out/expA_*.json and out/expB_*.json. usage: report.py <outdir>
import sys, json, glob, os
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt

od = sys.argv[1]
TARGETS = (0.9, 0.95, 0.99)

def cell(o):
    return 'n/a' if o is None else f"{o['share'] * 100:.2f}% ({o['scanned']:,.0f}; p={o['nprobe']})"

merged = {}
for fn in sorted(glob.glob(f'{od}/expA_*_K*.json')):
    nm = os.path.basename(fn)[5:].rsplit('_K', 1)[0]
    merged.setdefault(nm, {}).update(json.load(open(fn)))
for name, A in merged.items():
    A = dict(sorted(A.items(), key=lambda kv: int(kv[0])))
    print(f'\n### A: {name}\n')
    print('Share of items scanned to reach mean recall@10 (ANN-Benchmarks tolerance), with the unique item count '
          'and the number of clusters probed (p); strict recall at 0.95 in its own column.\n')
    print('| K | Structure | Ordering | 0.9 | 0.95 | 0.99 | strict 0.95 | codes read at 0.95 |')
    print('|---|---|---|---|---|---|---|---|')
    for K, r in A.items():
        for run, v in r['runs'].items():
            if 'test' not in v: continue
            st, on = run.split('/')
            if on.startswith('bound') and on != r[f"chosen_bound_{st.split('+')[0]}"]: continue
            t = v['test']
            c95 = t['rt0.95']; codes95 = 'n/a' if c95 is None else f"{c95['codes']:,.0f}"
            print(f"| {K} | {st} | {on} | {cell(t['rt0.9'])} | {cell(t['rt0.95'])} | {cell(t['rt0.99'])} | "
                  f"{cell(t['rs0.95'])} | {codes95} |")
    print('\nBound factor sweep (test queries, share at 0.95; * = chosen on pseudo-queries):\n')
    for K, r in A.items():
        for base in ('plain', 'balanced'):
            row = []
            for on in ('centre', 'l2', 'bound0.1', 'bound0.2', 'bound0.35', 'bound0.5', 'bound0.75'):
                v = r['runs'][f'{base}/{on}']; o = v['test']['rt0.95']; ps = v['pseudo']['rt0.95']
                mark = '*' if on == r[f'chosen_bound_{base}'] else ''
                row.append(f"{on}{mark} {o['share'] * 100:.2f}% (pseudo {ps['share'] * 100:.2f}%)" if o and ps else f'{on} n/a')
            print(f'- K={K} {base}: ' + '; '.join(row))
    print('\nCluster sizes:\n')
    for K, r in A.items():
        s = r['sizes']
        print(f"- K={K}: plain max/mean {s['plain']['max_over_mean']:.1f}, CV {s['plain']['cv']:.2f}, largest 10% of clusters hold "
              f"{s['plain']['share_in_top10pct'] * 100:.1f}% of items, empty {s['plain']['empty']}; balanced max/mean "
              f"{s['balanced']['max_over_mean']:.1f}, CV {s['balanced']['cv']:.2f}, top 10% hold {s['balanced']['share_in_top10pct'] * 100:.1f}%")
    # plot
    Ks = list(A)
    fig, axs = plt.subplots(1, len(Ks), figsize=(5 * len(Ks), 4.2), sharey=True, squeeze=False); axs = axs[0]
    for ax, K in zip(axs, Ks):
        r = A[K]; N = None
        for run, v in r['runs'].items():
            if 'curve' not in v: continue
            st, on = run.split('/')
            if on not in ('centre', r[f"chosen_bound_{st.split('+')[0]}"]): continue
            c = v['curve']
            if N is None:
                N = v['test']['rt0.9']['scanned'] / v['test']['rt0.9']['share']
            ax.plot([x / N for x in c['su']], c['rt'], label=f'{st} {on}', ls='-' if on == 'centre' else '--',
                    color={'plain': 'C0', 'plain+soar': 'C1', 'balanced': 'C2', 'balanced+soar': 'C3'}[st])
        ax.set_xscale('log'); ax.set_title(f'{name} K={K}'); ax.set_xlabel('share of items scanned'); ax.grid(alpha=.3)
        ax.set_ylim(0.5, 1.0); ax.set_xlim(1e-3, 1)
    axs[0].set_ylabel('recall@10 (ANN-Benchmarks)'); axs[-1].legend(fontsize=7, loc='lower right')
    fig.tight_layout(); fig.savefig(f'{od}/expA_{name}.png', dpi=110)

for fn in sorted(glob.glob(f'{od}/expA_tree_*.json')):
    TR = json.load(open(fn)); name = os.path.basename(fn)[10:-5]
    print(f'\n### Centre hierarchy: {name}\n')
    print('Balanced k-means, single assignment, bound ordering of centres. For each target: the cheapest configuration '
          '(fewest centres scored per query) whose share scanned is within 1.1x of the flat pass, and the cheapest that reaches it at all.\n')
    print('| K | structure | h | 0.9: centres / share | 0.95: centres / share | 0.99: centres / share |')
    print('|---|---|---|---|---|---|')
    for K, r in TR.items():
        fl = r['flat']['test']
        print(f"| {K} | flat (f={r['f_leaf']}) | | " + ' | '.join(f"{K} / {fl[f'rt{R}']['share'] * 100:.2f}%" for R in TARGETS) + ' |')
        for depth, t in r['trees'].items():
            cells = []
            for R in TARGETS:
                ok = [w for w in t['rows'] if w['test'][f'rt{R}']]
                near = [w for w in ok if w['test'][f'rt{R}']['share'] <= 1.1 * fl[f'rt{R}']['share']]
                c1 = min(near, key=lambda w: w['centres_scored']) if near else None
                c2 = min(ok, key=lambda w: w['centres_scored']) if ok else None
                f = lambda w: 'n/a' if w is None else f"{w['centres_scored']:,.0f} / {w['test'][f'rt{R}']['share'] * 100:.2f}% (beam {w['beam']})"
                cells.append(f(c1) + ('' if c1 is c2 else f'; cheapest {f(c2)}'))
            print(f"| {K} | {depth} levels {t['sizes']} | {t['h']} | " + ' | '.join(cells) + ' |')

for fn in sorted(glob.glob(f'{od}/expB_*.json')):
    B = json.load(open(fn))
    print(f"\n### B: {B['name']} (N={B['N']:,}, n={B['n']}, K={B['K']}; {B['n_pseudo']} pseudo-queries, {B['n_test']} test queries: {B['queries']})\n")
    print('| bits | target | m fitted on pseudo | pseudo recall | test recall (strict) | gap | test re-ranks | m test needs | re-ranks at that m |')
    print('|---|---|---|---|---|---|---|---|---|')
    for b, c in B['codes'].items():
        for f in c['fits']:
            if 'm_fit' not in f: print(f"| {b} | {f['target']} | none on grid | | | | | | |"); continue
            print(f"| {b} ({c['bytes']:.0f} B) | {f['target']} | {f['m_fit']} | {f['pseudo_recall']:.4f} | {f['test_recall']:.4f} ({f['test_recall_strict']:.4f}) | "
                  f"{f['test_recall'] - f['target']:+.4f} | {f['test_rerank']:,.0f} | {f.get('m_oracle', 'n/a')} | {f.get('oracle_test_rerank', float('nan')):,.0f} |")
    for b, c in B['codes'].items():
        zp, zt = c['runs']['pseudo']['z'], c['runs']['test']['z']
        print(f"- {b} bit: estimate error / sd, pseudo: sd {zp['sd']:.3f}, |z|>3 {zp['gt3']:.2e}; test: sd {zt['sd']:.3f}, |z|>3 {zt['gt3']:.2e}")
