"""Referee 3: two of DIAL.md's numbers computed a second way, without its scripts' code.
(a) Stop-only recall (every row of a read cluster scored) in dump order with a running k-th best and an 8-cluster
    window, at psi 4, 5, 6: DIAL.md's 'certified' column.
(b) The share of true neighbours whose home cluster has (t - mu_c) / sigma_c above 4, 5, 6 at the true threshold
    (DIAL.md section 2.2), with t as the scripts take it (the least score among the neighbours found in the dump) and
    with t the true k-th best where all ten are in the dump; small clusters included and excluded.
usage: python3 ref3_indep.py DUMP_PREFIX PREP_DIR
"""
import sys

import numpy as np

prefix, prep = sys.argv[1], sys.argv[2]
meta = dict(line.rstrip("\n").split(" ", 1) for line in open(prefix + ".meta"))
nq, M, L = int(meta["nq"]), int(meta["m"]), int(meta["levels"])
w = 5 + 2 * L
idx = np.fromfile(prefix + ".idx", np.uint32).reshape(nq, M, 2)
clu = np.fromfile(prefix + ".clu", np.float32).reshape(nq, -1, 4)
order = np.load(f"{prep}/order.npy", mmap_mode="r")
counts = idx[:, :, 1].astype(np.int64).ravel()
starts = np.concatenate([[0], np.cumsum(counts)])
fh = open(prefix + ".rows", "rb")
K = 10

rec = {4.0: 0, 5.0: 0, 6.0: 0}
home = {"t_found, >=128 rows": [], "t_found, all": [], "complete queries, all": []}
nqq = 0
for q in range(nq):
    truth = set(int(x) for x in order[q, :K])
    clusters = []
    for j in range(M):
        c, n = int(idx[q, j, 0]), int(idx[q, j, 1])
        if c == 0xFFFFFFFF or n == 0:
            continue
        fh.seek(int(starts[q * M + j]) * w * 4)
        blk = np.frombuffer(fh.read(n * w * 4), np.float32).reshape(n, w)
        ids = blk.view(np.uint32)[:, 0]
        score = blk[:, 1].astype(np.float64)
        isn = np.fromiter((int(i) in truth for i in ids), bool, n)
        clusters.append((float(clu[q, c, 0]), float(clu[q, c, 1]), n, score, isn))
    if not clusters:
        continue
    nqq += 1
    # (a)
    for psi in rec:
        best = []
        for j, (mu, sg, n, score, isn) in enumerate(clusters):
            if len(best) >= K:
                t = sorted(best, reverse=True)[K - 1][0]
                ahead = [(t - m) / s for m, s, _, _, _ in clusters[j:j + 8] if s > 0]
                if all(a > psi for a in ahead):
                    break
            best.extend(zip(score.tolist(), isn.tolist()))
            best = sorted(best, reverse=True)[:K]
        rec[psi] += sum(h for _, h in best)
    # (b)
    found = [(s, mu, sg, n) for mu, sg, n, score, isn in clusters for s in score[isn]]
    tf = min(s for s, _, _, _ in found)
    for s, mu, sg, n in found:
        if sg > 0:
            home["t_found, all"].append((tf - mu) / sg)
            if n >= 128:
                home["t_found, >=128 rows"].append((tf - mu) / sg)
    if len(found) == K:
        for s, mu, sg, n in found:
            if sg > 0:
                home["complete queries, all"].append((tf - mu) / sg)

print(f"## {prefix.split('/')[-1]}: {nqq} queries")
print("  (a) stop-only recall, whole clusters, running t: " + "  ".join(f"psi {p:g}: {rec[p] / (K * nqq):.3f}" for p in rec))
for k, v in home.items():
    v = np.array(v)
    print(f"  (b) {k} ({len(v)} neighbours): median {np.median(v):.2f}; share above 4 / 5 / 6: " + " / ".join(f"{np.mean(v > p):.3f}" for p in (4, 5, 6)))
