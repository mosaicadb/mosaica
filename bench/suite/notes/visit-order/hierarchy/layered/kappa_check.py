"""Two checks of the second-population reading, from `study_layered_dump` (s rows sampled per cluster, as psi_eff.py).

(1) Among the true neighbours in the first M clusters, the share of the excess score their 16 B error carries:
    r = e / (score - mu_c), e = score - a_0, mu_c the cluster's mean score (all its dumped rows), against 1 - f, f the
    share of the cluster's score variance the level explains (from all its rows): quantiles of r, of r / (1 - f), and
    of e / tau in cluster rms spreads.
(2) The width psi0 sqrt(f) tau + kappa max(t - mu_c, 0) at both deterministic levels, tau, f and mu_c from the sample,
    t the true k-th best: per kappa, the share of the true k kept at psi0 = 2 and 3, and the rows to keep 0.9 of them
    (interpolated along psi0), samples included.
usage: python3 -I kappa_check.py PREFIX PREP_DIR [s, default 64]
"""
import sys

import numpy as np

prefix, prep = sys.argv[1], sys.argv[2]
S = int(sys.argv[3]) if len(sys.argv) > 3 else 64
kk = 10
meta = dict(line.split(" ", 1) for line in open(prefix + ".meta"))
nq, M, L, ndl = (int(meta[x]) for x in ("nq", "m", "levels", "ndl"))
idx = np.fromfile(prefix + ".idx", np.uint32).reshape(nq, M, 2)
w = 5 + 2 * L
raw = np.memmap(prefix + ".rows", np.float32, mode="r").reshape(-1, w)
ent = np.memmap(prefix + ".rows", np.uint32, mode="r").reshape(-1, w)[:, 0]
off = np.concatenate([[0], np.cumsum(idx[:, :, 1].astype(np.int64).ravel())])
order = np.load(f"{prep}/order.npy", mmap_mode="r")
nlev = min(ndl, L)
kappas = ["1-f", "sqrt(1-f)", 0.0, 0.05, 0.1, 0.15, 0.2, 0.3, 0.5]
psis = [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4, 5, 6]
res = {(kp, p): [0, 0, 0] for kp in kappas for p in psis}
r_all, rf_all, z_all = [], [], []
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
        if hit.any() and len(ex) >= 50:
            e = ex - mid[:, 0]
            f_all = 1.0 - np.var(e) / np.var(ex)
            mu = ex.mean()
            for i in np.where(hit)[0]:
                if ex[i] - mu > 0:
                    r_all.append(e[i] / (ex[i] - mu))
                    rf_all.append(r_all[-1] / max(1.0 - f_all, 1e-6))
                    z_all.append(e[i] / max(np.std(e), 1e-30))
        sel = np.unique(np.linspace(0, len(ex) - 1, min(S, len(ex))).astype(int))
        st = []
        for l in range(nlev):
            es = ex[sel] - mid[sel, l]
            tau = float(np.std(es)) if len(sel) > 1 else 0.0
            vs = float(np.var(ex[sel])) if len(sel) > 1 else 0.0
            f = min(max(1.0 - tau * tau / vs, 0.0), 1.0) if vs > 0 else 0.0
            st.append((tau, f, max(t - float(np.mean(ex[sel])), 0.0)))
        for kp in kappas:
            for p in psis:
                ok = np.ones(len(ex), bool)
                for l, (tau, f, gap) in enumerate(st):
                    k_ = (1.0 - f) if kp == "1-f" else np.sqrt(1.0 - f) if kp == "sqrt(1-f)" else kp
                    ok &= mid[:, l] + p * np.sqrt(f) * tau + k_ * gap >= t
                ok[sel] = True
                x = res[(kp, p)]
                x[0] += int((ok & hit).sum())
                x[1] += int(hit.sum())
                x[2] += int(ok.sum())

name = prefix.split("/")[-1]
q = lambda a: " / ".join(f"{x:.2f}" for x in np.quantile(a, [0.1, 0.25, 0.5, 0.75, 0.8, 0.9]))
print(f"## {name} (s {S}); true neighbours above their cluster's mean: {len(r_all)}")
print(f"  (1) e / (s - mu) at 16 B, p10/25/50/75/80/90: {q(r_all)}")
print(f"      the same over 1 - f:                     {q(rf_all)}")
print(f"      e / tau (cluster rms spreads):           {q(z_all)}; share beyond 4: {np.mean(np.array(z_all) > 4):.2f}")
print("  (2) kappa: kept at psi0 2 / 3, rows to keep 0.9")
for kp in kappas:
    pts = sorted(((res[(kp, p)][0] / max(res[(kp, p)][1], 1), res[(kp, p)][2] / nq) for p in psis), key=lambda x: x[1])
    val, prev = "-", None
    for k, r in pts:
        if k >= 0.9:
            val = f"{r:.0f}" if prev is None or k == prev[0] else f"{prev[1] + (r - prev[1]) * (0.9 - prev[0]) / (k - prev[0]):.0f}"
            break
        prev = (k, r)
    k2 = res[(kp, 2)][0] / max(res[(kp, 2)][1], 1)
    k3 = res[(kp, 3)][0] / max(res[(kp, 3)][1], 1)
    print(f"      {str(kp):<5} kept {k2:.3f} / {k3:.3f}  rows to 0.9: {val}")
