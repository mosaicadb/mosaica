# Compare e2e2 CSVs of stages: bytes/item and median ms (1 and 4 threads) per class and cell.
# usage: python3 cmp.py LABEL=FILE.csv ...
import csv, sys
runs = [a.split("=", 1) for a in sys.argv[1:]]
data = {}
for lab, f in runs:
    for r in csv.DictReader(open(f)):
        if not r["what"].startswith("index2"):
            continue
        cell = "knn" if "knn" in r["what"] else r["answer_frac"]
        data.setdefault((r["class"], cell), {})[(lab, r["threads"])] = r
labs = [l for l, _ in runs]
print("class cell | " + " | ".join(f"{l}: B/item, ms1 [min-max], ms4 [min-max]" for l in labs))
for (cls, cell), d in data.items():
    out = []
    for l in labs:
        r1, r4 = d.get((l, "1")), d.get((l, "4"))
        if not r1:
            out.append("-"); continue
        s = f"{float(r1['bytes_read_per_item']):.2f}, {float(r1['median_of_runs_ms']):.2f} [{float(r1['min_run_ms']):.2f}-{float(r1['max_run_ms']):.2f}]"
        if r4:
            s += f", {float(r4['median_of_runs_ms']):.2f} [{float(r4['min_run_ms']):.2f}-{float(r4['max_run_ms']):.2f}]"
        out.append(s)
    print(f"{cls} {cell} | " + " | ".join(out))
