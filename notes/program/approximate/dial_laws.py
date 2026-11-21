"""DIAL.md, checks 1 and 2: the laws the two dials are quantiles of, and candidate formulas for the levels' dial.

From a `study_layered_dump` (first 30 clusters a query, every row's exact score and its interval at the two
deterministic levels) and the prep directory's true order. The threshold is the true k-th best (as the session's
replays). Per cluster, as the reader builds it (q.rs `KnnReader::sample`): 64 rows evenly over the cluster, tau_l the sd
of e = s - a_l, f_l = clamp(1 - tau_l^2 / sigma_c^2), m_l = mean(e) - tau_l (mean(s) - mu_c) / sigma_c, with mu_c and
sigma_c the library's (the .clu file), zeta = max((t - mu_c) / sigma_c, 0). A row passes level l at dial y when
min(certified upper end, a_l + m_l + (zeta + y sqrt(f_l)) tau_l) >= t; sampled rows are scored, so always pass.

usage: python3 -I dial_laws.py DUMP_PREFIX PREP_DIR [--pooled]
--pooled: the stop's delta and rho from the 64-row samples of the query's dumped clusters (else 0 and 1).
"""
import sys

import numpy as np

prefix, prep = sys.argv[1], sys.argv[2]
pooled = "--pooled" in sys.argv
KK, S = 10, 64
meta = dict(line.rstrip("\n").split(" ", 1) for line in open(prefix + ".meta"))
nq, M, L, ndl = (int(meta[x]) for x in ("nq", "m", "levels", "ndl"))
idx = np.fromfile(prefix + ".idx", np.uint32).reshape(nq, M, 2)
clu = np.fromfile(prefix + ".clu", np.float32).reshape(nq, -1, 4)
w = 5 + 2 * L
raw = np.memmap(prefix + ".rows", np.float32, mode="r").reshape(-1, w)
ent = np.memmap(prefix + ".rows", np.uint32, mode="r").reshape(-1, w)[:, 0]
off = np.concatenate([[0], np.cumsum(idx[:, :, 1].astype(np.int64).ravel())])
order = np.load(f"{prep}/order.npy", mmap_mode="r")
nlev = min(ndl, L)

PSIS = [4.0, 5.0, 6.0]
YG = np.arange(-2.0, 8.01, 0.25)  # sweep of a rule's free offset or dial
ZB = np.arange(-2.0, 9.01, 0.5)  # height bins

zhist = np.zeros(len(ZB) + 1)  # all rows' heights z = (s - mu) / sigma
nrows_all = 0
by_u = {}  # bin of zeta_c -> [clusters, neighbours, rows]
home_u = []  # per neighbour found in the dump: its cluster's u_c
nb = []  # per neighbour not sampled: (y*, y_0, y_1, zeta_c, x, f_0, u_c)
gb = [[] for _ in range(nlev)]  # bulk: the sampled rows' scatter about the ray, per level
found_tot = 0
dq = []
# rule families: per psi, per grid value: [kept neighbours, neighbours in read clusters, rows passing]
FAM = ["uniform", "margin1", "margin2", "marginL", "plain"]
res = {(p, fam): np.zeros((len(YG), 3)) for p in PSIS for fam in FAM}
stop = {p: [0, 0, 0] for p in PSIS}  # neighbours in read clusters, in unread dumped clusters, clusters read
rate_pairs = []

for qi in range(nq):
    top = np.array(order[qi, :KK])
    cls = []
    for j in range(M):
        c, nv = int(idx[qi, j, 0]), int(idx[qi, j, 1])
        if c == 0xFFFFFFFF or nv < 2 * S:
            continue
        lo, hi = off[qi * M + j], off[qi * M + j + 1]
        r = np.asarray(raw[lo:hi], np.float64)
        ex = r[:, 1]
        lv = r[:, 5:].reshape(-1, L, 2)[:, :nlev]
        hit = np.isin(np.asarray(ent[lo:hi]), top)
        mu, sg = float(clu[qi, c, 0]), float(clu[qi, c, 1])
        if sg <= 0:
            continue
        cls.append((ex, lv, hit, mu, sg))
    found = np.concatenate([c[0][c[2]] for c in cls]) if cls else np.array([])
    if len(found) == 0:
        continue
    found_tot += len(found)
    t = float(found.min())
    # the stop's moments
    if pooled:
        zs = np.concatenate([(c[0][np.arange(S) * (len(c[0]) - 1) // (S - 1)] - c[3]) / c[4] for c in cls])
        delta, rho = float(zs.mean()), float(zs.std())
    else:
        delta, rho = 0.0, 1.0
    dq.append((delta, rho))
    for ex, lv, hit, mu, sg in cls:
        n = len(ex)
        sel = np.arange(S) * (n - 1) // (S - 1)
        z = (ex - mu) / sg
        h, _ = np.histogram(z, np.concatenate([[-np.inf], ZB, [np.inf]]))
        zhist += h
        nrows_all += n
        zc = (t - mu) / sg
        uc = (zc - delta) / rho
        zeta = max(zc, 0.0)
        b = by_u.setdefault(float(np.floor(uc * 2) / 2), [0, 0, 0])
        b[0] += 1
        b[1] += int(hit.sum())
        b[2] += n
        home_u.extend([uc] * int(hit.sum()))
        ystar = np.full(n, -np.inf)
        pstar = np.full(n, -np.inf)
        yl = []
        ok_stats = True
        fs = []
        for l in range(nlev):
            a = (lv[:, l, 0] + lv[:, l, 1]) / 2
            e = ex[sel] - a[sel]
            em, tau = float(e.mean()), float(e.std())
            if tau <= 0:
                ok_stats = False
                break
            f = min(max(1.0 - tau * tau / (sg * sg), 0.0), 1.0)
            me = em - tau * (float(ex[sel].mean()) - mu) / sg
            fs.append(f)
            need = (t - a - me) / tau  # the width needed, in tau
            y = (need - zeta) / np.sqrt(f) if f > 0 else np.where(need <= zeta, -np.inf, np.inf)
            dead = lv[:, l, 1] < t  # the certified upper end already rules the row out
            y = np.where(dead, np.inf, y)
            yl.append(y)
            ystar = np.maximum(ystar, y)
            pstar = np.maximum(pstar, np.where(dead, np.inf, need))
            if f > 0:
                gb[l].extend((((ex[sel] - a[sel] - me) / tau - z[sel]) / np.sqrt(f)).tolist())
        if not ok_stats:
            continue
        samp = np.zeros(n, bool)
        samp[sel] = True
        ystar[samp] = -np.inf
        pstar[samp] = -np.inf
        for i in np.flatnonzero(hit & ~samp):
            nb.append((ystar[i], yl[0][i], yl[1][i] if nlev > 1 else yl[0][i], zc, z[i], fs[0], uc))
        ys, ps = np.sort(ystar), np.sort(pstar)
        yh, ph = np.sort(ystar[hit]), np.sort(pstar[hit])
        nh = int(hit.sum())
        for p in PSIS:
            if uc > p:
                stop[p][1] += nh
                continue
            stop[p][0] += nh
            stop[p][2] += 1
            mg = p - uc
            for fam in FAM:
                if fam == "plain":
                    thr, srt, sh = YG, ps, ph
                else:
                    k = {"uniform": 0.0, "margin1": 1.0, "margin2": 2.0, "marginL": 1.0}[fam]
                    thr = YG + k * mg if fam != "marginL" else YG + mg / np.sqrt(fs[0])
                    srt, sh = ys, yh
                x = res[(p, fam)]
                x[:, 0] += np.searchsorted(sh, thr, side="right")
                x[:, 1] += nh
                x[:, 2] += np.searchsorted(srt, thr, side="right")

name = prefix.split("/")[-1] + (" pooled" if pooled else "")
nqq = len(dq)
dq = np.array(dq)
print(f"## {name}: {nqq} queries, {found_tot / nqq:.2f} of the true {KK} a query in the dumped clusters; delta {dq[:, 0].mean():.2f} (sd {dq[:, 0].std():.2f}), rho {dq[:, 1].mean():.2f} (sd {dq[:, 1].std():.2f})")

# 1. the rows' heights: survival, and local rate
sv = (np.cumsum(zhist[::-1])[::-1] / nrows_all)[1:]  # P(z > ZB[i])
print("  rows' heights z, P(z > u): " + "  ".join(f"{u:g}: {sv[i]:.2e}" for i, u in enumerate(ZB) if u in (1, 2, 3, 4, 5, 6, 7)))
print("    rate -d ln P / du: " + "  ".join(f"{a:g}-{b:g}: {np.log(sv[list(ZB).index(a)] / max(sv[list(ZB).index(b)], 1e-12)) / (b - a):.2f}" for a, b in [(2, 3), (3, 4), (4, 5), (5, 6)]))
print(f"    rows a cluster {nrows_all / sum(v[0] for v in by_u.values()):.0f}; normal P(z > u): 3: 1.35e-03  4: 3.17e-05  5: 2.87e-07")
# 2. the stop's risk per cluster: neighbours a cluster by its threshold height
print("  neighbours a cluster by u_c (clusters; neighbours a cluster; rows a cluster times P(z > u_c + 0.25)):")
for u in sorted(by_u):
    v = by_u[u]
    if v[0] >= 20 and -1 <= u <= 7.5:
        i = int(np.searchsorted(ZB, u + 0.25))
        model = v[2] / v[0] * (sv[i - 1] * sv[i]) ** 0.5 if 0 < i < len(sv) else float("nan")
        print(f"    u {u:4.1f}: {v[0]:5d}  {v[1] / v[0]:8.4f}  {model:8.4f}")
        if v[1] > 0:
            rate_pairs.append((u + 0.25, np.log(v[1] / v[0])))
if len(rate_pairs) >= 3:
    rp = np.array([x for x in rate_pairs if x[0] >= 2.5])
    if len(rp) >= 3:
        sl, ic = np.polyfit(rp[:, 0], rp[:, 1], 1)
        print(f"    ln(neighbours a cluster) on u, u >= 2.5: slope {sl:.2f}, one neighbour a cluster at u = {-ic / sl:.2f}")
hu = np.array(home_u)
print("  neighbours' home height u_c: quantiles 10/50/90/99 " + " / ".join(f"{v:.2f}" for v in np.quantile(hu, [0.1, 0.5, 0.9, 0.99]))
      + "; share above psi (of those in the dump) " + "  ".join(f"{p:g}: {np.mean(hu > p):.3f}" for p in [3, 4, 5, 6, 7]))
# 3. the scatter: the dial a neighbour needs
nb = np.array(nb)
fin = np.isfinite(nb[:, 0])
print(f"  neighbours outside the sample: {len(nb)}; ruled out by a certified end {np.sum(~fin)}")
ys = nb[:, 0]
print("  miss at a uniform dial y, P(y* > y): " + "  ".join(f"{y:g}: {np.mean(ys > y):.4f}" for y in [0, 1, 2, 3, 4, 5, 6]))
print("    level 0 alone: " + "  ".join(f"{y:g}: {np.mean(nb[:, 1] > y):.4f}" for y in [0, 1, 2, 3, 4, 5, 6]))
print("    level 1 alone: " + "  ".join(f"{y:g}: {np.mean(nb[:, 2] > y):.4f}" for y in [0, 1, 2, 3, 4, 5, 6]))
print("    either over the larger of the two (the allowance for two levels): " + "  ".join(
    f"{y:g}: {np.mean(ys > y) / max(np.mean(nb[:, 1] > y), np.mean(nb[:, 2] > y), 1e-9):.2f}" for y in [1, 2, 3, 4]))
print(f"    y* quantiles 50/90/97.7/99/99.9: " + " / ".join(f"{v:.2f}" for v in np.quantile(ys, [0.5, 0.9, 0.977, 0.99, 0.999])) + f"; mean {ys.mean():.2f} sd {ys.std():.2f}")
print("    by the cluster's zeta, mean y* / sd / miss at 2: " + "  ".join(
    f"{a:g}-{b:g}: {ys[(nb[:, 3] >= a) & (nb[:, 3] < b)].mean():.2f} / {ys[(nb[:, 3] >= a) & (nb[:, 3] < b)].std():.2f} / {np.mean(ys[(nb[:, 3] >= a) & (nb[:, 3] < b)] > 2):.3f} (n {np.sum((nb[:, 3] >= a) & (nb[:, 3] < b))})"
    for a, b in [(-9, 2), (2, 3), (3, 4), (4, 5), (5, 99)] if np.sum((nb[:, 3] >= a) & (nb[:, 3] < b)) >= 20))
# the slack: neighbours' own scatter at the threshold's height, g_i = y0 + (x - zeta) beta
for l in range(nlev):
    g = np.array(gb[l])
    print(f"  level {l} bulk scatter about the ray (sampled rows, {len(g)}): mean {g.mean():.2f} sd {g.std():.2f}; P(g > y): " + "  ".join(f"{y:g}: {np.mean(g > y):.4f}" for y in [1, 2, 3, 4, 5]))
# 4. the candidate rules
print("  the stop on the dumped clusters (true t, every cluster priced): psi: neighbours read / unread, clusters read a query")
for p in PSIS:
    s = stop[p]
    print(f"    psi {p:g}: {s[0] / nqq:.2f} / {s[1] / nqq:.2f}, {s[2] / nqq:.1f}; stop's miss of the true {KK}: {1 - s[0] / (KK * nqq):.3f}")


def at(p, fam, y):
    x = res[(p, fam)][int(np.argmin(np.abs(YG - y)))]
    return x[0] / max(x[1], 1), x[2] / nqq


def rows_to(p, fam, target):
    x = res[(p, fam)]
    k, rw = x[:, 0] / np.maximum(x[:, 1], 1), x[:, 2] / nqq
    i = int(np.searchsorted(k, target))
    if i == 0 or i >= len(k):
        return None
    return rw[i - 1] + (rw[i] - rw[i - 1]) * (target - k[i - 1]) / max(k[i] - k[i - 1], 1e-12)


print("  kept share of the neighbours in read clusters / rows past both levels a query")
for p in PSIS:
    print(f"    stop psi {p:g}:")
    print("      uniform psi_l = y:        " + "  ".join(f"{y:g}: {at(p, 'uniform', y)[0]:.3f}/{at(p, 'uniform', y)[1]:.0f}" for y in [1, 2, 3, p]))
    print("      margin, psi_l = psi-u_c+y: " + "  ".join(f"{y:g}: {at(p, 'margin1', y)[0]:.3f}/{at(p, 'margin1', y)[1]:.0f}" for y in [-1, 0, 1, 2]))
    print("      margin2, 2(psi-u_c)+y:     " + "  ".join(f"{y:g}: {at(p, 'margin2', y)[0]:.3f}/{at(p, 'margin2', y)[1]:.0f}" for y in [-2, -1, 0, 1]))
    print("      marginL, (psi-u_c)/sqrt f+y: " + "  ".join(f"{y:g}: {at(p, 'marginL', y)[0]:.3f}/{at(p, 'marginL', y)[1]:.0f}" for y in [-1, 0, 1, 2]))
    print("      plain, W = y tau:          " + "  ".join(f"{y:g}: {at(p, 'plain', y)[0]:.3f}/{at(p, 'plain', y)[1]:.0f}" for y in [3, 4, 5, 6]))
    for tg in (0.95, 0.98, 0.99):
        print(f"      rows to keep {tg}: " + "  ".join(f"{fam} {'-' if rows_to(p, fam, tg) is None else format(rows_to(p, fam, tg), '.0f')}" for fam in FAM))
