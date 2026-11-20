"""Tables from study_order_stress's output. Per block (dataset, condition, visible set, k): each order's rows to each
recall as a ratio to the best order at that recall (clusters in brackets). Then per condition, each order's worst and
median ratio over its blocks and recalls; the best c of mu + c sigma per block against c_pred; mu's rows over
mu + 3 sigma's by k. python3 stress_table.py OUT_TABLES OUT_SUMMARY file [file ...]"""
import re
import statistics
import sys

TARGETS = ["0.9", "0.95", "0.99", "0.995", "0.999"]
blocks = []
for path in sys.argv[3:]:
    cur = None
    for l in open(path):
        m = re.match(r"== (?:\S*/stress-ix/)?(\S+) (\S+) (\S+) k=(\d+): queries (\S+) contributing (\S+) k_c (\S+) nbar (\S+) c_pred (\S+)", l)
        if m:
            cur = dict(ds=m.group(1), cond=m.group(2), filt=m.group(3), k=int(m.group(4)), mc=float(m.group(6)), kc=float(m.group(7)),
                       nbar=float(m.group(8)), cpred=float(m.group(9)), orders={}, errs={})
            blocks.append(cur)
            continue
        m = re.match(r"  error (.+?)\s+median (\S+) p90 (\S+) over (\d+)", l)
        if cur and m:
            cur["errs"][m.group(1).strip()] = (float(m.group(2)), float(m.group(3)), int(m.group(4)))
            continue
        m = re.match(r"  (\S.*?)\s+(0\.9: .*)$", l)
        if cur and m:
            vals = {t: (float(r), int(c)) for t, r, c in re.findall(r"([0-9.]+): (\d+) rows \((\d+) clusters\)", m.group(2))}
            cur["orders"][m.group(1).strip()] = vals

tab = []
for b in blocks:
    names = list(b["orders"])
    best = {t: min((b["orders"][o][t][0] for o in names if t in b["orders"][o]), default=None) for t in TARGETS}
    b["ratio"] = {o: {t: b["orders"][o][t][0] / best[t] for t in TARGETS if t in b["orders"][o] and best[t]} for o in names}
    tab.append(f"== {b['ds']} {b['cond']} {b['filt']} k={b['k']}: contributing clusters {b['mc']:.1f}, k_c {b['kc']:.2f}, "
               f"visible rows per contributing cluster {b['nbar']:.0f}, c_pred = Phi^-1(1 - k_c / nbar) {b['cpred']:.2f}")
    for e, (md, p9, cnt) in b["errs"].items():
        tab.append(f"   relative error {e:<22} median {md:.3f}  p90 {p9:.3f}  ({cnt} pairs)")
    tab.append("   " + f"{'order':<34}" + "".join(f"{t:>16}" for t in TARGETS))
    for o in names:
        cells = []
        for t in TARGETS:
            if t in b["ratio"][o]:
                cells.append(f"{b['ratio'][o][t]:.2f} ({b['orders'][o][t][1]})")
            else:
                cells.append("-")
        tab.append("   " + f"{o:<34}" + "".join(f"{c:>16}" for c in cells))
open(sys.argv[1], "w").write("\n".join(tab) + "\n")

out = []
conds = []
for b in blocks:
    if b["cond"] not in conds:
        conds.append(b["cond"])
for cond in conds:
    bs = [b for b in blocks if b["cond"] == cond]
    out.append(f"== {cond}: rows to recall / the best order's, worst and median over {len(bs)} blocks x recalls 0.9-0.999 "
               f"(and the worst block)")
    names = list(bs[0]["orders"])
    for o in names:
        rs = [(r, b, t) for b in bs for t, r in b["ratio"].get(o, {}).items()]
        if not rs:
            continue
        worst = max(rs, key=lambda x: x[0])
        out.append(f"   {o:<34} worst {worst[0]:.2f} median {statistics.median(x[0] for x in rs):.2f}   "
                   f"(worst: {worst[1]['ds']} {worst[1]['filt']} k={worst[1]['k']} at {worst[2]})")
    # the best c per block (least rows summed over the recalls each mu + c sigma reaches), and c_pred
    if not cond.startswith("sketch"):
        out.append("   best c of mu + c sigma per block (rows summed over recalls 0.9-0.995), against c_pred:")
        for b in bs:
            cs = [o for o in b["orders"] if re.fullmatch(r"mu\+[0-9.]+sigma", o)]
            tot = {o: sum(b["orders"][o][t][0] for t in TARGETS[:4] if t in b["orders"][o]) for o in cs}
            bc = min(tot, key=tot.get)
            m3 = sum(b["orders"]["mu+3sigma"][t][0] for t in TARGETS[:4] if t in b["orders"]["mu+3sigma"])
            mu = sum(b["orders"]["mu"][t][0] for t in TARGETS[:4] if t in b["orders"]["mu"])
            out.append(f"     {b['ds']:<14} {b['filt']:<18} k={b['k']:<5} best {bc[3:-5]:<4} c_pred {b['cpred']:.2f}   "
                       f"mu / mu+3sigma rows {mu / m3:.2f}   mu+3sigma / best {m3 / tot[bc]:.2f}")
open(sys.argv[2], "w").write("\n".join(out) + "\n")
print("\n".join(out))
