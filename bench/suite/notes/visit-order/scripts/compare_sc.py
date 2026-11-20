"""The self-correcting orders of study_order_stress against mu + 3 sigma, the gap order (B) and mu alone. Per
condition: each order's worst and median rows ratio to the best order, and its share of cells within 1.10; each
self-correcting order's rows over mu + 3 sigma's where that order is within 1.10 of the best (worst, median), and its
ratio to the best under the in-cluster bottom sets; and the estimates (delta, rho, delta_e) per visible set.
python3 compare_sc.py OUT file [file ...]"""
import re
import statistics
import sys

T = ["0.9", "0.95", "0.99", "0.995", "0.999"]
FOCUS = ["mu+3sigma", "(B) gap running", "mu", "(SC) self-correcting", "(SC-probe)", "(E2) exceedance"]
SC = ["(SC) self-correcting", "(SC-probe)", "(E2) exceedance"]
blocks = []
for path in sys.argv[2:]:
    cur = None
    for l in open(path):
        m = re.match(r"== (?:\S*/stress-ix/)?(\S+) (\S+) (\S+) k=(\d+)", l)
        if m:
            cur = None if m.group(2).startswith("sketch") else dict(ds=m.group(1), cond=m.group(2), filt=m.group(3), k=int(m.group(4)), rows={}, est={})
            if cur:
                blocks.append(cur)
            continue
        if cur is None:
            continue
        m = re.match(r"  estimate (.+?)\s+median (\S+) p10 (\S+) p90 (\S+)", l)
        if m:
            cur["est"][m.group(1).strip()] = (float(m.group(2)), float(m.group(3)), float(m.group(4)))
            continue
        m = re.match(r"  (\S.*?)\s+(0\.9: .*)$", l)
        if m:
            cur["rows"][m.group(1).strip()] = {t: float(r) for t, r in re.findall(r"([0-9.]+): (\d+) rows", m.group(2))}


def ratio(b, o, t):
    vals = [b["rows"][x][t] for x in b["rows"] if t in b["rows"][x]]
    return b["rows"][o][t] / min(vals) if t in b["rows"].get(o, {}) else None


out = []
groups = []
for b in blocks:
    g = b["cond"] if b["cond"] != "base" else ("synthetic" if b["ds"].startswith("synth") else "held-out")
    b["group"] = g
    if g not in groups:
        groups.append(g)
for g in groups:
    bs = [b for b in blocks if b["group"] == g]
    out.append(f"== {g} ({len(bs)} blocks x recalls 0.9-0.999): rows / the best order's, worst / median / share <= 1.10")
    for o in FOCUS:
        rs = [x for b in bs for t in T if (x := ratio(b, o, t)) is not None]
        if rs:
            out.append(f"   {o:<24} {max(rs):6.2f} {statistics.median(rs):6.2f} {sum(r <= 1.10 for r in rs) / len(rs):6.2f}")
    for o in SC:
        rel = [b["rows"][o][t] / b["rows"]["mu+3sigma"][t] for b in bs for t in T
               if t in b["rows"].get(o, {}) and t in b["rows"]["mu+3sigma"] and (ratio(b, "mu+3sigma", t) or 9) <= 1.10]
        if rel:
            out.append(f"   {o:<24} rows / mu+3sigma's where mu+3sigma is within 1.10 of the best: worst {max(rel):.2f} median {statistics.median(rel):.2f} best {min(rel):.2f}")
    bot = [b for b in bs if b["filt"].startswith("bottom")]
    if bot:
        for o in FOCUS:
            rs = [x for b in bot for t in T if (x := ratio(b, o, t)) is not None]
            out.append(f"   bottom sets: {o:<24} worst {max(rs):.2f} median {statistics.median(rs):.2f}")
out.append("")
out.append("== the estimates at the end of each query (median, p10, p90 over the queries), per block")
for b in blocks:
    e = b["est"]
    if not e:
        continue
    cells = "  ".join(f"{k} {v[0]:.2f} [{v[1]:.2f}, {v[2]:.2f}]" for k, v in e.items())
    out.append(f"   {b['ds']:<16} {b['cond']:<14} {b['filt']:<18} k={b['k']:<5} {cells}")
open(sys.argv[1], "w").write("\n".join(out) + "\n")
print("\n".join(out[:200]))
