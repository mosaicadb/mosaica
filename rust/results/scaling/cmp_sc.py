# Tables for the N sweep from RES/t_<n>_<size>_<rep>/{e2e2,breakdown2}.csv and RES/t_*.out; fit metadata from
# SC/meta_<n>_<size>.json. Model: wall ms per query = c0 + cg groups + ccut K + cop opened clusters + cu units
# + cil item-levels + cfr full reads, non-negative least squares relative to the measured time, on 500k and 1M.
import csv, glob, os, re, sys, collections, json
import numpy as np
from scipy.optimize import nnls
D = sys.argv[1]; R = f"{D}/res"; SC = f"{D}/sc"
FR = [0.0001, 0.001, 0.01, 0.1, 0.5, 0.75]
# k500, m1: the row-group samples (500k within 1M); r1m, r3m: uniform samples of the 5M population (R1M within R3M)
NS = ["k500", "m1", "r1m", "r3m"]; NN = {"k500": 499_495, "m1": 999_000, "r1m": 1_000_000, "r3m": 3_000_000}; SIZES = [1000, 2000, 4000, 8000]
knn = collections.defaultdict(list)
for d in sorted(glob.glob(f"{R}/knn_*_*")):                          # the separate k-NN processes of the row-group fits
    _, nn, size = os.path.basename(d).split("_")
    if os.path.exists(f"{d}/e2e2.csv"):
        for r in csv.DictReader(open(f"{d}/e2e2.csv")):
            if r["threads"] == "1" and r["what"].startswith("index2_knn"): knn[(nn, int(size), r["class"], int(float(r["answer_frac"])))].append(r)
e2e = collections.defaultdict(list); bd = collections.defaultdict(list); cnt = collections.defaultdict(list); rss = {}
for d in sorted(x for x in glob.glob(f"{R}/t_*_*_[12]") if os.path.exists(f"{x}/breakdown2.csv")):
    _, nn, size, rep = os.path.basename(d).split("_"); key = (nn, int(size))
    if (nn, size) == ("r3m", "2000"): continue  # timed in a later session (about 5% faster); cmp_r3m_session.py compares it
    for r in csv.DictReader(open(f"{d}/e2e2.csv")):
        if r["threads"] == "1" and r["what"] == "index2_thr": e2e[key + (r["class"], float(r["answer_frac"]))].append(r)
        if r["threads"] == "1" and r["what"].startswith("index2_knn"): knn[key + (r["class"], int(float(r["answer_frac"])))].append(r)
    for r in csv.DictReader(open(f"{d}/breakdown2.csv")):
        if r["threads"] == "1": bd[key + (r["class"], float(r["answer_frac"]))].append(r)
    for line in open(f"{d}.out"):
        m = re.match(r"breakdown2 counts \S+ (\S+) (\S+): per query ([\d.]+) clusters on the index path, ([\d.]+) on full vectors, ([\d.]+) items entering the first level, ([\d.]+) item-levels, ([\d.]+) groups, ([\d.]+) full reads", line)
        if m: cnt[key + (m[1], float(m[2]))].append([float(m[i]) for i in range(3, 9)])
        m = re.match(r"breakdown2 tables \S+ (\S+) (\S+): per query ([\d.]+) \(cluster, level\) tables, ([\d.]+) units", line)
        if m: cnt[("tab",) + key + (m[1], float(m[2]))].append([float(m[3]), float(m[4])])
        m = re.search(r"peak RSS ([\d.]+) GiB", line)
        if m: rss[(nn, int(size), rep)] = float(m[1])
mean = lambda v: sum(v) / len(v)
fits = [(nn, s) for nn in NS for s in SIZES if any(k[:2] == (nn, s) for k in e2e)]
meta = lambda nn, s: json.load(open(f"{SC}/meta_{nn}_{s}.json"))
print("## Fits\n\n| N | size | K | G | cluster sizes | group sizes | clusters per group | groups below n = 768 | cond_max | peak RSS (GiB) |\n|---|---|---|---|---|---|---|---|---|---|")
for nn, s in fits:
    m = meta(nn, s); st = m["stats"]
    rr = "/".join(f"{rss[x]:.1f}" for x in sorted(rss) if x[:2] == (nn, s))
    print(f"| {nn} | {s} | {m['K']} | {m['G']} | {st['cluster_min']}..{st['cluster_max']} | {st['group_min']}..{st['group_max']} | {st['cpg_min']}..{st['cpg_max']} | {st['groups_below_768']} | {m['fit']['cond_max']:.0f} | {rr} |")
# timing tables
spread_cells = []
for c in ["text", "item"]:
    print(f"\n## {c}: ms, mean of repeats [repeats]; change against 1,000 per cluster at the same N\n")
    print("| N | size | " + " | ".join(f"{f*100:g}%" for f in FR) + " |\n|---|---|" + "---|" * len(FR))
    for nn, s in fits:
        cells = []
        for f in FR:
            v = [float(r["median_of_runs_ms"]) for r in e2e[(nn, s, c, f)]]
            b = [float(r["median_of_runs_ms"]) for r in e2e.get((nn, 1000, c, f), [])]
            if len(v) > 1:
                sp = (max(v) - min(v)) / mean(v); spread_cells.append((sp, nn, s, c, f, v))
            cells.append(f"{mean(v):.1f}" + (f", {100*(mean(v)/mean(b)-1):+.0f}%" if b and s != 1000 else "") + f" [{'/'.join(f'{x:.1f}' for x in v)}]")
        print(f"| {nn} | {s} | " + " | ".join(cells) + " |")
spread_cells.sort(reverse=True)
print("\n## Noise: spread (max - min) / mean between repeats\n")
if spread_cells:
    print(f"largest {100*spread_cells[0][0]:.1f}% ({spread_cells[0][1:5]}); cells above 6%: " + ("; ".join(f"{x[1]} {x[2]} {x[3]} {x[4]*100:g}%: {100*x[0]:.1f}% {x[5]}" for x in spread_cells if x[0] > 0.06) or "none") + f"; {len(spread_cells)} cells with repeats, median spread {100*float(np.median([x[0] for x in spread_cells])):.1f}%")
# best size per N, class, answer
print("\n## Best size per N, class and answer (mean ms; margin = next best / best - 1)\n")
print("| N | class | " + " | ".join(f"{f*100:g}%" for f in FR) + " |\n|---|---|" + "---|" * len(FR))
for nn in NS:
    for c in ["text", "item"]:
        cells = []
        for f in FR:
            v = sorted((mean([float(r["median_of_runs_ms"]) for r in e2e[(nn, s, c, f)]]), s) for s in SIZES if (nn, s, c, f) in e2e)
            if not v: cells.append("–"); continue
            cells.append(f"{v[0][1]} ({v[0][0]:.1f})" + (f", +{100*(v[1][0]/v[0][0]-1):.0f}% to {v[1][1]}" if len(v) > 1 else ""))
        if any(x != "–" for x in cells): print(f"| {nn} | {c} | " + " | ".join(cells) + " |")
# counts across N at fixed size
print("\n## Counts per query across N (first repeat): touched share, entry share at first level, opened clusters (share of K), item-levels per item of N, full reads per item\n")
for c in ["text", "item"]:
    for f in FR:
        rows = []
        for s in SIZES:
            parts = []
            for nn in NS:
                k = (nn, s, c, f)
                if k not in e2e or k not in cnt: continue
                r = e2e[k][0]; cn = cnt[k][0]; K = meta(nn, s)["K"]; N = NN[nn]
                parts.append(f"{nn}: t {float(r['touched_frac']):.3f}, open {cn[0]:.0f} ({cn[0]/K:.2f}), il/N {cn[3]/N:.2f}, full {cn[5]/N*100:.3f}%")
            if parts: rows.append(f"  {s}: " + " | ".join(parts))
        if rows: print(f"{c} {f*100:g}%:\n" + "\n".join(rows))
# exponents in N at fixed size, per population (two points each: 500k -> 1M row groups; R1M -> R3M)
print("\n## Change with N at fixed size: log-log slope between the two N of one population (touched share / opened share of K / item-levels per item of N / full reads per item of N)\n")
for pa, pb in [("k500", "m1"), ("r1m", "r3m")]:
    for sz in SIZES:
        for c in ["text", "item"]:
            out = []
            for f in FR:
                ka, kb = (pa, sz, c, f), (pb, sz, c, f)
                if not all(k in cnt and k in e2e for k in (ka, kb)): continue
                dx = np.log(NN[pb] / NN[pa])
                v = lambda k: (float(e2e[k][0]["touched_frac"]), cnt[k][0][0] / meta(k[0], sz)["K"], cnt[k][0][3] / NN[k[0]], cnt[k][0][5] / NN[k[0]])
                A, B = v(ka), v(kb)
                out.append(f"{f*100:g}%: " + "/".join(f"{np.log(max(y, 1e-12) / max(x, 1e-12)) / dx:+.2f}" for x, y in zip(A, B)))
            if out: print(f"{pa}->{pb} {sz} {c}: " + "; ".join(out))
# model: each profiled phase against its count, fitted on 500k and 1M (row groups); wall = s x profiled sum
PH = {"groups": ["basis", "rotations", "group post"], "cut": ["cut"], "cluster": ["cluster setup", "tables", "level setup"],
      "item": ["re-pack", "level decode", "bounds+decisions", "compaction", "level other"], "full": ["full vectors"],
      "other": ["anchor", "candidate init", "result bitmap", "column order", "cluster other", "merge", "query other", "unattributed"]}
rows = []
for k, rs in bd.items():
    if k not in cnt or ("tab",) + k not in cnt: continue
    nn, sz, c, f = k
    cn = [mean(x) for x in zip(*cnt[k])]; tb = [mean(x) for x in zip(*cnt[("tab",) + k])]
    ph = {n: mean([sum(float(r[x]) for x in cols) for r in rs]) for n, cols in PH.items()}
    rows.append(dict(k=k, K=meta(nn, sz)["K"], g=cn[4], op=cn[0], u=tb[1], il=cn[3], fr=cn[5], ent=cn[2], ph=ph,
                     prof=mean([float(r["prof_wall_ms"]) for r in rs]), wall=mean([float(r["median_of_runs_ms"]) for r in e2e[k]])))
train = [r for r in rows if r["k"][0] in ("k500", "m1")]
if len(train) >= 8:
    def ls(X, y, nonneg=True):
        X, y = np.array(X, float), np.array(y, float); w = 1 / np.maximum(y, 1e-3)
        return nnls(X * w[:, None], y * w)[0]
    cg = ls([[r["g"]] for r in train], [r["ph"]["groups"] for r in train])
    cc = ls([[r["K"]] for r in train], [r["ph"]["cut"] for r in train])
    ccl = ls([[r["op"], r["u"]] for r in train], [r["ph"]["cluster"] for r in train])
    cil = ls([[r["il"]] for r in train], [r["ph"]["item"] for r in train])
    cfr = ls([[r["fr"]] for r in train], [r["ph"]["full"] for r in train])
    cot = ls([[1, r["ent"]] for r in train], [r["ph"]["other"] for r in train])
    pp = lambda r: cg[0] * r["g"] + cc[0] * r["K"] + ccl[0] * r["op"] + ccl[1] * r["u"] + cil[0] * r["il"] + cfr[0] * r["fr"] + cot[0] + cot[1] * r["ent"]
    print("\n## Model (fitted on the 500k and 1M row-group breakdowns: each profiled phase against its count, non-negative least squares relative to the phase time; it predicts the profiled time; the wall time is that times the median wall/profiled ratio of the training cells, per class)\n")
    print(f"group {cg[0]*1e3:.1f} us per group; cut {cc[0]*1e3:.2f} us per cluster; opened cluster {ccl[0]*1e3:.2f} us + {ccl[1]*1e6:.1f} ns per table unit; "
          f"{cil[0]*1e6:.2f} ns per item-level; {cfr[0]*1e6:.1f} ns per full read; other {cot[0]:.2f} ms + {cot[1]*1e6:.2f} ns per item entering (all profiled)")
    ratio = {c: float(np.median([r["wall"] / r["prof"] for r in train if r["k"][2] == c])) for c in ("item", "text")}
    print("wall/profiled ratio: " + ", ".join(f"{c} {v:.3f}" for c, v in ratio.items()))
    pred = lambda r: ratio[r["k"][2]] * pp(r)
    json.dump(dict(group_us=cg[0]*1e3, cut_us=cc[0]*1e3, cluster_us=ccl[0]*1e3, unit_ns=ccl[1]*1e6, itemlevel_ns=cil[0]*1e6, full_ns=cfr[0]*1e6, other_ms=cot[0], other_ns=cot[1]*1e6, ratio=ratio), open(f"{SC}/model.json", "w"), indent=1)
    perrs = collections.defaultdict(list)
    for r in rows: perrs[r["k"][0]].append(abs(pp(r) / r["prof"] - 1))
    print("profiled prediction against profiled measurement: " + "; ".join(f"{nn}: median |error| {100*np.median(v):.1f}%, largest {100*max(v):.1f}%" for nn, v in perrs.items()))
    errs = collections.defaultdict(list)
    print("\n| N | size | class | answer | measured ms | predicted ms | error |\n|---|---|---|---|---|---|---|")
    for r in sorted(rows, key=lambda r: (NS.index(r["k"][0]), r["k"][2], r["k"][3], r["k"][1])):
        nn, sz, c, f = r["k"]; pw = pred(r); e = pw / r["wall"] - 1; errs[nn].append(abs(e))
        print(f"| {nn} | {sz} | {c} | {f*100:g}% | {r['wall']:.1f} | {pw:.1f} | {100*e:+.0f}% |")
    print("\n" + "; ".join(f"{nn}: median |error| {100*np.median(v):.1f}%, largest {100*max(v):.1f}% ({'in sample' if nn in ('k500', 'm1') else 'out of sample'})" for nn, v in errs.items()))
# breakdown summary
PC = ["cut", "cluster setup", "tables", "level setup"]; PI = ["re-pack", "level decode", "bounds+decisions", "compaction", "level other"]
print("\n## Breakdown (profiled ms per query, mean of repeats) and counts\n")
print("| N | size | cell | wall | prof | groups ms | per-cluster ms | per-item ms | full ms | groups | opened (flat) | tables, units | entering | item-levels | full reads |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
for c, f in [("text", 0.0001), ("text", 0.01), ("text", 0.5), ("item", 0.01), ("item", 0.5)]:
    for nn, s in fits:
        k = (nn, s, c, f)
        if k not in bd or k not in cnt: continue
        g = lambda x: mean([float(r[x]) for r in bd[k]])
        cn = [mean(x) for x in zip(*cnt[k])]; tb = [mean(x) for x in zip(*cnt[("tab",) + k])]
        print(f"| {nn} | {s} | {c} {f*100:g}% | {g('wall_ms'):.1f} | {g('prof_wall_ms'):.1f} | {g('basis')+g('rotations')+g('group post'):.2f} | {sum(g(x) for x in PC):.2f} | {sum(g(x) for x in PI):.2f} | {g('full vectors'):.2f} | {cn[4]:.0f} | {cn[0]:.0f} ({cn[1]:.0f}) | {tb[0]:.0f}, {tb[1]:.0f} | {cn[2]:.0f} | {cn[3]:.0f} | {cn[5]:.0f} |")

print("\n## Exact k-NN: ms, mean of runs [runs]; bytes per item; change against 1,000 per cluster at the same N\n")
print("| N | size | item k10 | item k100 | text k10 | text k100 |\n|---|---|---|---|---|---|")
for nn in NS:
    for sz in SIZES:
        cells = []
        for c, k in [("item", 10), ("item", 100), ("text", 10), ("text", 100)]:
            v = [float(r["median_of_runs_ms"]) for r in knn.get((nn, sz, c, k), [])]; b = [float(r["median_of_runs_ms"]) for r in knn.get((nn, 1000, c, k), [])]
            if not v: cells.append("–"); continue
            cells.append(f"{mean(v):.2f}" + (f", {100*(mean(v)/mean(b)-1):+.0f}%" if b and sz != 1000 else "") + f" [{'/'.join(f'{x:.2f}' for x in v)}], {float(knn[(nn, sz, c, k)][0]['bytes_read_per_item']):.1f} B")
        if any(x != "–" for x in cells): print(f"| {nn} | {sz} | " + " | ".join(cells) + " |")
