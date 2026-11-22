# table.py OUT...: study_w_principled's summary lines compacted: per (set, rule, psi) recall, clusters read / direct /
# opened, groups, stored vectors, rows entering level 1, rows past the deterministic levels, and the misses unread /
# dropped at the deterministic levels / dropped at the randomised levels (over all queries).
import re
import sys

for out in sys.argv[1:]:
    print(f"== {out}")
    print(f"{'set':<10} {'rule':<14} {'psi':>4} {'recall':>6} {'read':>5} {'dir':>5} {'open':>5} {'grp':>5} {'stored':>6} {'L1in':>6} {'past':>6} | unread det rand")
    for line in open(out):
        if line.startswith("#"):
            continue
        m = re.match(r"(\S+)\s+(\S+)\s+psi=(\S+): recall (\S+) visited (\S+) direct (\S+) index \S+ opened (\S+) groups (\S+) .* stored (\S+) .* past_det (\S+) enter (\S+) \| misses \(all queries\): unread (\S+) direct \S+ dropped by level (\S+)", line)
        s, r, psi, rec, vis, dr, op, gr, sto, past, enter, unr, lv = m.groups()
        enter = enter.split("/")
        lv = [int(x) for x in lv.split("/")]
        ndl = 2
        print(f"{s:<10} {r:<14} {float(psi):>4g} {float(rec):>6.3f} {float(vis):>5.1f} {float(dr):>5.1f} {float(op):>5.1f} {float(gr):>5.2f} {int(sto):>6} {int(enter[1]):>6} {int(past):>6} | {int(unr):>4} {sum(lv[:ndl]):>3} {sum(lv[ndl:]):>3}")
