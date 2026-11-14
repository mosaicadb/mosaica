# usage: cmp.py BASE_TAG[,BASE_TAG..] NEW_TAG[,NEW_TAG..]: mean over runs of each side's per-run median ms, bytes, full share
# (TAG:V reads variant V of an interleaved run; default a)
import sys, csv, statistics as st
R = '/home/joe/code/vexact-bounds-target/res'
def load(tags):
    d = {}
    for t in tags.split(','):
        t, _, v = t.partition(':')
        for r in csv.DictReader(open(f'{R}/{t}/e2e2.csv')):
            if r['variant'] != (v or 'a'): continue
            k = (r['class'], float(r['answer_frac']))
            d.setdefault(k, []).append((float(r['median_of_runs_ms']), float(r['bytes_read_per_item']), float(r['full_reads_per_item'])))
    return {k: tuple(st.mean(x[i] for x in v) for i in range(3)) for k, v in d.items()}
a, b = load(sys.argv[1]), load(sys.argv[2])
print("class answer | ms base new delta | B/item base new | full share base new")
for k in sorted(a):
    x, y = a[k], b[k]
    print(f"{k[0]} {k[1]*100:g}% | {x[0]:.1f} {y[0]:.1f} {100*(y[0]/x[0]-1):+.0f}% | {x[1]:.1f} {y[1]:.1f} | {100*x[2]:.2f}% {100*y[2]:.2f}%")
