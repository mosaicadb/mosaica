"""Per dataset and filter, each arm's recall at psi 4, 5 and 6, and at psi 5 the rows past the deterministic levels and
the stored vectors read per query (where the arm's bindings report them; "-" for d999606), from ab-knn-DS-ARM.txt.
usage: python3 -I counts.py DIR ARM1,ARM2,... DATASET1,DATASET2,...
"""
import re
import sys

d, arms, dss = sys.argv[1], sys.argv[2].split(","), sys.argv[3].split(",")
pat = re.compile(r"^(\S+)\s+adaptive\s+psi=(\S+):\s+([\d.]+)\s+(\d+) rows.*?(?:stored (\d+) past_det (\d+))?$")
for ds in dss:
    cells = {}
    for arm in arms:
        for line in open(f"{d}/ab-knn-{ds}-{arm}.txt"):
            m = pat.match(line.rstrip())
            if m:
                f, psi, rec, _, stored, past = m.groups()
                cells[(f, arm, float(psi))] = (float(rec), stored, past)
    filters = list(dict.fromkeys(k[0] for k in cells))
    print(f"== {ds}: recall at psi 4 / 5 / 6 | at psi 5: rows past the deterministic levels, stored vectors read")
    print(f"{'filter':<10} " + " ".join(f"{a:>32}" for a in arms))
    for f in filters:
        row = []
        for a in arms:
            r = [cells.get((f, a, p)) for p in (4.0, 5.0, 6.0)]
            rec = "/".join(f"{x[0]:.3f}" if x else "-" for x in r)
            c5 = r[1]
            cnt = f"{c5[2]} {c5[1]}" if c5 and c5[1] is not None else "-"
            row.append(f"{rec + ' | ' + cnt:>32}")
        print(f"{f:<10} " + " ".join(row))
    print()
