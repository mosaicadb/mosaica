"""The frozen self-correcting estimate (moments only before k rows are held) against SC-probe, from
study_order_stress's frozen condition: per block, SC-frozen's rows over SC-probe's at recall 0.9 to 0.999, and both
over the best order's; with the final estimates. python3 frozen_cmp.py OUT file [file ...]"""
import re
import statistics
import sys

T = ["0.9", "0.95", "0.99", "0.995", "0.999"]
out, allr = [], []
for path in sys.argv[2:]:
    cur = None
    blocks = []
    for l in open(path):
        m = re.match(r"== (?:\S*/stress-ix/)?(\S+) (\S+) (\S+) k=(\d+)", l)
        if m:
            cur = dict(name=f"{m.group(1)} {m.group(3)} k={m.group(4)}", rows={}, est={})
            blocks.append(cur)
            continue
        if cur is None:
            continue
        m = re.match(r"  estimate (.+?)\s+median (\S+)", l)
        if m:
            cur["est"][m.group(1).strip()] = float(m.group(2))
            continue
        m = re.match(r"  (\S.*?)\s+(0\.9: .*)$", l)
        if m:
            cur["rows"][m.group(1).strip()] = {t: float(r) for t, r in re.findall(r"([0-9.]+): (\d+) rows", m.group(2))}
    for b in blocks:
        rw = b["rows"]
        fz, pb, m3 = rw["(SC-frozen)"], rw["(SC-probe)"], rw["mu+3sigma"]
        best = {t: min(v[t] for v in rw.values() if t in v) for t in T}
        rat = [fz[t] / pb[t] for t in T if t in fz and t in pb]
        allr += rat
        e = b["est"]
        out.append(f"{b['name']:<28} frozen/probe " + "/".join(f"{x:.2f}" for x in rat)
                   + "   frozen/best " + "/".join(f"{fz[t] / best[t]:.2f}" for t in T if t in fz)
                   + "   probe/best " + "/".join(f"{pb[t] / best[t]:.2f}" for t in T if t in pb)
                   + "   mu+3sigma/best " + "/".join(f"{m3[t] / best[t]:.2f}" for t in T if t in m3)
                   + f"   delta/rho probe {e.get('delta (SC-probe)', 0):.2f}/{e.get('rho (SC-probe)', 0):.2f} frozen {e.get('delta (SC-frozen)', 0):.2f}/{e.get('rho (SC-frozen)', 0):.2f}")
out.append(f"frozen / probe over every cell: worst {max(allr):.3f} best {min(allr):.3f} median {statistics.median(allr):.3f}")
open(sys.argv[1], "w").write("\n".join(out) + "\n")
print("\n".join(out))
