# Tables for orders5.py: spread accuracy, per-dataset recall, leave-one-dataset-out choice of (alpha, w) per spread,
# worst-case losses, query-time cost, and the stopping rule.
# usage: python -I table5.py RESDIR OUT.txt MAIN1,... [EXTRA1,...]
import sys, os, json
import numpy as np

rd, outp = sys.argv[1], sys.argv[2]
main = sys.argv[3].split(",")
extra = sys.argv[4].split(",") if len(sys.argv) > 4 else []
R = {n: json.load(open(os.path.join(rd, f"{n}.json"))) for n in main + extra}
SH = ["0.005", "0.016", "0.063", "0.25"]
ALPHAS, WS = (0.5, 0.75, 1.0), (0.0, 0.1, 0.2, 0.3, 0.4)
SPREADS = ["own r16", "own r4", "sh16", "sh32", "sh64", "sh128", "sh32+own2", "sh64+own4", "group", "exact"]
L = []
p = L.append
get = lambda r, k: r["orders"][f"rad f={r['oracle']}"] if k == "oracle" else r["orders"][k]
avg = lambda n, k: get(R[n], k)["avg"]

p("score: recall@10 at 0.5 / 1.6 / 6.3 / 25% scanned and the mean over 0.5%..25% (avg)")
p("\nspread accuracy on pseudo-queries: rms relative error of sigma_c, plain / shifted; shared basis: share of the residual variance it spans")
seen = set()
for n in main + extra:
    ds = R[n]["dataset"]
    if ds in seen:
        continue
    seen.add(ds)
    r = R[n]
    p(f"  {ds:28s} " + "  ".join(f"{k} {v['plain']:.3f}/{v['shifted']:.3f}" for k, v in r["sigma_err"].items() if k != "exact"))
    p(f"  {'':28s} shared span " + ", ".join(f"{k} (R {r['R_actual'][k]}) {v:.2f}" for k, v in r["shared_var_share"].items()) +
      "; rad-aware model beta " + ", ".join(f"{k.replace(' beta', '')} {v['beta']:+.2f}" for k, v in r["models"].items() if k.endswith(" beta")))

# leave-one-dataset-out choice of (alpha, w) for each spread's H form
glob = {}
p("\nH form per spread: (alpha, w) chosen on all main sets, and leave-one-dataset-out losses (held-out set's own best minus LOO choice)")
for s in SPREADS:
    keys = [f"H {s} a{a} w{w}" for a in ALPHAS for w in WS]
    M = np.array([[avg(n, k) for k in keys] for n in main])
    gi = int(np.argmax(M.mean(0)))
    glob[s] = keys[gi]
    loo = []
    for j in range(len(main)):
        o = np.delete(M, j, 0).mean(0)
        loo.append(M[j].max() - M[j][int(np.argmax(o))])
    p(f"  {s:10s} global {keys[gi].split(s + ' ')[1]:10s}  LOO losses " + " ".join(f"{x:.4f}" for x in loo) + f"  (max {max(loo):.4f})")

rows = [("H own r16 a0.75 w0.3", "H r16 (as proposed)"), ("H own r4 a0.5 w0.3", "H r4 (as proposed)"), ("O2 f'=0.4", "f'=0.4")]
rows += [(glob[s], f"H {s} (LOO-chosen)") for s in SPREADS]
rows += [(f"lin {s} 3.3", f"mu+3.3 sigma {s}") for s in ("own r16", "sh32", "sh64", "sh64+own4", "group")]
rows += [(f"model {s} {b}", f"rad-model {s} {b}") for s in ("own r16", "sh32", "sh64", "sh64+own4", "group", "exact") for b in ("beta", "nobeta")]
rows += [("oracle", "oracle"), ("faiss nlist=K", "faiss nlist=K")]
for n in main + extra:
    r = R[n]
    p(f"\n{n} (K {r['K']}, nq {r['nq']}, oracle f {r['oracle']})")
    for k, lab in rows:
        o = get(r, k)
        p(f"  {lab:34s} " + " ".join(f"{o['fixed'][s]:.3f}" for s in SH) + f"   avg {o['avg']:.3f}   [{k}]")


def losses(sets, title):
    p(f"\n{title}: worst loss against the oracle (avg / 1.6% / 6.3%; negative = better) over {', '.join(sets)}; "
      "then against H r16 (avg: worst, mean)")
    out = []
    for k, lab in rows:
        if k in ("oracle",):
            continue
        la = [get(R[n], "oracle")["avg"] - get(R[n], k)["avg"] for n in sets]
        l16 = [get(R[n], "oracle")["fixed"]["0.016"] - get(R[n], k)["fixed"]["0.016"] for n in sets]
        l63 = [get(R[n], "oracle")["fixed"]["0.063"] - get(R[n], k)["fixed"]["0.063"] for n in sets]
        lh = [get(R[n], "H own r16 a0.75 w0.3")["avg"] - get(R[n], k)["avg"] for n in sets]
        out.append((max(la), lab, la, l16, l63, lh))
    for w, lab, la, l16, l63, lh in sorted(out):
        p(f"  {lab:34s} {max(la):+.3f} / {max(l16):+.3f} / {max(l63):+.3f}   per set " + " ".join(f"{x:+.3f}" for x in la) +
          f"   vs H r16 worst {max(lh):+.3f} mean {np.mean(lh):+.3f}")


losses(main, "main sets")
if extra:
    losses(extra, "extra sets")

# query-time cost at LAION 1M (K 500, d 512, G 64): beyond q.m (K d = 256k MACs, 256 KB int8 centres)
K, d, Gn = 500, 512, 64
p(f"\nquery-time cost at LAION 1M (K {K}, d {d}, G {Gn}), beyond q.m for every cluster (K d = {K * d / 1e3:.0f}k multiply-adds, "
  f"{K * d / 1024:.0f} KB of int8 centres); for scale, scanning 1.6% of the rows' f16 vectors is {16000 * d / 1e6:.1f}M multiply-adds "
  f"and {16000 * d * 2 / 2**20:.0f} MB")
p(f"  {'order':26s} {'multiply-adds':>14s} {'bytes read':>12s}   special functions")
cost = [("f'=0.4 (O2)", 2 * K, 4 * K, "none (sd of q.m over clusters: K)"),
        ("H own r4", K * 4 * d + 2 * K, K * 4 * d * 2 + K * 4 * 4 + 8 * K, "K sqrt"),
        ("H own r16", K * 16 * d + 2 * K, K * 16 * d * 2 + K * 16 * 4 + 8 * K, "K sqrt")]
for Rr in (16, 32, 64, 128):
    cost.append((f"H sh{Rr}", Rr * d + K * Rr + 2 * K, Rr * d * 2 + K * Rr * 2 + 8 * K, "K sqrt"))
cost += [("H sh32+own2", 32 * d + K * 32 + K * 2 * d, 32 * d * 2 + K * 32 * 2 + K * 2 * d * 2 + 8 * K, "K sqrt"),
         ("H sh64+own4", 64 * d + K * 64 + K * 4 * d, 64 * d * 2 + K * 64 * 2 + K * 4 * d * 2 + 8 * K, "K sqrt"),
         ("H group (every group)", Gn * d * d + K * d, Gn * d * d + K * d * 2, "K sqrt"),
         ("rad-model + spread", 40 * K, 64 * 4, "+ about 40 K exp / table lookups for tau (bisection), K logs")]
for lab, m, b, sf in cost:
    p(f"  {lab:26s} {m / 1e3:>12.1f}k {b / 1024:>10.1f}KB   {sf}")

p("\nstopping: recall at equal mean rows scanned, global threshold on the calibrated per-row probability / fixed nprobe "
  "(calibration: E[true neighbours | (s_max - s_c)/sd(q)] by isotonic regression on pseudo-queries, a table of <= 800 values)")
ST = [("H own r16 a0.75 w0.3", "H r16"), ("H own r4 a0.5 w0.3", "H r4"), ("O2 f'=0.4", "f'=0.4")]
ST += [(glob[s], f"H {s} LOO") for s in ("sh32", "sh64", "sh64+own4", "group")]
ST += [("model own r16 beta", "rad-model own r16"), ("model sh64 beta", "rad-model sh64"), ("model sh32 beta", "rad-model sh32")]
for n in main + extra:
    st = R[n]["stopping"]
    p(f"  {n}")
    for k, lab in ST:
        if k in st:
            p(f"    {lab:22s} " + "  ".join(f"{float(s) * 100:g}%: {st[k][s]['global_']:.3f}/{st[k][s]['fixed']:.3f} ({st[k][s]['global_'] - st[k][s]['fixed']:+.3f})" for s in SH))
p("\nstopping gain (global - fixed), mean over the main sets")
for k, lab in ST:
    p(f"  {lab:22s} " + "  ".join(f"{float(s) * 100:g}%: {np.mean([R[n]['stopping'][k][s]['global_'] - R[n]['stopping'][k][s]['fixed'] for n in main]):+.3f}" for s in SH)
      + "   recall with stopping: " + "  ".join(f"{float(s) * 100:g}% min {min(R[n]['stopping'][k][s]['global_'] for n in main):.3f}" for s in ("0.005", "0.016")))
# recall targets (targets5.py), if present
ID, OOD = ["laion-image", "glove", "yahoo"], ["laion-text", "vibe-laion"]
EXT = ["glove-1000*", "yahoo-900*", "laion-text-learn*", "vibe-laion-learn*"]
T = {}
for n in main + extra:
    fp = os.path.join(rd, f"targets-{n}.json")
    if os.path.exists(fp):
        T[n] = json.load(open(fp))
if T:
    p("\nrecall targets: achieved mean recall / share of queries below the target / mean rows scanned (% of N); for the "
      "average target also the rows a fixed nprobe of the same order needs for the same recall. Groups: in-distribution "
      f"{ID}, out-of-distribution {OOD}, extra sets {EXT} (each averaged over its sets). Calibration on plain or mixed pseudo-queries.")
    keys = sorted(next(iter(T.values()))["results"].keys(), key=lambda k: (k.split(" | ")[1], k))
    for k in keys:
        for mode in ("per-query", "running", "average"):
            if f"{mode} 0.9" not in next(iter(T.values()))["results"][k]:
                continue
            for gname, gsets in (("ID", ID), ("OOD", OOD), ("extra", EXT)):
                gs = [x for x in gsets if x in T]
                if not gs:
                    continue
                cells = []
                for r in ("0.8", "0.9", "0.95", "0.99"):
                    v = [T[x]["results"][k][f"{mode} {r}"] for x in gs]
                    c = f"{r}: {np.mean([a['recall'] for a in v]):.3f}/{np.mean([a['below'] for a in v]):.2f}/{np.mean([a['share'] for a in v]) * 100:.1f}%"
                    if mode == "average":
                        c += f" (fixed {np.mean([a['fixed_share_same_recall'] for a in v]) * 100:.1f}%)"
                    cells.append(c)
                p(f"  {k:30s} {mode:9s} {gname:5s} " + "  ".join(cells))
txt = "\n".join(L)
print(txt)
open(outp, "w").write(txt + "\n")
