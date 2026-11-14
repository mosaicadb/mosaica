# Tables for the cluster-size sweep (fit metadata from D/cl/meta_<fit>.json), from RES/s_<fit>_<rep>/{e2e2,breakdown2}.csv and RES/s_<fit>_<rep>.out.
import csv, glob, os, re, sys, collections, json
D = sys.argv[1]; R = f"{D}/res"
FITS = ["base", "p4", "p16", "d4", "d16"]
IDX = {"base": f"{D}/c1m", **{f: f"{D}/cl/{f}" for f in FITS[1:]}}
FR = [0.0001, 0.001, 0.01, 0.1, 0.5, 0.75]
e2e = collections.defaultdict(list); bd = collections.defaultdict(list); cnt = collections.defaultdict(list)
for d in sorted(x for x in glob.glob(f"{R}/s_*_[12]") if os.path.exists(f"{x}/breakdown2.csv")):
    _, fit, rep = os.path.basename(d).split("_")
    for r in csv.DictReader(open(f"{d}/e2e2.csv")):
        if r["threads"] == "1":
            e2e[(fit, r["class"], float(r["answer_frac"]))].append(r)
    if os.path.exists(f"{d}/breakdown2.csv"):
        for r in csv.DictReader(open(f"{d}/breakdown2.csv")):
            if r["threads"] == "1":
                bd[(fit, r["class"], float(r["answer_frac"]))].append(r)
    for line in open(f"{d}.out"):
        m = re.match(r"breakdown2 counts \S+ (\S+) (\S+): per query ([\d.]+) clusters on the index path, ([\d.]+) on full vectors, ([\d.]+) items entering the first level, ([\d.]+) item-levels", line)
        if m:
            cnt[(fit, m[1], float(m[2]))].append([float(m[i]) for i in range(3, 7)])
        m = re.match(r"breakdown2 tables \S+ (\S+) (\S+): per query ([\d.]+) \(cluster, level\) tables, ([\d.]+) units", line)
        if m:
            cnt[("tab", fit, m[1], float(m[2]))].append([float(m[3]), float(m[4])])
fits = [f for f in FITS if any(k[0] == f for k in e2e)]
mean = lambda v: sum(v) / len(v)
print("## Fits\n\n| Fit | K | G | cluster sizes | group sizes | cond_max |\n|---|---|---|---|---|---|")
for f in fits:
    m = json.load(open(f"{D}/cl/meta_{f}.json")); log = open(f"{D}/fit1m.log" if f == "base" else f"{D}/cl/fit_{f}.log").read()
    cs = re.search(r"sizes (\d+)\.\.(\d+)\)", log); gs = re.search(r"group sizes (\d+) \.\. (\d+)", log)
    print(f"| {f} | {m['K']} | {m['G']} | {cs[1]}..{cs[2]} | {gs[1]}..{gs[2]} | {m['fit']['cond_max']:.0f} |")
for c in ["text", "item"]:
    print(f"\n## {c}: ms (mean of repeats; change against base; [repeats])\n\n| Fit | " + " | ".join(f"{f*100:g}%" for f in FR) + " |\n|---|" + "---|" * len(FR))
    for fit in fits:
        cells = []
        for f in FR:
            v = [float(r["median_of_runs_ms"]) for r in e2e[(fit, c, f)]]; b = [float(r["median_of_runs_ms"]) for r in e2e[("base", c, f)]]
            sp = f" spread {100*(max(v)-min(v))/mean(v):.0f}%"
            cells.append(f"{mean(v):.1f}" + ("" if fit == "base" else f", {100*(mean(v)/mean(b)-1):+.0f}%") + f" [{'/'.join(f'{x:.1f}' for x in v)}]" + (sp if fit == "base" else ""))
        print(f"| {fit} | " + " | ".join(cells) + " |")
    print(f"\n{c}: bytes per item, full-vector share, touched share, gap (full reads minus last entry share)\n\n| Fit | " + " | ".join(f"{f*100:g}%" for f in FR) + " |\n|---|" + "---|" * len(FR))
    for fit in fits:
        cells = []
        for f in FR:
            r = e2e[(fit, c, f)][0]; last = float(r["enter_frac_by_level"].split(";")[-1])
            cells.append(f"{float(r['bytes_read_per_item']):.1f} B, {100*float(r['full_reads_per_item']):.2f}%, t {float(r['touched_frac']):.3f}, gap {100*(float(r['full_reads_per_item'])-last):.2f}%")
        print(f"| {fit} | " + " | ".join(cells) + " |")
    for f in [0.0001, 0.01, 0.5]:
        print(f"\n{c} {f*100:g}% entry shares:")
        for fit in fits:
            print(f"  {fit:5s} " + " ".join(f"{float(x):.3f}" for x in e2e[(fit, c, f)][0]["enter_frac_by_level"].split(";")))
PC = ["cut", "cluster setup", "tables", "level setup"]; PI = ["re-pack", "level decode", "bounds+decisions", "compaction", "level other", "full vectors"]
FX = ["anchor", "basis", "rotations", "group post", "candidate init", "result bitmap"]
print("\n## Breakdown (profiled ms per query, mean of repeats)\n")
print("| Cell | Fit | wall | prof | " + " | ".join(PC) + " | " + " | ".join(PI) + " | basis+rot+gpost | idx clusters | flat clusters | tables | units | items entering | item-levels | µs per opened cluster (setup+tables+lvsetup) | cut µs per cluster | ns per item-level (repack..level other) |")
print("|---|---|" + "---|" * (2 + len(PC) + len(PI) + 9))
for (cls, f) in [("text", 0.0001), ("text", 0.01), ("text", 0.5), ("item", 0.01), ("item", 0.5)]:
    for fit in fits:
        rs = bd.get((fit, cls, f))
        if not rs: continue
        g = lambda k: mean([float(r[k]) for r in rs])
        cn = [mean(x) for x in zip(*cnt[(fit, cls, f)])]; tb = [mean(x) for x in zip(*cnt[("tab", fit, cls, f)])]
        K = json.load(open(f"{D}/cl/meta_{fit}.json"))["K"]
        pcl = (g("cluster setup") + g("tables") + g("level setup")) * 1e3 / max(cn[0], 1)
        pit = sum(g(k) for k in PI[:5]) * 1e6 / max(cn[3], 1)
        print(f"| {cls} {f*100:g}% | {fit} | {g('wall_ms'):.1f} | {g('prof_wall_ms'):.1f} | " + " | ".join(f"{g(k):.2f}" for k in PC) + " | " + " | ".join(f"{g(k):.2f}" for k in PI)
              + f" | {g('basis')+g('rotations')+g('group post'):.2f} | {cn[0]:.0f} | {cn[1]:.0f} | {tb[0]:.0f} | {tb[1]:.0f} | {cn[2]:.0f} | {cn[3]:.0f} | {pcl:.1f} | {g('cut')*1e3/K:.2f} | {pit:.1f} |")

# Do the gains stack: NDL 2 with --dlv 16,32 (RES/k_<fit>_<rep>) against the six-level runs of the same fit.
ks = collections.defaultdict(list)
for d in sorted(glob.glob(f"{R}/k_*_[12]")):
    if not os.path.exists(f"{d}/e2e2.csv"): continue
    _, fit, rep = os.path.basename(d).split("_")
    for r in csv.DictReader(open(f"{d}/e2e2.csv")):
        if r["threads"] == "1":
            ks[(fit, r["class"], float(r["answer_frac"]))].append(r)
if ks:
    for c in ["text", "item"]:
        print(f"\n## Stacking, {c}: ms (change against six-level base fit) [repeats], bytes per item\n\n| Variant | " + " | ".join(f"{f*100:g}%" for f in FR) + " |\n|---|" + "---|" * len(FR))
        for name, src, fit in [("base, 6 levels", e2e, "base"), ("base, 16/32", ks, "base"), ("p4, 6 levels", e2e, "p4"), ("p4, 16/32", ks, "p4")]:
            cells = []
            for f in FR:
                rs = src.get((fit, c, f))
                if not rs: cells.append("–"); continue
                v = [float(r["median_of_runs_ms"]) for r in rs]; b = [float(r["median_of_runs_ms"]) for r in e2e[("base", c, f)]]
                cells.append(f"{mean(v):.1f}, {100*(mean(v)/mean(b)-1):+.0f}% [{'/'.join(f'{x:.1f}' for x in v)}], {float(rs[0]['bytes_read_per_item']):.1f} B")
            print(f"| {name} | " + " | ".join(cells) + " |")
