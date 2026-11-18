# Appends the curated tables (rows picked from tables.py's output) to the hand-written text of replay.md.
# usage: python3 -I assemble.py HEAD.md TABLES.md OUT.md
import sys, re

head, tables, out = sys.argv[1:4]
T = open(tables).read()
sec = {m.group(1): m.group(2) for m in re.finditer(r"### (.*?)\n(.*?)(?=\n### |\Z)", T, re.S)}


def rows(ds, start, keep, cols=None):
    body = next(v for k, v in sec.items() if k.startswith(f"{ds}: {start}"))
    o = []
    for ln in body.split("\n"):
        if ln.startswith("|") and (ln.startswith("|---") or re.search(keep, ln)):
            if cols:
                c = ln.split("|")
                ln = "|" + "|".join(c[i] for i in cols) + "|"
            o.append(ln)
    return "\n".join(o)


def ratio(ds):
    body = next(v for k, v in sec.items() if k.startswith(f"{ds}: sub-clusters under"))
    body = body[body.index("Ratio of items"):]
    return "\n".join(l for l in body.split("\n") if l.startswith("|") and
                     re.search(r"^\| variant|^\|---|x4 direct|x16 (direct|2L|GL)|x64 (direct|2L)", l))


UNDER = (r"structure|flat (spread|H r16)|x4 direct spread 0.4|x16 (direct (centre|nonin|spread 0.4|spread 0.8|Hown4 a0.75|gb16|gb64)"
         r"|two-level P(32|64|128) spread|two-level P64 Hown4|group-limited P(16|32|64))"
         r"|x64 (direct (nonin|spread 0.4)|two-level P(64|128) spread|group-limited P(16|32))")
BEAM = (r"^\| partition|real .*\| groups \| (c|st 1) \||refit5000 .*hkm 256 \| (c|st 1) \|"
        r"|refit20000 .*(hkm 256|16x16x16) \| (c|st 1) \|")
o = [open(head).read()]
o.append("### A. Sub-clusters, k-NN, no filter\n\nItems scanned at recall 0.9 / 0.95 / 0.99; groups and clusters touched; "
         "centres scored; estimated µs.\nMore rows (other f, H variants, P) in `replay/results/tables.md`.\n")
for ds in ("laion", "glove", "yahoo"):
    o.append(f"**{ds}**\n\n" + rows(ds, "sub-clusters, k-NN", UNDER, [1, 3, 4, 5, 6, 7, 8, 9]) + "\n")
o.append("### B. Sub-clusters under visible sets and for thresholds\n\nPer cell: the ratio of items at recall 0.9, at 0.95 "
         "(flat spread over the variant; `nan`: 0.95 not reached), and the\nchange in groups touched at 0.9. `2L` / `GL`: the "
         "two-level / group-limited run with the fewest parents within 5% of\nthe direct run's items at 0.9. `thr 0/1/2`: "
         "thresholds with answers of 0.01%, 1%, 10% of the corpus.\n")
for ds in ("laion", "glove", "yahoo"):
    o.append(f"**{ds}**\n\n" + ratio(ds) + "\n")
o.append("### C, D, E. Trees\n\nPer dataset: (flat) items / centres / groups at each recall for the flat orders on each "
         "partition; (C) beams at the\nflat order's nprobe for recall 0.9: recall; share of true neighbours lost per tree "
         "level; centres scored (share of\nK), for widths m1..m8 (nodes) and w2..w16 (clusters); (D) payoff: fewest items / "
         "groups within a budget of centres;\n(E) items / centres / groups at recall 0.9 under visible sets and for "
         "thresholds (beam rows: the groups as the tree\non the real fit, 256 nodes on the refits, statistical key).\n")
for ds in ("laion", "glove", "yahoo"):
    o.append(f"**{ds}, flat**\n\n" + rows(ds, "flat orders", r"^\|") + "\n")
    o.append(f"**{ds}, C**\n\n" + rows(ds, "beams", BEAM) + "\n")
    o.append(f"**{ds}, D**\n\n" + rows(ds, "payoff", r"^\|") + "\n")
    o.append(f"**{ds}, E**\n\n" + rows(ds, "visible sets and threshold", r"^\| partition|flat spread|st 1 w") + "\n")
open(out, "w").write("\n".join(o))
