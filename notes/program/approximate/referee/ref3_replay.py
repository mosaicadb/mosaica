"""Referee 3: a re-run of DIAL.md's causal replay (dial_replay.py) with the rules' logic copied unchanged, rows read
as slices (no whole-file map), and these additions:
  - rules offX: psi_l = psi - X, a constant offset from the stop's dial;
  - rule fbm: the closing step with separate scales, psi_l = -sbar + (thy/thu)(psi - ubar) + thy ln(thu/(thy phi)),
    thu = 0.75, thy = 1.0 (the measured ranges' middles);
  - per rule: the stop's misses (true neighbours in clusters not read), the levels' drops, r = drops / stop misses,
    the mean phi-hat, and rows past the certified levels of clusters that are not sampled (read before k are held:
    all their rows; small clusters read after: rows whose certified upper ends reach t at both levels);
  - the cost proxy as DIAL.md has it, and the same proxy with the unsampled clusters' passed rows priced at c1.
usage: python3 ref3_replay.py DUMP_PREFIX PREP_DIR [psi,psi,...] [rule,rule,...]
"""
import os
import sys

import numpy as np

prefix, prep = sys.argv[1], sys.argv[2]
PSIS = [float(x) for x in sys.argv[3].split(",")] if len(sys.argv) > 3 else [4.0, 4.5, 5.0, 5.5, 6.0]
RULES = sys.argv[4].split(",") if len(sys.argv) > 4 else ["cert", "psi", "c2", "c3", "med+1", "fbf1", "fbf3", "off1.5", "off2", "off2.5", "fbm"]
KK, S, LOOK = 10, 64, 8
meta = dict(line.rstrip("\n").split(" ", 1) for line in open(prefix + ".meta"))
nq, M, L, ndl = (int(meta[x]) for x in ("nq", "m", "levels", "ndl"))
idx = np.fromfile(prefix + ".idx", np.uint32).reshape(nq, M, 2)
clu = np.fromfile(prefix + ".clu", np.float32).reshape(nq, -1, 4)
w = 5 + 2 * L
off = np.concatenate([[0], np.cumsum(idx[:, :, 1].astype(np.int64).ravel())])
order = np.load(f"{prep}/order.npy", mmap_mode="r")
nlev = min(ndl, L)
fh = open(prefix + ".rows", "rb")


def rows(lo, hi):
    fh.seek(int(lo) * w * 4)
    b = np.frombuffer(fh.read(int(hi - lo) * w * 4), np.float32).reshape(-1, w)
    return b, b.view(np.uint32)[:, 0]


# 0 hits, 1 clusters, 2 touched, 3 past, 4 samples, 5 dropped, 6 dial sum, 7 dial n, 8 dial sq, 9 stop-missed,
# 10 unsampled passed, 11 phi sum, 12 phi n
res = {(r, p): np.zeros(13) for r in RULES for p in PSIS}


def load(qi):
    top = np.array(order[qi, :KK])
    cls = []
    for j in range(M):
        c, nv = int(idx[qi, j, 0]), int(idx[qi, j, 1])
        if c == 0xFFFFFFFF or nv == 0:
            continue
        lo, hi = off[qi * M + j], off[qi * M + j + 1]
        rb, en = rows(lo, hi)
        r = np.asarray(rb, np.float64)
        lv = r[:, 5:].reshape(-1, L, 2)[:, :nlev]
        d = dict(ex=r[:, 1], a=(lv[:, :, 0] + lv[:, :, 1]) / 2, hi=lv[:, :, 1], hit=np.isin(en, top),
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


def slack_of(c, hs, hc, cls, t):
    f0 = c["st"][0][2] if c["st"] else 0.0
    if 0.0 < f0 < 1.0:
        beta = (1.0 - np.sqrt(1.0 - f0)) / np.sqrt(f0 * (1.0 - f0))
        sg = np.array([cls[k]["sg"] for k in hc])
        return beta * float(np.mean((hs - t) / np.where(sg > 0, sg, np.inf)))
    return 0.0


def run(cls, rule, psi, out):
    hs, hh, hc = np.empty(0), np.empty(0, bool), np.empty(0, int)

    def hold(s, h, c):
        nonlocal hs, hh, hc
        hs, hh, hc = np.concatenate([hs, s]), np.concatenate([hh, h]), np.concatenate([hc, c])
        if len(hs) > KK:
            keep = np.argsort(-hs)[:KK]
            hs, hh, hc = hs[keep], hh[keep], hc[keep]

    j, past, seen, nsamp, dropped = 0, 0, 0, 0, 0
    while j < len(cls):
        t = hs.min() if len(hs) >= KK else -np.inf
        if len(hs) >= KK and all((t - c["mu"]) / c["sg"] > psi for c in cls[j:j + LOOK] if c["sg"] > 0):
            break
        c = cls[j]
        n = c["n"]
        out[1] += 1
        out[2] += n
        if len(hs) < KK or n < 2 * S or c["sg"] <= 0:
            if len(hs) < KK:
                out[10] += n
            else:
                out[10] += int(np.all(c["hi"] >= t, axis=1).sum())
            hold(c["ex"], c["hit"], np.full(n, j))
            j += 1
            continue
        sel, samp = c["sel"], c["samp"]
        hold(c["ex"][sel], c["hit"][sel], np.full(S, j))
        out[4] += S
        t = hs.min()
        zeta = max((t - c["mu"]) / c["sg"], 0.0)
        ystar = np.full(n, -np.inf)
        for l, tau, f, am in c["st"]:
            need = (t - am) / tau
            y = (need - zeta) / np.sqrt(f) if f > 0 else np.where(need <= zeta, -np.inf, np.inf)
            y[c["hi"][:, l] < t] = np.inf
            ystar = np.maximum(ystar, y)
        phi = max(past, nsamp + S) / (seen + n)
        out[11] += phi
        out[12] += 1
        if rule == "cert":
            d = np.inf
        elif rule == "psi":
            d = psi
        elif rule.startswith("off"):
            d = psi - float(rule[3:])
        elif rule[0] == "c":
            d = float(rule[1:])
        else:
            u = np.array([(t - cls[k]["mu"]) / cls[k]["sg"] if cls[k]["sg"] > 0 else np.nan for k in hc])
            ubar = float(np.nanmedian(u))
            if rule.startswith("med"):
                d = psi - ubar + float(rule[3:])
            elif rule.startswith("fbf"):
                kap = float(rule[3:])
                d = psi - ubar + 0.7071 * np.log(1.0 / min(1.0, kap * phi)) - slack_of(c, hs, hc, cls, t)
            elif rule == "fbm":
                thu, thy = 0.75, 1.0
                d = (thy / thu) * (psi - ubar) + thy * np.log(thu / (thy * min(1.0, phi))) - slack_of(c, hs, hc, cls, t)
        d = max(d, 0.0) if np.isfinite(d) else d
        ok = (ystar <= d) & ~samp
        out[3] += int(ok.sum())
        past += int(ok.sum())
        seen += n
        nsamp += S
        dr = int((c["hit"] & ~samp & ~ok & (c["ex"] >= t)).sum())
        out[5] += dr
        dropped += dr
        if np.isfinite(d):
            out[6] += d
            out[7] += 1
            out[8] += d * d
        hold(c["ex"][ok], c["hit"][ok], np.full(int(ok.sum()), j))
        j += 1
    out[0] += int(hh.sum())
    # true neighbours in the dump's clusters not read, plus those outside the dump: the stop's misses
    out[9] += KK - int(hh.sum()) - dropped


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
print(f"## {name}: {nqq} queries; per rule: recall | rows touched | past (sampled clusters) | past (unsampled) | samples | dropped | stop-missed | r = dropped/stop-missed | mean phi | mean dial (sd)")
for p in PSIS:
    print(f"  psi {p:g}")
    for r in RULES:
        x = res[(r, p)]
        md = x[6] / x[7] if x[7] else float("nan")
        sd = np.sqrt(max(x[8] / x[7] - md * md, 0.0)) if x[7] else float("nan")
        rr = x[5] / x[9] if x[9] else float("nan")
        print(f"    {r:<7} {x[0] / (KK * nqq):.3f} | {x[2] / nqq:7.0f} | {x[3] / nqq:6.0f} | {x[10] / nqq:6.0f} | {x[4] / nqq:5.0f} | {x[5] / nqq:.3f} | {x[9] / nqq:.3f} | {rr:.3f} | {x[11] / max(x[12], 1):.4f} | {md:5.2f} ({sd:.2f})")


def proxy(r, c1, unsampled):
    pts = sorted((res[(r, p)][0] / (KK * nqq), (res[(r, p)][2] + c1 * (res[(r, p)][3] + (res[(r, p)][10] if unsampled else 0)) + 3.0 * res[(r, p)][4]) / nqq) for p in PSIS)
    vals = []
    for tg in (0.90, 0.95):
        v = None
        for (k0, t0), (k1, t1) in zip(pts, pts[1:]):
            if k0 <= tg <= k1 and k1 > k0:
                v = np.exp(np.log(t0) + (np.log(t1) - np.log(t0)) * (tg - k0) / (k1 - k0))
                break
        vals.append(v)
    return vals


print("  cost proxy at recall 0.90 / 0.95 over the rule 'psi': touched + c1 * past + 3 * samples [DIAL.md], then with the unsampled clusters' passed rows also priced at c1 [+u]")
for unsampled in (False, True):
    for c1 in (3.0, 6.0, 12.0):
        base = proxy("psi", c1, unsampled)
        cells = []
        for r in RULES:
            v = proxy(r, c1, unsampled)
            cells.append(f"{r} " + "/".join("-" if a is None or b is None else f"{a / b:.2f}" for a, b in zip(v, base)))
        print(f"    c1 {c1:g}{' +u' if unsampled else ''}: " + "  ".join(cells))
