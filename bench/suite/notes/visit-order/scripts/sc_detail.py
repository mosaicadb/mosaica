"""Details for compare_sc: the cells where a self-correcting order needs over 1.05 times mu + 3 sigma's rows while mu +
3 sigma is within 1.10 of the best; and the final estimates pooled over benign visible sets (none, random, region,
the mix) against the in-cluster bottom and top sets. python3 sc_detail.py OUT file [file ...]"""
import re
import statistics
import sys

T = ["0.9", "0.95", "0.99", "0.995", "0.999"]
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
out = ["== cells where a self-correcting order needs over 1.05 times mu+3sigma's rows (mu+3sigma within 1.10 of the best)"]
for b in blocks:
    for t in T:
        vals = [b["rows"][x][t] for x in b["rows"] if t in b["rows"][x]]
        if t not in b["rows"].get("mu+3sigma", {}) or b["rows"]["mu+3sigma"][t] / min(vals) > 1.10:
            continue
        for o in SC:
            if t in b["rows"].get(o, {}):
                r = b["rows"][o][t] / b["rows"]["mu+3sigma"][t]
                if r > 1.05:
                    out.append(f"   {o:<22} {r:.2f}  {b['ds']} {b['cond']} {b['filt']} k={b['k']} at {t}")


def kind(b):
    f = b["filt"]
    if f.startswith("bottom"):
        return "bottom"
    if f.startswith("top"):
        return "top"
    if f.startswith("dir"):
        return "dir"
    return "benign"


out.append("")
out.append("== the final estimates' per-block medians, pooled: median [min, max] over blocks")
for kd in ["benign", "dir", "top", "bottom"]:
    bs = [b for b in blocks if kind(b) == kd and b["est"]]
    cells = []
    for e in ["delta (SC)", "rho (SC)", "delta (SC-probe)", "rho (SC-probe)", "delta_e (E2)"]:
        v = [b["est"][e][0] for b in bs]
        cells.append(f"{e} {statistics.median(v):.2f} [{min(v):.2f}, {max(v):.2f}]")
    out.append(f"   {kd:<7} ({len(bs)} blocks): " + "  ".join(cells))
for kd in ["bottom", "top"]:
    for b in blocks:
        if kind(b) == kd:
            e = b["est"]
            out.append(f"   {b['ds']:<6} {b['filt']:<10} delta {e['delta (SC)'][0]:.2f} rho {e['rho (SC)'][0]:.2f}  probe delta {e['delta (SC-probe)'][0]:.2f} rho {e['rho (SC-probe)'][0]:.2f}  delta_e {e['delta_e (E2)'][0]:.2f}")
open(sys.argv[1], "w").write("\n".join(out) + "\n")
print("\n".join(out))
