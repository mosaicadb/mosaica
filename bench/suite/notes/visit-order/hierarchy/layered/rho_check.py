"""The spread of a level's error against what each rule for the statistical width predicts, from `study_layered_dump`.

Per (query, cluster) over the first M clusters of the k-NN order and each deterministic level l: the row's error
e = score - a_l (a_l the midpoint of its certified interval, h its half-width). Rules for the spread tau of e:
- derived: rho_g,l * h, rho from the build's weights (`GroupPlan2::rho`, layered.md 4.2);
- form 1: tau^2 = sigma_c(q)^2 - Var_rows(a_l), sigma_c(q) the library's spread from the cluster's 16 directions;
- form 1, exact: the same with the rows' own score variance in place of sigma_c(q)^2 (separates sigma's error);
- form 2: per query, the rms of e / h over the rows that pass level l at psi 4 with the derived width at the true k-th
  best, pooled over the query's clusters, times h;
- sampled s (16, 32, 64): form 1 with both variances over s rows of the cluster spread evenly over its rows, each
  scored exactly (their stored vectors read: s rows a cluster added to the rows count);
- sampled sd s: the standard deviation of e over the same s rows (the error measured directly, not inflated).
Reported: the ratio of the actual rms of e to each rule's tau (median, 10th and 90th percentiles over (query, cluster),
all clusters and those holding a true neighbour); then, replaying the two deterministic levels at the true k-th best
score t, the share of the true k that survive (a row goes on when a_l + psi tau >= t at both levels) and the rows that
reach the stored vectors, per rule and psi.
usage: python3 -I rho_check.py PREFIX PREP_DIR [k]   (PREFIX.meta, .clu, .idx, .rows, .grp.npy)
"""
import ast
import sys

import numpy as np

prefix, prep = sys.argv[1], sys.argv[2]
kk = int(sys.argv[3]) if len(sys.argv) > 3 else 10
meta = {}
for line in open(prefix + ".meta"):
    k, v = line.split(" ", 1)
    meta[k] = v.strip()
nq, K, M, L, ndl = (int(meta[x]) for x in ("nq", "k", "m", "levels", "ndl"))
rho_g = np.array(ast.literal_eval(meta["rho"]))  # groups x ndl
clu = np.fromfile(prefix + ".clu", np.float32).reshape(nq, K, 4)
idx = np.fromfile(prefix + ".idx", np.uint32).reshape(nq, M, 2)
w = 5 + 2 * L
raw = np.memmap(prefix + ".rows", np.float32, mode="r").reshape(-1, w)
ent = np.memmap(prefix + ".rows", np.uint32, mode="r").reshape(-1, w)[:, 0]
grp = np.load(prefix + ".grp.npy")
off = np.concatenate([[0], np.cumsum(idx[:, :, 1].astype(np.int64).ravel())])
order = np.load(f"{prep}/order.npy", mmap_mode="r")
nlev = min(ndl, L)
SAMPLES = (16, 32, 64)
rules = ["derived", "form 1", "form 1 exact", "form 2"] + [f"sampled {m}" for m in SAMPLES] + [f"sampled sd {m}" for m in SAMPLES] + ["oracle"]
ratios = {r: [[] for _ in range(nlev)] for r in rules[:-1]}
ratios_hit = {r: [[] for _ in range(nlev)] for r in rules[:-1]}
psis = [4.0, 5.0]
kept = {(r, p): [0, 0] for r in ["certified"] + rules for p in psis}  # true rows surviving, true rows present
passed = {(r, p): 0.0 for r in ["certified"] + rules for p in psis}


def rms(x):
    return float(np.sqrt(np.mean(np.square(x)))) if len(x) else 0.0


for qi in range(nq):
    top = set(order[qi, :kk].tolist())
    cl = []
    for j in range(M):
        c = idx[qi, j, 0]
        if c == 0xFFFFFFFF or idx[qi, j, 1] == 0:
            continue
        a, b = off[qi * M + j], off[qi * M + j + 1]
        r = np.asarray(raw[a:b], np.float64)
        e_ = np.asarray(ent[a:b])
        ex = r[:, 1]
        lv = r[:, 5:].reshape(-1, L, 2)
        mid = (lv[:, :, 0] + lv[:, :, 1]) / 2
        h = (lv[:, :, 1] - lv[:, :, 0]) / 2
        hit = np.isin(e_, list(top))
        cl.append((c, ex, mid, h, hit))
    found = np.concatenate([x[1][x[4]] for x in cl]) if cl else np.array([])
    if len(found) == 0:
        continue
    t = float(found.min())  # the true k-th best among the dumped rows of the true k
    # per cluster and level, each rule's spread
    taus = []
    for c, ex, mid, h, hit in cl:
        g = grp[c]
        sg = float(clu[qi, c, 1])
        tl = {}
        for l in range(nlev):
            e = ex - mid[:, l]
            hl = rms(h[:, l])
            tl[("derived", l)] = rho_g[g, l] * h[:, l]
            v1 = max(sg * sg - np.var(mid[:, l]), 0.0)
            tl[("form 1", l)] = np.full(len(ex), np.sqrt(v1))
            v1x = max(np.var(ex) - np.var(mid[:, l]), 0.0)
            tl[("form 1 exact", l)] = np.full(len(ex), np.sqrt(v1x))
            tl[("oracle", l)] = np.full(len(ex), rms(e))
            for m in SAMPLES:
                sel = np.unique(np.linspace(0, len(ex) - 1, min(m, len(ex))).astype(int))
                vs = max(np.var(ex[sel]) - np.var(mid[sel, l]), 0.0) if len(sel) > 1 else 0.0
                tl[(f"sampled {m}", l)] = np.full(len(ex), np.sqrt(vs))
                tl[(f"sampled sd {m}", l)] = np.full(len(ex), float(np.std(e[sel])) if len(sel) > 1 else 0.0)
            if len(ex) >= 20 and hl > 0:
                for r in ["derived", "form 1", "form 1 exact"] + [f"sampled {m}" for m in SAMPLES] + [f"sampled sd {m}" for m in SAMPLES]:
                    pred = rms(tl[(r, l)])
                    if pred > 0:
                        ratios[r][l].append(rms(e) / pred)
                        if hit.any():
                            ratios_hit[r][l].append(rms(e) / pred)
        taus.append(tl)
    # form 2: e / h pooled over the query's clusters, rows passing level l at psi 4 with the derived width
    f2 = []
    for l in range(nlev):
        num, den = 0.0, 0
        for (c, ex, mid, h, hit), tl in zip(cl, taus):
            ok = mid[:, l] + 4.0 * tl[("derived", l)] >= t
            if ok.any():
                num += float(np.sum(np.square((ex - mid[:, l])[ok] / np.maximum(h[:, l][ok], 1e-30))))
                den += int(ok.sum())
        f2.append(np.sqrt(num / den) if den else rho_g[:, l].mean())
    for (c, ex, mid, h, hit), tl in zip(cl, taus):
        for l in range(nlev):
            tl[("form 2", l)] = f2[l] * h[:, l]
            if len(ex) >= 20 and rms(tl[("form 2", l)]) > 0:
                ratios["form 2"][l].append(rms(ex - mid[:, l]) / rms(tl[("form 2", l)]))
                if hit.any():
                    ratios_hit["form 2"][l].append(rms(ex - mid[:, l]) / rms(tl[("form 2", l)]))
        for p in psis:
            for r in ["certified"] + rules:
                ok = np.ones(len(ex), bool)
                for l in range(nlev):
                    wdt = h[:, l] if r == "certified" else np.minimum(p * tl[(r, l)], h[:, l])
                    ok &= mid[:, l] + wdt >= t
                if r.startswith("sampled"):
                    # the sampled rows are scored exactly: kept whatever the levels say, and counted as read
                    sel = np.unique(np.linspace(0, len(ex) - 1, min(int(r.split()[-1]), len(ex))).astype(int))
                    ok[sel] = True
                kept[(r, p)][0] += int((ok & hit).sum())
                kept[(r, p)][1] += int(hit.sum())
                passed[(r, p)] += int(ok.sum())

name = prefix.split("/")[-1]
print(f"## {name}: {nq} queries, first {M} clusters, groups {rho_g.shape[0]}, derived rho median per level "
      f"{', '.join(f'{np.median(rho_g[:, l]):.4f}' for l in range(nlev))}")
print("actual rms(error) / rule's spread, p10 / median / p90 (all clusters | clusters holding a true neighbour)")
for r in rules[:-1]:
    for l in range(nlev):
        a = np.array(ratios[r][l])
        b = np.array(ratios_hit[r][l])
        print(f"  level {l} {r:<13} {' / '.join(f'{x:.2f}' for x in np.quantile(a, [0.1, 0.5, 0.9]))}"
              f"  |  {' / '.join(f'{x:.2f}' for x in np.quantile(b, [0.1, 0.5, 0.9])) if len(b) else '-'}")
print("replay at the true k-th best: share of the true k kept, rows reaching the stored vectors per query")
for p in psis:
    for r in ["certified"] + rules:
        s, n = kept[(r, p)]
        print(f"  psi {p:g} {r:<13} kept {s / max(n, 1):.3f}  rows {passed[(r, p)] / nq:.0f}")
