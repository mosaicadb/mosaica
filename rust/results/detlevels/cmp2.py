# Tables for the report. Baseline pool: the pristine binary's runs and the "all" variant (ddec=all, identical logic) of
# the b1 and b2 processes. Noise floor: (max - min) / mean over the pool's runs.
import csv, glob, os, sys, collections
D = sys.argv[1]
NAMES = {("base","a"):"base", ("b1","a"):"base", ("b1","b"):"B {4,24,32}", ("b1","c"):"B {4,32}",
         ("b2","a"):"base", ("b2","b"):"B {16,32}", ("b2","c"):"B {32}",
         ("c3","a"):"C 4/16/32", ("c3","b"):"C 4/16/32, decide {4,32}", ("c3","c"):"C 4/16/32, decide {32}",
         ("c2a","a"):"C 8/32", ("c2a","b"):"C 8/32, decide {32}", ("c2b","a"):"C 16/32", ("c2b","b"):"C 16/32, no det. decisions"}
ms = collections.defaultdict(list); inproc = collections.defaultdict(list); info = {}
runs = collections.defaultdict(list)
for f in sorted(glob.glob(f"{D}/res/t_*_[12]/e2e2.csv")):
    tag = os.path.basename(os.path.dirname(f)); _, proc, rep = tag.split("_")
    rs = [r for r in csv.DictReader(open(f)) if r["threads"] == "1"]
    a = {(r["class"], float(r["answer_frac"])): float(r["median_of_runs_ms"]) for r in rs if r["variant"] == "a"}
    for r in rs:
        k = (NAMES[(proc, r["variant"])], r["class"], float(r["answer_frac"]))
        v = float(r["median_of_runs_ms"]); ms[k].append(v)
        if proc in ("b1", "b2") and r["variant"] != "a":
            inproc[k].append(v / a[k[1:]] - 1)
        info.setdefault(k, r)
FR = [0.0001, 0.001, 0.01, 0.1, 0.5, 0.75]
names = list(dict.fromkeys(k[0] for k in ms))
mean = lambda v: sum(v) / len(v)
for c in ["item", "text"]:
    print(f"\n### {c} queries: ms (mean of repeats) and change against the baseline pool")
    print("| Variant | " + " | ".join(f"{f*100:g}%" for f in FR) + " |\n|---|" + "---|" * len(FR))
    for n in names:
        cells = []
        for f in FR:
            v = ms[(n, c, f)]; b = ms[("base", c, f)]
            if n == "base":
                cells.append(f"{mean(v):.1f} (n={len(v)}, spread {100*(max(v)-min(v))/mean(v):.0f}%)")
            else:
                ip = inproc.get((n, c, f))
                cells.append(f"{mean(v):.1f}, {100*(mean(v)/mean(b)-1):+.0f}%" + (f" (in-process {100*mean(ip):+.0f}%)" if ip else "") + (f" [{'/'.join(f'{x:.1f}' for x in v)}]" if len(v) > 1 else ""))
        print(f"| {n} | " + " | ".join(cells) + " |")
    print(f"\n{c}: bytes per item (change), full-vector share")
    print("| Variant | " + " | ".join(f"{f*100:g}%" for f in FR) + " |\n|---|" + "---|" * len(FR))
    for n in names:
        cells = []
        for f in FR:
            r = info[(n, c, f)]; b = info[("base", c, f)]
            by, bb = float(r["bytes_read_per_item"]), float(b["bytes_read_per_item"])
            cells.append(f"{by:.1f}" + ("" if n == "base" else f" ({100*(by/bb-1):+.0f}%)") + f", {100*float(r['full_reads_per_item']):.2f}%")
        print(f"| {n} | " + " | ".join(cells) + " |")
print("\n### Entry shares (share of N entering each level, last entry: to full vectors from the level walk)")
for c in ["item", "text"]:
    for f in [0.01, 0.5]:
        print(f"\n{c} {f*100:g}%:")
        for n in names:
            print(f"  {n:28s} " + " ".join(f"{float(x):.3f}" for x in info[(n, c, f)]["enter_frac_by_level"].split(";")))
