# Appends the calibration summary and the cost tables to the hand-written text of policy_replay.md.
# usage: python3 -I assemble_policy.py HEAD.md CTABLES.md PTABLES.md OUT.md
import sys, re

head, ct, pt, out = sys.argv[1:5]
P = open(pt).read()
i = P.index("## Modelled cost relative to T1")
full = {m.group(1): m.group(2) for m in re.finditer(r"### (.*?)\n(.*?)(?=\n### |\n## |\Z)", P[:i], re.S)}
o = [open(head).read(),
     "### C1. Calibration checks (policy.md 4.4)\n\ndelta: the largest |ln(observed/predicted)| over bins of predicted P holding "
     "95% of the answers. kappa: per query, the sum\nof n P(tau_true) over the units, divided by the answer count.\n",
     open(ct).read(), "\n### P1. " + P[i + 3:], "\n### P2. Unfiltered k-NN, every arm (modelled us, rows, clusters, groups, centres)\n"]
for ds in ("laion", "glove", "yahoo"):
    o.append(f"**{ds}**\n" + full[f"{ds}, knn"])
open(out, "w").write("\n".join(o))
