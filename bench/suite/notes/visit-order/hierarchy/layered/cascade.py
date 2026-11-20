"""Replay B, part 2, for layered.md: what a cluster read costs when rows are dropped by the stored key and level
codes at a statistical width instead of the certified one. Counts only (nothing is timed).

Input: `study_layered_dump`'s files for one dataset and visible set (the library's k-NN order, each cluster's mu
and sigma, and for the first M clusters every visible row's exact score, key and certified intervals from the key and
from each level). For each query the clusters are read in the library's order with a running k-th best t, under one
of these row rules:

  direct   every visible row scored from its stored vector (what the library does for most reads at 1M);
  cert     the index path: the key's certified range, then each level, a row dropped when its certified upper bound
           is below t; the rest scored from stored vectors;
  stat psi the levels at a statistical width: a row dropped when min(certified upper bound, a + psi tau) < t, a the
           level's estimate (the midpoint of its certified interval) and tau = rho h, h the certified
           half-width. `stat-d`: rho = 1 / sqrt(d), d the vectors' dimension (Cauchy-Schwarz's worst alignment
           replaced by a random one; no constant). `stat-m`: rho per level as a build would measure it on the index
           (`measured_rho`). The key's range stays certified. With `key`, the key's range at a statistical width too:
           a = (q.m^) key, tau_i = sqrt(1 - key_i^2) tau0, tau0^2 = (sigma_c^2 - (q.m^)^2 Var_c(key)) / E_c[1 -
           key^2] (the cluster's spread less what the key explains).

`cert` and `stat` read a cluster through the levels only when that saves bytes: when its range's rows times (a stored
vector less a first-level row) exceed the cluster's tables plus, when the group is new to the query, the group's
setup (n^2); else its rows are scored directly. (The library decides the same by its measured costs.)

Until k rows are held, `cert` and `stat` seed: the first level over the whole cluster and its 2k + 8 best estimates
scored in full (as the exact k-NN's seed does); `direct` scores the cluster.

Stops: the same number of clusters for every query (the nprobe curve), or the library's stop (the next 8 clusters'
largest mu + psi sigma below t). Reported at recall targets: bytes read by part, rows entering each stage, clusters
and groups.

usage: python3 cascade.py DATASET DUMP_PREFIX OUT.json [k ...]
"""
import json
import os
import sys
import time

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from dump import Dump  # noqa: E402

TARGETS = (0.8, 0.9, 0.95, 0.98, 0.99, 0.995)
PSIS = (2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 6.0, 7.0, 8.0)
STOPS = (1.0, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 6.0, 7.0, 8.0)
LOOKAHEAD = 8
DIRECT_MAX = 32


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


class Rule:
    """kind: direct, cert or stat; for stat: psi, the levels used (indices, ascending), whether the key's range is
    statistical too, and tau's rho (d: 1 / sqrt(d); m: measured)."""

    def __init__(self, kind, psi=None, use=(0, 1), key=False, tau="d"):
        self.kind, self.psi, self.use, self.key, self.tau = kind, psi, tuple(use), key, tau
        self.name = kind

    def named(self, D):
        if self.kind == "stat":
            self.name = f"stat-{self.tau} psi={self.psi:g} L" + "+".join(str(D.level_bytes[l]) for l in self.use) + (" key" if self.key else "")
        return self


def measured_rho(D, nq=20, first=24):
    """Per level, the root mean square of (score - estimate) / certified half-width over the rows of the clusters
    ranked `first` and beyond of the first `nq` queries: what a build would measure on the index (how much of
    Cauchy-Schwarz's worst alignment the code's errors realise), here on rows that hold almost no answer."""
    s, n = np.zeros(D.L), 0
    for qi in range(min(nq, D.nq)):
        for j in range(min(first, D.M - 1), D.M):
            if D.idx[qi, j, 1] < 50:
                continue
            _, ex, _, _, _, lv = D.cluster(qi, j)
            a, h = 0.5 * (lv[:, :, 0] + lv[:, :, 1]), 0.5 * (lv[:, :, 1] - lv[:, :, 0])
            s += (((ex[:, None] - a) / np.maximum(h, 1e-12)) ** 2).sum(0)
            n += len(ex)
    return np.sqrt(s / max(n, 1))


def run(D, rule, k, truth):
    """One pass per query. Returns per query and cluster position: the counts after reading it, recall, and t before."""
    L = D.L
    nst = 3 + L  # counts: rows offered, rows in the key range, rows entering each level..., rows scored in full
    out_cnt = np.zeros((D.nq, D.M, nst + 3))  # + clusters on the index path, new groups, clusters read
    out_rec = np.zeros((D.nq, D.M))
    out_t = np.full((D.nq, D.M), -np.inf)
    rho = D.rho[rule.tau] if rule.kind == "stat" else None
    stat = rule.kind == "stat"
    use = tuple(range(L)) if rule.kind == "cert" else rule.use if stat else ()
    l0 = use[0] if use else 0
    # the index path pays for a cluster when the bytes it saves (stored vectors against first-level rows) exceed its
    # fixed bytes: the cluster's tables, and the group's setup when the group is new to the query
    lbs = D.level_bytes
    full_b = 2 * D.n
    first_b = (lbs[l0] + 2.5) if use else 0
    tab_b = 2 * (lbs[use[-1]] if use else 0) * 96

    def pays(nrows, g, groups):
        return nrows * (full_b - first_b) > tab_b + (0 if g in groups else D.n * D.n)

    for qi in range(D.nq):
        tr = truth[qi, :k]
        bs, be = np.empty(0, np.float32), np.empty(0, np.uint32)
        groups = set()
        cnt = np.zeros(nst + 3)

        def merge(ex, ent):
            nonlocal bs, be
            bs = np.concatenate([bs, ex])
            be = np.concatenate([be, ent])
            if len(bs) > k:
                p = np.argpartition(-bs, k - 1)[:k]
                bs, be = bs[p], be[p]

        for j in range(D.M):
            c = D.idx[qi, j, 0]
            nv = D.idx[qi, j, 1]
            out_t[qi, j] = bs.min() if len(bs) >= k else -np.inf
            if nv > 0:
                ent, ex, key, klo, khi, lv = D.cluster(qi, j)
                cnt[0] += nv
                cnt[nst + 2] += 1
                done = np.zeros(nv, bool)  # rows already scored in full
                seeded = False
                g = D.grp[c]
                if rule.kind != "direct" and len(bs) < k and nv >= 4 * k + DIRECT_MAX and pays(nv - 2 * k - 8, g, groups):
                    # the seed: the first level over the whole cluster, its 2k + 8 best estimates scored in full
                    a0 = 0.5 * (lv[:, l0, 0] + lv[:, l0, 1])
                    top = np.argpartition(-a0, 2 * k + 8)[:2 * k + 8]
                    merge(ex[top], ent[top])
                    done[top] = True
                    cnt[nst - 1] += len(top)
                    cnt[1] += nv
                    cnt[2 + l0] += nv
                    seeded = True
                if rule.kind == "direct" or len(bs) < k:
                    keep = ~done
                else:
                    t = bs.min()
                    mu, sg = float(D.clu[qi, c, 0]), float(D.clu[qi, c, 1])
                    ub = khi
                    if stat and rule.key:
                        cm = mu / D.mn[c]
                        vk = max(float(D.kst[c, 1] - D.kst[c, 0] ** 2), 0.0)
                        tau0sq = max(sg * sg - cm * cm * vk, 0.0) / max(1.0 - float(D.kst[c, 1]), 1e-9)
                        ub = np.minimum(ub, cm * key + rule.psi * np.sqrt(np.maximum(1.0 - key * key, 0.0) * tau0sq))
                    keep = (ub >= t) & ~done
                    if not seeded:
                        cnt[1] += keep.sum()
                    if seeded or pays(keep.sum(), g, groups):
                        if seeded or keep.any():
                            cnt[nst] += 1
                            if g not in groups:
                                groups.add(g)
                                cnt[nst + 1] += 1
                        for l in use:
                            idx = np.flatnonzero(keep)
                            if len(idx) == 0:
                                break
                            if not (seeded and l == l0):
                                cnt[2 + l] += len(idx)
                            lo, hi = lv[idx, l, 0], lv[idx, l, 1]
                            ub = hi
                            if stat:
                                # the certified half-width's worst alignment replaced by a random one over d dimensions
                                ub = np.minimum(hi, 0.5 * (lo + hi) + rule.psi * rho[l] * 0.5 * (hi - lo))
                            keep[idx] = ub >= t
                idx = np.flatnonzero(keep)
                cnt[nst - 1] += len(idx)
                if len(idx):
                    merge(ex[idx], ent[idx])
            out_cnt[qi, j] = cnt
            out_rec[qi, j] = np.isin(be, tr).sum() / k
    return out_cnt, out_rec, out_t


def costs(D, cnt, rule):
    """Bytes read, by part, from the counts (any leading shape): level codes, stored vectors, group setups, tables."""
    L = D.L
    nst = 3 + L
    lb = D.level_bytes
    code = 0.0
    use = tuple(range(L)) if rule.kind == "cert" else rule.use if rule.kind == "stat" else ()
    prev = 0
    for i, l in enumerate(use):
        # a used level's bytes: the codes since the last used level; the key (2 B) with the first; a norm code (0.5 B)
        # for a deterministic level; the first randomised level's scalars (5.5 B)
        b = lb[l] - prev + (2 if i == 0 else 0) + (0.5 if l < D.ndl else 5.5 if l == D.ndl else 0)
        prev = lb[l]
        code = code + cnt[..., 2 + l] * b
    full = cnt[..., nst - 1] * 2 * D.n
    # a group's setup: the int8 basis product (n^2 bytes); a cluster's tables: 16 entries of 2 B per 4-bit unit of
    # the levels used (written), from the group's and the centre's (read): 3 x
    units = 2 * (lb[use[-1]] if use else 0)
    grp = cnt[..., nst + 1] * D.n * D.n
    tab = cnt[..., nst] * units * 16 * 2 * 3
    return code, full, grp, tab


def at_recall(rec, vals):
    """Each column of vals (per point of a curve of increasing recall) interpolated at the recall targets."""
    out = []
    o = np.argsort(rec, kind="stable")
    rec, vals = rec[o], vals[o]
    for tg in TARGETS:
        i = np.searchsorted(rec, tg - 1e-12)
        if i >= len(rec):
            out.append(None)
        elif i == 0:
            out.append([float(x) for x in vals[0]])
        else:
            f = (tg - rec[i - 1]) / max(rec[i] - rec[i - 1], 1e-12)
            out.append([float(x) for x in vals[i - 1] + f * (vals[i] - vals[i - 1])])
    return out


def stop_index(D, t, psi):
    """Per query, the clusters read under the library's stop: the first position (after k are held) at which the next
    LOOKAHEAD clusters' largest mu + psi sigma is below the running k-th best."""
    nq, M = t.shape
    n = np.full(nq, M)
    for qi in range(nq):
        cl = D.idx[qi, :, 0]
        live = D.idx[qi, :, 1] > 0
        key = np.where(live, D.clu[qi, np.where(live, cl, 0), 0] + psi * D.clu[qi, np.where(live, cl, 0), 1], -np.inf)
        for j in range(1, M):
            if not live[j:].any():
                n[qi] = j
                break
            if t[qi, j] > -np.inf and key[j:j + LOOKAHEAD].max() < t[qi, j]:
                n[qi] = j
                break
    return n


def main():
    ds, prefix, outp = sys.argv[1], sys.argv[2], sys.argv[3]
    ks = [int(x) for x in sys.argv[4:]] or [10]
    D = Dump(prefix, ds)
    truth = np.asarray(D.truth()[:, :max(ks)])
    log(ds, D.set, "queries", D.nq, "clusters dumped", D.M, "levels", D.level_bytes[:D.L], "n", D.n)
    D.rho = {"d": np.full(D.L, 1 / np.sqrt(D.d)), "m": measured_rho(D)}
    log("rho: 1/sqrt(d)", f"{1 / np.sqrt(D.d):.4f}", "measured per level", [f"{x:.4f}" for x in D.rho["m"]])
    rules = [Rule("direct"), Rule("cert")]
    lb = D.level_bytes[:D.L]
    base = (lb.index(16), lb.index(32))  # the default build's two deterministic levels
    for tau in ("d", "m"):
        for psi in PSIS:
            rules.append(Rule("stat", psi, base, tau=tau))
    variants = [(base, True), (base[:1], False)]
    if 56 in lb:
        variants.append((base + (lb.index(56),), False))
    for first in (4, 8):
        if first in lb:
            variants.append(((lb.index(first),) + base, False))
            variants.append(((lb.index(first), base[1]), False))
    if 4 in lb:
        variants.append((tuple(range(lb.index(32) + 1)), False))
    for use, key in variants:
        for psi in (3.0, 4.0, 5.0, 6.0, 7.0, 8.0):
            rules.append(Rule("stat", psi, use, key=key, tau="m"))
    rules = [r.named(D) for r in rules]
    res = {"dataset": ds, "set": D.set, "base": list(base), "rho_measured": [float(x) for x in D.rho["m"]], "d": D.d, "nq": D.nq, "K": D.K, "M": D.M, "n": D.n, "level_bytes": D.level_bytes, "targets": TARGETS, "runs": {}}
    fixed = D.K * D.n  # the centre scan, int8
    names = ["bytes", "code", "full", "group", "table", "sigma", "clusters", "groups", "rows", "in_range"] + [f"enter_l{l}" for l in range(D.L)] + ["full_rows"]
    res["columns"] = names
    res["fixed_centre_bytes"] = fixed
    for k in ks:
        for rule in rules:
            cnt, rec, t = run(D, rule, k, truth)
            nst = 3 + D.L

            def cols(c, nclu):
                code, full, grp, tab = costs(D, c, rule)
                sig = (nclu + LOOKAHEAD) * 16 * D.n
                tot = fixed + code + full + grp + tab + sig
                return [tot, code, full, grp, tab, sig, c[..., nst + 2], c[..., nst + 1], c[..., 0], c[..., 1]] + [c[..., 2 + l] for l in range(D.L)] + [c[..., nst - 1]]

            # the nprobe curve: the same number of clusters for every query
            mrec = rec.mean(0)
            mv = np.stack([np.broadcast_to(x, (D.M,)) if np.ndim(x) == 0 else x for x in cols(cnt.mean(0), np.arange(1, D.M + 1))], 1)
            e = {"nprobe": at_recall(mrec, mv), "max_recall": float(mrec[-1])}
            # the library's stop at each psi
            pts_r, pts_v = [], []
            for ps in STOPS:
                n = stop_index(D, t, ps)
                qi = np.arange(D.nq)
                c = cnt[qi, n - 1]
                pts_r.append(rec[qi, n - 1].mean())
                pts_v.append([float(np.mean(x)) for x in cols(c, n)])
            e["stop"] = at_recall(np.array(pts_r), np.array(pts_v))
            e["stop_points"] = [[float(ps), float(r)] + v for ps, r, v in zip(STOPS, pts_r, pts_v)]
            if rule.kind == "stat":
                # one dial: the stop's psi is the rows' psi
                n = stop_index(D, t, rule.psi)
                qi = np.arange(D.nq)
                e["one_dial"] = [float(rec[qi, n - 1].mean())] + [float(np.mean(x)) for x in cols(cnt[qi, n - 1], n)]
            res["runs"][f"k={k} {rule.name}"] = e
            log(f"k={k}", rule.name, "max recall", f"{mrec[-1]:.4f}")
    with open(outp, "w") as f:
        json.dump(res, f)
    log("wrote", outp)


if __name__ == "__main__":
    main()
