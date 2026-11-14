import re, sys, statistics as st
R='/home/joe/code/vexact-setup-target/res/'  # the runs' .out files
def load(tag):
    d={}
    for line in open(R+tag+'.out'):
        if line.startswith('e2e2 a,'):
            f=line[5:].split(',')
            d[(f[1],f[4] if f[3]=='index2_thr' else 'knn10')]=(float(f[5]),float(f[10]))
    return d
base=[load(t) for t in sys.argv[1].split(',')]
new=[load(t) for t in sys.argv[2].split(',')]
print("| Queries | Answer | Baseline ms (runs) | New ms (runs) | Change | Bytes/item |")
print("|---|---|---|---|---|---|")
for cls in ['item','text']:
    for a in ['0.0001','0.001','0.01','0.1','0.3','0.5','0.75','knn10']:
        k=(cls,a)
        if not all(k in x for x in base+new): continue
        b=[x[k][0] for x in base]; n=[x[k][0] for x in new]
        mb, mn = st.median(b), st.median(n)
        lab = 'k-NN 10' if a=='knn10' else f"{float(a)*100:g}%"
        print(f"| {cls} | {lab} | {mb:.1f} ({' / '.join(f'{x:.1f}' for x in b)}) | {mn:.1f} ({' / '.join(f'{x:.1f}' for x in n)}) | {100*(mn/mb-1):+.0f}% | {base[0][k][1]:.1f} / {new[0][k][1]:.1f} |")
