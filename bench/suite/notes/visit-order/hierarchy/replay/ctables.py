# A compact markdown summary of calib.py's printed checks.   usage: python3 -I ctables.py DIR
import sys, os, re

DIR = sys.argv[1]
print("| dataset | level, tier | beta | delta: plain / shifted / real | ln(obs/pred): plain / shifted / real | ln kappa at k = 10 (answer fraction 1e-5), median [10%, 90%]: plain; shifted; real | ln kappa at 1%: real |")
print("|---|---|---|---|---|---|---|")
filt = []
for ds in ("laion", "glove", "yahoo"):
    p = os.path.join(DIR, f"calib_{ds}.txt")
    if not os.path.exists(p):
        continue
    cur, rows = None, {}
    for ln in open(p):
        m = re.match(r"-- level (\w+), tier (\w+): beta ([-+.\d]+)", ln)
        if m:
            cur = (m.group(1), m.group(2))
            rows[cur] = dict(beta=m.group(3), f={})
            continue
        m = re.match(r"(plain|shifted|real) +delta ([.\d]+); ln\(obs/pred\) overall ([-+.\d]+); .*?1e-05: (\S+ \[\S+ \S+\]); 0.001: (\S+ \[\S+ \S+\]); 0.01: (\S+ \[\S+ \S+\])", ln)
        if m and cur:
            rows[cur][m.group(1)] = m.groups()[1:]
            continue
        m = re.match(r"real, (\S+) +delta ([.\d]+); ln\(obs/pred\) ([-+.\d]+); ln kappa at k = 10: (\S+ \[\S+ \S+\]); at 1% of the visible: (\S+ \[\S+ \S+\])", ln)
        if m and cur:
            rows[cur]["f"][m.group(1)] = m.groups()[1:]
    for (lv, tr), r in rows.items():
        g = lambda i: " / ".join(r[k][i] for k in ("plain", "shifted", "real"))
        print(f"| {ds} | {lv}, {tr} | {r['beta']} | {g(0)} | {g(1)} | {'; '.join(r[k][2] for k in ('plain', 'shifted', 'real'))} | {r['real'][4]} |")
        filt.append((ds, lv, tr, r["f"]))
names = ["random-1", "random-25", "clus-1", "clus-25", "cap-1", "cap-25"]
print("\nReal queries under visible sets: delta; ln kappa at k = 10, median [10%, 90%]\n")
print("| dataset | level, tier | " + " | ".join(names) + " |")
print("|---|---|" + "---|" * len(names))
for ds, lv, tr, f in filt:
    print(f"| {ds} | {lv}, {tr} | " + " | ".join(f"{f[n][0]}; {f[n][2]}" if n in f else "-" for n in names) + " |")
