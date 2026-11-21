"""DIAL.md, check 3: a causal replay of the stop and the sampled level width on a `study_layered_dump`, with the
levels' dial set by each candidate rule.

Per query the dumped clusters are read in dump order (the k-NN order). The running k-th best t is that of the rows
scored so far. Before k are held a cluster is scored whole. After, as q.rs `KnnReader::sample`: 64 rows evenly over the
cluster are scored, t is taken after them, zeta = max((t - mu_c) / sigma_c, 0), and at each deterministic level tau_l,
f_l, m_l are the sample's; a row passes when min(certified upper end, a_l + m_l + (zeta + psi_l sqrt(f_l)) tau_l) >= t
at both levels, and is then scored (the randomised levels are taken as exact). The stop: k held and every one of the
next 8 clusters has (t - mu_c) / sigma_c > psi (delta 0, rho 1: no cluster is read directly after the first).
Rules for psi_l: cert (certified levels), psi (the stop's psi), a constant, and
  med+c: psi - ubar + c, ubar the median over the k held rows of their home clusters' u_c at the current t;
  fbK, fbsK, fbTK: psi - ubar + theta ln(1 / r), r = min(1, K (rows past the levels so far + 1) / (rows of the sampled
      clusters so far, this one included)), theta = 0.7071 (1 for fbT); fbs also takes off the held rows' mean slack,
      beta mean((s_i - t) / sigma_home), beta = (1 - sqrt(1 - f)) / sqrt(f (1 - f)) at level 0 of the cluster in hand;
  fbfK: as fbsK with the rows past the levels floored at the rows sampled so far (this cluster's included);
  strK: psi - ubar + 0.7071 ln(n_c / (K 64)) - the mean slack: the levels' share set to K samples' worth of the cluster;
  eq: ybar + thy ((psi - ubar) / thu), with thu the mean of u_c - ubar over held rows above ubar, ybar and thy the
      median and the mean excess over it of the held rows' needed dials y* (rows scored whole or as samples have none
      and are left out); needs 4 held rows with a y*, else med+0.
Dials are floored at 0.
usage: python3 dial_replay.py DUMP_PREFIX PREP_DIR
"""
import sys

import numpy as np

prefix, prep = sys.argv[1], sys.argv[2]
KK, S, LOOK = 10, 64, 8
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
PSIS = [3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5]
RULES = ["cert", "psi", "c2", "c3", "med-1", "med+0", "med+1", "med+2", "eq", "fb4", "fb16", "fbs4", "fbs16", "fbT8", "fbf1", "fbf3", "str1", "str3"]
# per (rule, psi): recall hits, clusters read, rows touched, rows past both levels (not samples), samples,
# neighbours dropped by a level in a read cluster, sum of dials, dial count, sum of squared dials
res = {(r, p): np.zeros(11) for r in RULES for p in PSIS}


def load(qi):
    top = np.array(order[qi, :KK])
    cls = []
    for j in range(M):
        c, nv = int(idx[qi, j, 0]), int(idx[qi, j, 1])
        if c == 0xFFFFFFFF or nv == 0:
            continue
        lo, hi = off[qi * M + j], off[qi * M + j + 1]
        r = np.asarray(raw[lo:hi], np.float64)
        lv = r[:, 5:].reshape(-1, L, 2)[:, :nlev]
        d = dict(ex=r[:, 1], a=(lv[:, :, 0] + lv[:, :, 1]) / 2, hi=lv[:, :, 1], hit=np.isin(np.asarray(ent[lo:hi]), top),
                 mu=float(clu[qi, c, 0]), sg=float(clu[qi, c, 1]), n=nv, st=[])
        if nv >= 2 * S and d["sg"] > 0:
            sel = np.arange(S) * (nv - 1) // (S - 1)
            d["sel"] = sel
            samp = np.zeros(nv, bool)
            samp[sel] = True
            d["samp"] = samp
            for l in range(nlev):
                e = d["ex"][sel] - d["a"][sel, l]
                em, tau = float(e.mean()), float(e.std())
                if tau <= 0:
                    continue
                f = min(max(1.0 - tau * tau / (d["sg"] ** 2), 0.0), 1.0)
                me = em - tau * (float(d["ex"][sel].mean()) - d["mu"]) / d["sg"]
                d["st"].append((l, tau, f, d["a"][:, l] + me))
        cls.append(d)
    return cls


def run(cls, rule, psi, out):
    # the held rows: score, hit, home cluster, needed dial (nan when scored without the levels)
    hs, hh, hc, hy = np.empty(0), np.empty(0, bool), np.empty(0, int), np.empty(0)

    def hold(s, h, c, y):
        nonlocal hs, hh, hc, hy
        hs, hh, hc, hy = np.concatenate([hs, s]), np.concatenate([hh, h]), np.concatenate([hc, c]), np.concatenate([hy, y])
        if len(hs) > KK:
            keep = np.argsort(-hs)[:KK]
            hs, hh, hc, hy = hs[keep], hh[keep], hc[keep], hy[keep]

    j, past, seen, nsamp = 0, 0, 0, 0
    while j < len(cls):
        t = hs.min() if len(hs) >= KK else -np.inf
        if len(hs) >= KK and all((t - c["mu"]) / c["sg"] > psi for c in cls[j:j + LOOK] if c["sg"] > 0):
            break
        c = cls[j]
        n = c["n"]
        out[1] += 1
        out[2] += n
        if len(hs) < KK or n < 2 * S or c["sg"] <= 0:
            hold(c["ex"], c["hit"], np.full(n, j), np.full(n, np.nan))
            j += 1
            continue
        sel, samp = c["sel"], c["samp"]
        hold(c["ex"][sel], c["hit"][sel], np.full(S, j), np.full(S, np.nan))
        out[4] += S
        t = hs.min()
        zeta = max((t - c["mu"]) / c["sg"], 0.0)
        ystar = np.full(n, -np.inf)
        for l, tau, f, am in c["st"]:
            need = (t - am) / tau
            y = (need - zeta) / np.sqrt(f) if f > 0 else np.where(need <= zeta, -np.inf, np.inf)
            y[c["hi"][:, l] < t] = np.inf
            ystar = np.maximum(ystar, y)
        # the dial
        if rule == "cert":
            d = np.inf
        elif rule == "psi":
            d = psi
        elif rule[0] == "c":
            d = float(rule[1:])
        else:
            u = np.array([(t - cls[k]["mu"]) / cls[k]["sg"] if cls[k]["sg"] > 0 else np.nan for k in hc])
            ubar = float(np.nanmedian(u))
            d = psi - ubar + (float(rule[3:]) if rule.startswith("med") else 0.0)
            if rule.startswith("fbf") or rule.startswith("str"):
                # as fbs, the rows past the levels floored at the rows sampled (fbf), or the sample alone (str)
                kap = float(rule[3:])
                num = max(past, nsamp + S) if rule.startswith("fbf") else S
                den = (seen + n) if rule.startswith("fbf") else n
                d = psi - ubar + 0.7071 * np.log(1.0 / min(1.0, kap * num / den))
                f0 = c["st"][0][2] if c["st"] else 0.0
                if 0.0 < f0 < 1.0:
                    beta = (1.0 - np.sqrt(1.0 - f0)) / np.sqrt(f0 * (1.0 - f0))
                    sg = np.array([cls[k]["sg"] for k in hc])
                    slack = beta * float(np.mean((hs - t) / np.where(sg > 0, sg, np.inf)))
                    d -= slack
                    out[9] += slack
                    out[10] += 1
            elif rule.startswith("fb"):
                # the share of the work past the levels so far, kappa rows' worth each; theta the scatter's mean excess
                kap = float(rule.lstrip("fbsT"))
                th = 1.0 if "T" in rule else 0.7071
                rhat = min(1.0, kap * (past + 1.0) / (seen + n))
                d = psi - ubar + th * np.log(1.0 / rhat)
                if "s" in rule and c["st"]:
                    f0 = c["st"][0][2]
                    if 0.0 < f0 < 1.0:
                        beta = (1.0 - np.sqrt(1.0 - f0)) / np.sqrt(f0 * (1.0 - f0))
                        sg = np.array([cls[k]["sg"] for k in hc])
                        d -= beta * float(np.mean((hs - t) / np.where(sg > 0, sg, np.inf)))
            if rule == "eq":
                yy = hy[np.isfinite(hy)]
                up = u[u > ubar] - ubar
                if len(yy) >= 4 and len(up) >= 2 and up.mean() > 0:
                    ybar = float(np.median(yy))
                    thy = float(np.mean(yy[yy > ybar] - ybar)) if np.any(yy > ybar) else 0.0
                    d = ybar + thy * (psi - ubar) / float(up.mean())
        d = max(d, 0.0) if np.isfinite(d) else d
        ok = (ystar <= d) & ~samp
        out[3] += int(ok.sum())
        past += int(ok.sum())
        seen += n
        nsamp += S
        out[5] += int((c["hit"] & ~samp & ~ok & (c["ex"] >= t)).sum())
        if np.isfinite(d):
            out[6] += d
            out[7] += 1
            out[8] += d * d
        hold(c["ex"][ok], c["hit"][ok], np.full(int(ok.sum()), j), ystar[ok])
        j += 1
    out[0] += int(hh.sum())


nqq = 0
for qi in range(nq):
    cls = load(qi)
    if not cls:
        continue
    nqq += 1
    for r in RULES:
        for p in PSIS:
            run(cls, r, p, res[(r, p)])

name = prefix.split("/")[-1]
print(f"## {name}: {nqq} queries, causal replay over the dumped clusters ({M} a query)")
print("  per stop psi and rule: recall | clusters read | rows touched | rows past both levels (not samples) | neighbours dropped by a level, a query | mean dial (sd)")
for p in PSIS:
    print(f"  psi {p:g}")
    for r in RULES:
        x = res[(r, p)]
        md = x[6] / x[7] if x[7] else float("nan")
        sd = np.sqrt(max(x[8] / x[7] - md * md, 0.0)) if x[7] else float("nan")
        print(f"    {r:<6} {x[0] / (KK * nqq):.3f} | {x[1] / nqq:5.1f} | {x[2] / nqq:7.0f} | {x[3] / nqq:7.0f} | {x[5] / nqq:.3f} | {md:5.2f} ({sd:.2f})" + (f" | mean slack {x[9] / x[10]:.2f}" if x[10] else ""))
# cost at equal recall: a proxy c0 rows touched + c1 rows past the levels + cs samples, interpolated along psi
print("  cost proxy at equal recall, over the rule 'psi' (touched + c1 * past + 3 * samples; c1 = 3 and 6), recall 0.90 / 0.95:")
for c1 in (3.0, 6.0):
    line = []
    for r in RULES:
        pts = sorted((res[(r, p)][0] / (KK * nqq), (res[(r, p)][2] + c1 * res[(r, p)][3] + 3.0 * res[(r, p)][4]) / nqq) for p in PSIS)
        vals = []
        for tg in (0.90, 0.95):
            v = None
            for (k0, t0), (k1, t1) in zip(pts, pts[1:]):
                if k0 <= tg <= k1 and k1 > k0:
                    v = np.exp(np.log(t0) + (np.log(t1) - np.log(t0)) * (tg - k0) / (k1 - k0))
                    break
            vals.append(v)
        line.append((r, vals))
    base = dict(line)["psi"]
    print(f"    c1 {c1:g}: " + "  ".join(f"{r} " + "/".join("-" if v is None or b is None else f"{v / b:.2f}" for v, b in zip(vals, base)) for r, vals in line))
