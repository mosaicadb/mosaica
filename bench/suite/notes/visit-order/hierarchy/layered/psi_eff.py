"""The statistical width with a sampled spread, plain against selection-corrected, from `study_layered_dump`.

Per (query, cluster) over the first M clusters of the k-NN order, s rows spread evenly over the cluster are scored
exactly. At each deterministic level l, from those rows: tau the sd of e = score - a_l, mean and sd of the scores,
f = 1 - Var_s(e) / Var_s(score) (the share of the cluster's score variance the level explains), and the threshold's
height zeta = (t - mean_s) / sd_s. A row goes on from level l when a_l + w >= t, with
- plain: w = psi tau;
- corrected: w = (psi sqrt(f) + zeta sqrt(1 - f)) tau, so that a row scoring exactly t is kept with probability Phi(psi)
  under a joint Gaussian model whatever f (layered.md 4.3);
t the true k-th best score (the replay's simplification: the reader uses the running one). Sampled rows are scored, so
always kept and counted. Printed per psi: the share of the true k kept and the rows reaching the stored vectors per
query; then, per rule, the rows to keep 0.8 / 0.9 / 0.95 of the true k (interpolated along psi).
usage: python3 -I psi_eff.py PREFIX PREP_DIR [s, default 64]
"""
import sys

import numpy as np

prefix, prep = sys.argv[1], sys.argv[2]
S = int(sys.argv[3]) if len(sys.argv) > 3 else 64
kk = 10
meta = {}
for line in open(prefix + ".meta"):
    k, v = line.split(" ", 1)
    meta[k] = v.strip()
nq, K, M, L, ndl = (int(meta[x]) for x in ("nq", "k", "m", "levels", "ndl"))
idx = np.fromfile(prefix + ".idx", np.uint32).reshape(nq, M, 2)
w = 5 + 2 * L
raw = np.memmap(prefix + ".rows", np.float32, mode="r").reshape(-1, w)
ent = np.memmap(prefix + ".rows", np.uint32, mode="r").reshape(-1, w)[:, 0]
off = np.concatenate([[0], np.cumsum(idx[:, :, 1].astype(np.int64).ravel())])
order = np.load(f"{prep}/order.npy", mmap_mode="r")
nlev = min(ndl, L)
psis = {"plain": [1, 1.5, 2, 2.5, 3, 3.5, 4, 4.5, 5, 6, 7, 8], "corrected": [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4, 5]}
res = {(r, p): [0, 0, 0] for r in psis for p in psis[r]}
fs = []
for qi in range(nq):
    top = set(order[qi, :kk].tolist())
    cls = []
    for j in range(M):
        if idx[qi, j, 0] == 0xFFFFFFFF or idx[qi, j, 1] == 0:
            continue
        a, b = off[qi * M + j], off[qi * M + j + 1]
        r = np.asarray(raw[a:b], np.float64)
        lv = r[:, 5:].reshape(-1, L, 2)
        cls.append((r[:, 1], (lv[:, :, 0] + lv[:, :, 1]) / 2, np.isin(np.asarray(ent[a:b]), list(top))))
    found = np.concatenate([c[0][c[2]] for c in cls]) if cls else np.array([])
    if len(found) == 0:
        continue
    t = float(found.min())
    for ex, mid, hit in cls:
        sel = np.unique(np.linspace(0, len(ex) - 1, min(S, len(ex))).astype(int))
        st = []
        for l in range(nlev):
            e = ex[sel] - mid[sel, l]
            tau = float(np.std(e)) if len(sel) > 1 else 0.0
            vs = float(np.var(ex[sel])) if len(sel) > 1 else 0.0
            f = min(max(1.0 - tau * tau / vs, 0.0), 1.0) if vs > 0 else 0.0
            zeta = (t - float(np.mean(ex[sel]))) / np.sqrt(vs) if vs > 0 else 0.0
            st.append((tau, f, zeta))
            if len(ex) >= 50:
                fs.append(f)
        for rule, plist in psis.items():
            for p in plist:
                ok = np.ones(len(ex), bool)
                for l, (tau, f, zeta) in enumerate(st):
                    m = p if rule == "plain" else p * np.sqrt(f) + max(zeta, 0.0) * np.sqrt(1.0 - f)
                    ok &= mid[:, l] + m * tau >= t
                ok[sel] = True
                x = res[(rule, p)]
                x[0] += int((ok & hit).sum())
                x[1] += int(hit.sum())
                x[2] += int(ok.sum())

name = prefix.split("/")[-1]
print(f"## {name}: s {S}; f (share the 16 B level explains, all clusters of 50+ rows) p10/50/90 "
      f"{' / '.join(f'{x:.2f}' for x in np.quantile(fs[0::nlev], [0.1, 0.5, 0.9]))}")
for rule in psis:
    pts = [(res[(rule, p)][0] / max(res[(rule, p)][1], 1), res[(rule, p)][2] / nq, p) for p in psis[rule]]
    print(f"  {rule:<9} " + "  ".join(f"psi {p:g}: {k:.3f}/{r:.0f}" for k, r, p in pts))
    cells = []
    for target in (0.8, 0.9, 0.95):
        prev = None
        val = "-"
        for k, r, p in sorted(pts, key=lambda x: x[1]):
            if k >= target:
                val = f"{r:.0f}" if prev is None or k == prev[0] else f"{prev[1] + (r - prev[1]) * (target - prev[0]) / (k - prev[0]):.0f}"
                break
            prev = (k, r)
        cells.append(val)
    print(f"  {rule:<9} rows to keep 0.8 / 0.9 / 0.95: {' / '.join(cells)}")
