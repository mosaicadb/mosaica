"""Checks C1, C4, C5, C6 and C7 of the theory program's REPLAY-SPEC-RHO.md (the level width of claim A4), from
`study_layered_dump`. The width at deterministic level l of a cluster is y = (m' zeta + psi0 sqrt(f)) tau (+ the
sample's mean error, `--shift`), from s rows sampled evenly over the cluster's visible rows, each scored exactly:
tau the sd of e = score - a_l, f = 1 - Var(e) / Var(score), zeta = (t - mean score) / sd score, t the true k-th best;
a row goes on when a_l + y >= t at both deterministic levels; sampled rows are scored, so kept and counted.
Rules: m' = 1; m' = the index's fitted slope (of e / tau on (s - mu) / sigma through the origin, over its true
neighbours, per level; in-sample); m' = corr(e, s) over the sample, per cluster; kappa = 1 - f (y = psi0 sqrt(f) tau +
(1 - f)(t - mu)); plain (y = psi0 tau).
--within top25|bottom25 keeps, per query, each dumped cluster's top or bottom quarter of rows by score as the visible
set (truth: the k best among those rows of the dumped clusters).
usage: python3 -I width_checks.py PREFIX PREP_DIR [--within top25|bottom25] [--shift] [--s 64]
"""
import argparse

import numpy as np

ap = argparse.ArgumentParser()
ap.add_argument("prefix")
ap.add_argument("prep")
ap.add_argument("--within", default="")
ap.add_argument("--shift", action="store_true")
ap.add_argument("--s", type=int, default=64)
ap.add_argument("--whole", action="store_true", help="zeta and f against the whole cluster's mu_c and sigma_c(q) (the order's)")
a = ap.parse_args()
kk = 10
meta = dict(line.rstrip("\n").split(" ", 1) for line in open(a.prefix + ".meta"))
nq, M, L, ndl = (int(meta[x]) for x in ("nq", "m", "levels", "ndl"))
setname = meta.get("set", "none")
idx = np.fromfile(a.prefix + ".idx", np.uint32).reshape(nq, M, 2)
clu = np.fromfile(a.prefix + ".clu", np.float32).reshape(nq, -1, 4)
w = 5 + 2 * L
raw = np.memmap(a.prefix + ".rows", np.float32, mode="r").reshape(-1, w)
ent = np.memmap(a.prefix + ".rows", np.uint32, mode="r").reshape(-1, w)[:, 0]
off = np.concatenate([[0], np.cumsum(idx[:, :, 1].astype(np.int64).ravel())])
order = np.load(f"{a.prep}/order.npy" if setname == "none" else f"{a.prep}/order_{setname}.npy", mmap_mode="r")
nlev = min(ndl, L)
SIZES = (16, 32, 64, 128, 0)

# the queries' clusters: (score, midpoints, truth flags)
Q = []
for qi in range(nq):
    cls = []
    for j in range(M):
        if idx[qi, j, 0] == 0xFFFFFFFF or idx[qi, j, 1] == 0:
            continue
        lo, hi = off[qi * M + j], off[qi * M + j + 1]
        r = np.asarray(raw[lo:hi], np.float64)
        lv = r[:, 5:].reshape(-1, L, 2)
        ex, mid, en = r[:, 1], (lv[:, :, 0] + lv[:, :, 1]) / 2, np.asarray(ent[lo:hi])
        if a.within:
            q4 = np.quantile(ex, 0.75 if a.within == "top25" else 0.25)
            keep = ex >= q4 if a.within == "top25" else ex <= q4
            ex, mid, en = ex[keep], mid[keep], en[keep]
        if len(ex) >= 2:
            cls.append([ex, mid, en, (float(clu[qi, idx[qi, j, 0], 0]), float(clu[qi, idx[qi, j, 0], 1]))])
    if not cls:
        continue
    if a.within:
        allx = np.concatenate([c[0] for c in cls])
        t = float(np.sort(allx)[-kk]) if len(allx) >= kk else float(allx.min())
        for c in cls:
            c.insert(3, c[0] >= t)
    else:
        top = set(order[qi, :kk].tolist())
        for c in cls:
            c.insert(3, np.isin(c[2], list(top)))
        found = np.concatenate([c[0][c[3]] for c in cls])
        if len(found) == 0:
            continue
        t = float(found.min())
    Q.append((t, cls))


def stats(ex, mid, s, t, whole=None):
    sel = np.arange(len(ex)) if s == 0 else np.unique(np.linspace(0, len(ex) - 1, min(s, len(ex))).astype(int))
    out = []
    for l in range(nlev):
        e = ex[sel] - mid[sel, l]
        tau, vs = float(np.std(e)), float(np.var(ex[sel]))
        f = min(max(1.0 - tau * tau / vs, 0.0), 1.0) if vs > 0 else 0.0
        zeta = (t - float(np.mean(ex[sel]))) / np.sqrt(vs) if vs > 0 else 0.0
        rho = float(np.corrcoef(e, ex[sel])[0, 1]) if len(sel) > 2 and tau > 0 and vs > 0 else 0.0
        if a.whole and whole is not None and whole[1] > 0:
            f = min(max(1.0 - tau * tau / (whole[1] ** 2), 0.0), 1.0)
            zeta = (t - whole[0]) / whole[1]
        out.append(dict(tau=tau, f=f, zeta=zeta, rho=rho, me=float(np.mean(e)), mu=float(np.mean(ex[sel])), sg=np.sqrt(vs)))
    return sel, out


# the index's fitted slope per level, from its true neighbours (64-row statistics)
xs = {l: [] for l in range(nlev)}
ys = {l: [] for l in range(nlev)}
for t, cls in Q:
    for ex, mid, en, hit, whole in cls:
        if not hit.any() or len(ex) < 20:
            continue
        _, st = stats(ex, mid, 64, t, whole)
        for l in range(nlev):
            if st[l]["tau"] > 0 and st[l]["sg"] > 0:
                xs[l].extend(((ex[hit] - st[l]["mu"]) / st[l]["sg"]).tolist())
                ys[l].extend(((ex[hit] - mid[hit, l]) / st[l]["tau"]).tolist())
slope = {l: float(np.sum(np.square(xs[l])) and np.dot(xs[l], ys[l]) / np.sum(np.square(xs[l]))) for l in range(nlev)}


def width(rule, p, st):
    tau, f, zeta = st["tau"], st["f"], st["zeta"]
    if rule == "plain":
        y = p * tau
    elif rule == "1-f":
        y = p * np.sqrt(f) * tau + (1 - f) * max(t_cur - st["mu"], 0.0)
    else:
        m = 1.0 if rule == "m=1" else slope[st["l"]] if rule == "m=fitted" else max(st["rho"], 0.0)
        y = (m * max(zeta, 0.0) + p * np.sqrt(f)) * tau
    return y + (st["me"] if a.shift else 0.0)


PSIS = [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4, 5, 6, 8]
RULES = ["m=1", "m=fitted", "m=rho_es", "1-f", "plain"]


def run(rules, s, by_height=False):
    global t_cur
    res = {(r, p): [0, 0, 0] for r in rules for p in PSIS}
    hb = {}
    for t, cls in Q:
        t_cur = t
        for ex, mid, en, hit, whole in cls:
            sel, st = stats(ex, mid, s, t, whole)
            for l in range(nlev):
                st[l]["l"] = l
            for r in rules:
                for p in PSIS:
                    ok = np.ones(len(ex), bool)
                    for l in range(nlev):
                        ok &= mid[:, l] + width(r, p, st[l]) >= t
                    ok[sel] = True
                    x = res[(r, p)]
                    x[0] += int((ok & hit).sum())
                    x[1] += int(hit.sum())
                    x[2] += int(ok.sum())
                    if by_height and r == "m=1" and p == 2 and hit.any():
                        z = st[0]["zeta"]
                        b = "<2" if z < 2 else "2-3" if z < 3 else "3-4" if z < 4 else ">4"
                        h = hb.setdefault(b, [0, 0])
                        h[0] += int((ok & hit).sum())
                        h[1] += int(hit.sum())
    return res, hb


def rows_to(res, r, target):
    pts = sorted(((res[(r, p)][0] / max(res[(r, p)][1], 1), res[(r, p)][2] / len(Q)) for p in PSIS), key=lambda x: x[1])
    prev = None
    for k, rw in pts:
        if k >= target:
            return rw if prev is None or k == prev[0] else prev[1] + (rw - prev[1]) * (target - prev[0]) / (k - prev[0])
        prev = (k, rw)
    return None


name = a.prefix.split("/")[-1] + (f" within {a.within}" if a.within else "") + (" shift" if a.shift else "") + (" whole" if a.whole else "")
fmt = lambda v: "-" if v is None else f"{v:.0f}"
print(f"## {name}: {len(Q)} queries; fitted slope by level " + ", ".join(f"{slope[l]:.2f}" for l in range(nlev)))
# C1: the scatter g = (e - m x tau) / (sqrt(f) tau) of the true neighbours, m the fitted slope
g = {l: [] for l in range(nlev)}
gx = {l: [] for l in range(nlev)}
for t, cls in Q:
    for ex, mid, en, hit, whole in cls:
        if not hit.any() or len(ex) < 20:
            continue
        _, st = stats(ex, mid, a.s, t, whole)
        for l in range(nlev):
            s_ = st[l]
            if s_["tau"] <= 0 or s_["f"] <= 0:
                continue
            x = (ex[hit] - s_["mu"]) / s_["sg"]
            g[l].extend(((ex[hit] - mid[hit, l] - slope[l] * x * s_["tau"]) / (np.sqrt(s_["f"]) * s_["tau"])).tolist())
            gx[l].extend(x.tolist())
for l in range(nlev):
    v, x = np.array(g[l]), np.array(gx[l])
    qs = np.quantile(v, [0.9, 0.977, 0.99, 0.999])
    byh = "  ".join(f"{lo}-{hi}: sd {np.std(v[(x >= lo) & (x < hi)]):.2f} p99 {np.quantile(v[(x >= lo) & (x < hi)], 0.99):.2f}"
                    for lo, hi in [(0.5, 2), (2, 3), (3, 4), (4, 99)] if np.sum((x >= lo) & (x < hi)) >= 30)
    print(f"  C1 level {l}: sd {np.std(v):.2f}, p90/97.7/99/99.9 {' / '.join(f'{u:.2f}' for u in qs)} (normal 1.28/2.00/2.33/3.09, Laplace 1.14/2.18/2.77/4.39); {byh}")
res, hb = run(RULES, a.s, by_height=True)
print("  C4 kept at psi0 2 (m' = 1) by the cluster's zeta: " + "  ".join(f"{b}: {h[0] / max(h[1], 1):.3f} (n {h[1]})" for b, h in sorted(hb.items())))
print(f"  C5 (s {a.s}): kept at psi0 2 / 3; rows to keep 0.9 / 0.95")
for r in RULES:
    k2 = res[(r, 2)][0] / max(res[(r, 2)][1], 1)
    k3 = res[(r, 3)][0] / max(res[(r, 3)][1], 1)
    print(f"    {r:<9} {k2:.3f} / {k3:.3f}; {fmt(rows_to(res, r, 0.9))} / {fmt(rows_to(res, r, 0.95))}")
if not a.within and setname == "none":
    print("  C6 (m' = 1): kept at psi0 2 / 3; rows to keep 0.9 / 0.95, by sample size")
    for s in SIZES:
        r6, _ = run(["m=1"], s)
        k2 = r6[("m=1", 2)][0] / max(r6[("m=1", 2)][1], 1)
        k3 = r6[("m=1", 3)][0] / max(r6[("m=1", 3)][1], 1)
        print(f"    {'all rows' if s == 0 else f'{s} rows':<9} {k2:.3f} / {k3:.3f}; {fmt(rows_to(r6, 'm=1', 0.9))} / {fmt(rows_to(r6, 'm=1', 0.95))}")
