# Compact tables: a pass-by-pass table for item 1, and a difference table of partition B against A with the seed rule.
# usage: tab_compare.py passes NAME...   |   tab_compare.py diff A B [KEY...]
import json, sys
W = 'work/scores/'
S = [json.load(open(W + f'A{s}.json')) for s in (1, 2, 3)]
sp = lambda k: max(x[k] for x in S) - min(x[k] for x in S)
DEF = ['Si_S1d_16', 'Si_S1b_0.06', 'Si_S1d_4', 'Si_S1d_32', 'Si_S1b_0.03', 'Si_S1b_0.12', 'Si_S1a_mean', 'Si_S1a_rms',
       'Sm_S1d_16', 'Sm_S1b_0.06']
if sys.argv[1] == 'passes':
    print("| partition | pass | fit rows moved in the pass | S0 (all valid rows) | mean key | mean sq. residual | S1d 16 B (Si) | S1b w*=.06 (Si) | S2+3 item .01% | S2+3 item 1% | S2+3 species 1% | S4a text | S4a item | smallest group (fit rows) |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for r in sys.argv[2:]:
        d = json.load(open(W + r + '.json'))
        print(f"| {r} | {d.get('pass', 0)} | {d.get('moved_fit', 0):.4f} | {d['S0_all']:.4f} | {d['mean_key']:.5f} | {d['msq_resid']:.5f} | {d['Si_S1d_16']:.5f} | {d['Si_S1b_0.06']:.3f} | {d['S23_item_0.01%']:.4f} | {d['S23_item_1%']:.4f} | {d['S23_species_1%']:.4f} | {d['S4a_mean_text']:.4f} | {d['S4a_mean_item']:.4f} | {d['group_fit_rows_min']} |")
    print(f"| seed spread (A1–A3) | | | {sp('S0_all'):.4f} | {sp('mean_key'):.5f} | {sp('msq_resid'):.5f} | {sp('Si_S1d_16'):.5f} | {sp('Si_S1b_0.06'):.3f} | {sp('S23_item_0.01%'):.4f} | {sp('S23_item_1%'):.4f} | {sp('S23_species_1%'):.4f} | {sp('S4a_mean_text'):.4f} | {sp('S4a_mean_item'):.4f} | |")
else:
    a = json.load(open(W + sys.argv[2] + '.json')); f = json.load(open(W + sys.argv[3] + '.json'))
    keys = sys.argv[4:] or DEF
    print(f"| score | {sys.argv[2]} | {sys.argv[3]} | difference | relative | 2 × seed spread | a result? |")
    print("|---|---|---|---|---|---|---|")
    for k in keys:
        d = f[k] - a[k]; s2 = 2 * sp(k)
        print(f"| {k} | {a[k]:.5f} | {f[k]:.5f} | {d:+.5f} | {d / a[k] * 100:+.2f}% | {s2:.5f} | {'yes' if abs(d) > s2 else 'no: under twice the seed spread'} |")
