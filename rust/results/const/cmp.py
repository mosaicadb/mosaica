# usage: python3 cmp.py RESDIR BIN [BIN...]: per binary, the mean over runs t_BIN_* of each cell's median time, and its
# bytes per item and full-read share; the change against the first binary.
import glob, re, sys, collections
res, bins = sys.argv[1], sys.argv[2:]
d = collections.defaultdict(lambda: collections.defaultdict(list))
for b in bins:
    for f in sorted(glob.glob(f"{res}/t_{b}_*.out")):
        for line in open(f):
            if line.startswith("e2e2 a,"):
                p = line.split()[1].split(",")
                d[b][(p[1], float(p[4]))].append((float(p[5]), float(p[10]), float(p[12])))
print("runs:", {b: len(glob.glob(f'{res}/t_{b}_*.out')) for b in bins})
base = bins[0]
for key in sorted(d[base]):
    row = [f"{key[0]:4s} {key[1]*100:6.2f}%"]
    m0 = sum(x[0] for x in d[base][key]) / len(d[base][key])
    for b in bins:
        v = d[b][key]
        if not v:
            row.append("-"); continue
        m = sum(x[0] for x in v) / len(v)
        row.append(f"{b}: {m:7.2f} ms {100*(m/m0-1):+5.1f}% B {v[0][1]:6.1f} full {100*v[0][2]:.2f}%")
    print(" | ".join(row))
