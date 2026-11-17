# The summary table of the replay: recall@10 at the scanned shares for each order (both fits) and faiss.
# usage: python -I table.py REPLAYDIR
import sys, os, json

R = sys.argv[1]
sh = ["0.005", "0.016", "0.03", "0.063", "0.125", "0.25"]
fa = json.load(open(os.path.join(R, "faiss_ref.json")))
lines = []
for fit, K in (("merged", "500"), ("old", "1000")):
    d = json.load(open(os.path.join(R, f"orders_{fit}.json")))
    ff, fl = d["fit_f"]["pseudo"], d["fit_f"]["learn"]
    lines.append(f"\n{fit} fit: K {d['K']}, fitted f (pseudo-queries) {ff}, fit_queries f (learn text) {fl}")
    lines.append(f"{'order':34s} " + " ".join(f"{float(s) * 100:>6g}%" for s in sh) + "   n16  r=0@1.6%  nprobe16/64/256")
    rows = [(f"int8 m + {f} rad" + (" (fitted)" if f == ff else "") + (" (fit_queries)" if f == fl else ""), d["orders"][f"int8 m + {f} rad"])
            for f in (0.0, 0.1, 0.2, 0.35, 0.5, 0.75)]
    rows += [(f"f32 m + {f} rad", d["orders"][f"f32 m + {f} rad"]) for f in (0.1, ff)]
    rows += [("m_hat (normalised mean)", d["orders"]["m_hat (normalised mean)"]), ("C (k-means centre)", d["orders"]["C (k-means centre)"])]
    rows += [(f"faiss IVF nlist {K} (= K)", fa[K]), ("faiss IVF nlist 4096", fa["4096"])]
    for name, o in rows:
        z = o["zero16"]
        lines.append(f"{name:34s} " + " ".join(f"{o['fixed'][s]:7.3f}" for s in sh) +
                     f"  {z['nprobe']:4d}  {z['n_zero']:3d}      " + "/".join(f"{v['recall']:.3f}" for v in o["nprobe"].values()))
    lines.append("per-query budget variant (each query probes whole clusters within the share):")
    for name, o in rows:
        lines.append(f"  {name:32s} " + " ".join(f"{o['budget'][s]:7.3f}" for s in sh))
out = "\n".join(lines)
print(out)
open(os.path.join(R, "summary_table.txt"), "w").write(out + "\n")
