# Replay checks for hierarchy/theory.md on one dataset prepared by prep.py. Counts only; nothing is timed.
#   python3 hier.py DATASET SCRATCH_DIR [sections, default ABCDEFGHI]
# Levels: G (the fit's groups), C (the fit's clusters), a4 / a16 / a64 (4, 16, 64 nested sub-clusters per cluster),
# d16 (16 sub-clusters by one k-means). A "curve" probes whole leaves in a key's order with the same number of leaves
# for every query, and reports mean rows read against mean recall, interpolated linearly between leaf counts.
import sys, numpy as np
from statistics import NormalDist
ND = NormalDist()
ds, S_ = sys.argv[1], sys.argv[2]
SECT = sys.argv[3] if len(sys.argv) > 3 else "ABCDEFGHI"
D = f"{S_}/{ds}/"
I = f"/home/user/suite-data/work/{ds}/vexact/index/"
S = np.load(D + "S.npy")
Q = np.load(D + "Q.npy")
nq, N = S.shape
cstart = np.load(I + "cstart.npy"); K = len(cstart) - 1
grp = np.load(I + "grp.npy")
rowc = np.repeat(np.arange(K), np.diff(cstart)).astype(np.int32)
LAB = {"C": rowc, "G": grp[rowc].astype(np.int32)}
for nm in ("a4", "a16", "a64", "d16"):
    LAB[nm] = np.load(D + nm + ".npy")
C64 = np.load(D + "a64_cent.npy"); n64 = np.bincount(LAB["a64"])
par64 = {"a64": np.arange(len(n64))}


def centres(nm):
    """Centres (means), counts and rad = sqrt(1 - |centre|^2) of a level."""
    lab = LAB[nm]; L = lab.max() + 1
    if nm == "d16":
        C = np.load(D + "d16_cent.npy"); cnt = np.bincount(lab, minlength=L)
    else:
        m = np.zeros(len(n64), np.int64); m[LAB["a64"]] = lab            # a64 leaf -> this level's node
        cnt = np.bincount(m, weights=n64, minlength=L)
        C = np.zeros((L, C64.shape[1])); np.add.at(C, m, C64 * n64[:, None]); C /= cnt[:, None]
    return C.astype(np.float32), cnt, np.sqrt(np.maximum(1 - (C * C).sum(1), 0))


def stats(nm, vis=None):
    """Per query and leaf: mean score, variance of the scores, and rows (visible rows if vis is given)."""
    lab = LAB[nm]; L = lab.max() + 1
    if vis is not None:
        lab = lab[vis]
    cnt = np.bincount(lab, minlength=L).astype(np.float64)
    mu = np.zeros((nq, L)); var = np.zeros((nq, L))
    for q in range(nq):
        s = S[q] if vis is None else S[q, vis]
        a = np.bincount(lab, weights=s, minlength=L); b = np.bincount(lab, weights=s.astype(np.float64) ** 2, minlength=L)
        mu[q] = a / np.maximum(cnt, 1); var[q] = np.maximum(b / np.maximum(cnt, 1) - mu[q] ** 2, 0)
    return mu, var, cnt


def hits_of(nm, rows):
    """rows: list (per query) of row indices of the answer. Returns answers per leaf (nq x L)."""
    lab = LAB[nm]; L = lab.max() + 1
    return np.stack([np.bincount(lab[r], minlength=L) for r in rows]).astype(np.float64)


def curve(key, cnt, hits, allow=None):
    """Mean rows and mean recall after 0..L leaves in decreasing key. Leaves with no rows are never probed.
    allow: optional boolean nq x L, leaves that may be probed (two-level probing)."""
    key = np.where(cnt[None] > 0, key, -np.inf) if cnt.ndim == 1 else np.where(cnt > 0, key, -np.inf)
    if allow is not None:
        key = np.where(allow, key, -np.inf)
    o = np.argsort(-key, axis=1)
    c = np.broadcast_to(cnt, key.shape) if cnt.ndim == 1 else cnt
    ok = np.take_along_axis(key, o, 1) > -np.inf
    rows = np.cumsum(np.take_along_axis(c, o, 1) * ok, 1).mean(0)
    rec = (np.cumsum(np.take_along_axis(hits, o, 1) * ok, 1) / hits.sum(1, keepdims=True)).mean(0)
    return np.r_[0, rows], np.r_[0, rec], o


def rows_at(rows, rec, r):
    """Mean rows at which the curve first reaches mean recall r (nan if never)."""
    if rec[-1] < r:
        return np.nan
    j = np.searchsorted(rec, r)
    if j == 0:
        return 0.0
    return rows[j - 1] + (rows[j] - rows[j - 1]) * (r - rec[j - 1]) / max(rec[j] - rec[j - 1], 1e-12)


def rec_at(rows, rec, b):
    return float(np.interp(b, rows, rec))


def sd(x):
    return x.std(1, keepdims=True)


def spread_key(mu, rad, f, cnt=None):
    """q.m + f sd(q) rad / sd(rad), sd over the leaves that hold rows."""
    w = slice(None) if cnt is None else cnt > 0
    return mu + f * sd(mu[:, w]) * rad[None] / rad[w].std()


def topk_rows(k, vis=None):
    out = []
    for q in range(nq):
        s = S[q] if vis is None else S[q, vis]
        p = np.argpartition(-s, k)[:k]
        out.append(p if vis is None else vis[p])
    return out


def fmt(x, n=N):
    return " nan " if np.isnan(x) else f"{100 * x / n:5.2f}"


print(f"== {ds}: N {N}, K {K}, groups {grp.max() + 1}, queries {nq}", flush=True)
CEN = {nm: centres(nm) for nm in LAB}
ST = {}
def st(nm):
    if nm not in ST:
        ST[nm] = stats(nm)
    return ST[nm]

# ---------------------------------------------------------------- A: granularity and keys, unfiltered
if "A" in SECT:
    for k in (10, 100):
        truth = topk_rows(k)
        tau = np.array([S[q, truth[q]].min() for q in range(nq)])[:, None]
        print(f"\nA. k = {k}, unfiltered: share of rows (%) to mean recall .8 / .9 / .95 | recall at 0.5 / 1.6 / 6.3% of rows")
        for nm in ("G", "C", "a4", "a16", "d16", "a64"):
            mu, var, cnt = st(nm); rad = CEN[nm][2]; h = hits_of(nm, truth); sg = np.sqrt(var)
            keys = {"centre score": mu, "+0.2 rad": mu + 0.2 * rad[None], "spread f'=0.4": spread_key(mu, rad, 0.4)}
            best = None
            for f in (0.2, 0.3, 0.5, 0.6, 0.8, 1.0, 1.3, 1.6, 2.0):
                r, c, _ = curve(spread_key(mu, rad, f), cnt, h); v = rows_at(r, c, 0.9)
                if best is None or v < best[0]:
                    best = (v, f)
            keys[f"spread best f' ({best[1]})"] = spread_key(mu, rad, best[1])
            best = None
            for psi in (1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0):
                r, c, _ = curve(mu + psi * sg, cnt, h); v = rows_at(r, c, 0.9)
                if best is None or v < best[0]:
                    best = (v, psi)
            keys[f"mu + psi sigma exact, best psi ({best[1]})"] = mu + best[1] * sg
            keys["zeta, exact sigma, true tau"] = -(tau - mu) / np.maximum(sg, 1e-9)
            keys["(tau - mu) / rad, true tau"] = -(tau - mu) / np.maximum(rad[None], 1e-6)
            keys["clairvoyant"] = h / np.maximum(cnt, 1)[None]
            for kn, key in keys.items():
                r, c, _ = curve(key, cnt, h)
                print(f"  {nm:4s} {kn:42s} {fmt(rows_at(r, c, .8))} {fmt(rows_at(r, c, .9))} {fmt(rows_at(r, c, .95))} | "
                      f"{rec_at(r, c, .005 * N):.3f} {rec_at(r, c, .016 * N):.3f} {rec_at(r, c, .063 * N):.3f}", flush=True)

# ---------------------------------------------------------------- B: two-level probing, node keys
if "B" in SECT:
    k = 10; truth = topk_rows(k)
    muC, varC, cntC = st("C"); radC = CEN["C"][2]
    for nm in ("a16", "d16"):
        mu, var, cnt = st(nm); rad = CEN[nm][2]; h = hits_of(nm, truth)
        par = np.zeros(len(cnt), np.int64); par[LAB[nm]] = rowc
        F = np.bincount(par, minlength=K)
        # between-child spread s_g(q) (sd of child centre scores, weighted by rows) and mean within-child variance
        w = cnt[None] / cntC[par][None]
        sg2 = np.zeros((nq, K)); wv = np.zeros((nq, K))
        for q in range(nq):
            sg2[q] = np.bincount(par, weights=w[0] * (mu[q] - muC[q, par]) ** 2, minlength=K)
            wv[q] = np.bincount(par, weights=w[0] * var[q], minlength=K)
        ckey = spread_key(mu, rad, 0.4)
        zF = ND.inv_cdf(1 - 1 / 16)
        oracle = np.full((nq, K), -np.inf)
        for q in range(nq):
            np.maximum.at(oracle[q], par, ckey[q])
        pkeys = {"centre score": muC, "spread f'=0.4 (cluster)": spread_key(muC, radC, 0.4),
                 "mu + 3 sigma_C (exact total spread)": muC + 3 * np.sqrt(varC),
                 "mu + 2 sigma_C": muC + 2 * np.sqrt(varC),
                 "mu + z(1,F) s_g (between-child only)": muC + zF * np.sqrt(sg2),
                 "mu + z(1,F) s_g + 2 sigma_within": muC + zF * np.sqrt(sg2) + 2 * np.sqrt(wv),
                 "oracle: best child key": oracle}
        r0, c0, o0 = curve(ckey, cnt, h)
        print(f"\nB. two-level probing, {nm} under C, k = 10, child key spread f'=0.4. Flat order over every sub-centre: "
              f"rows to .8/.9/.95 = {fmt(rows_at(r0, c0, .8))} {fmt(rows_at(r0, c0, .9))} {fmt(rows_at(r0, c0, .95))} %")
        print("   parent key: share of rows (%) to recall .9 with the best P parents expanded, P = 8, 16, 32, 64, 128 | "
              "parents needed to hold 95% / 100% of the first L flat leaves (mean; L = leaves to recall .9) | groups those parents lie in")
        L9 = int(np.searchsorted(c0, 0.9))
        for kn, pk in pkeys.items():
            rk = np.argsort(np.argsort(-pk, axis=1), axis=1)            # rank of each parent
            out = []
            for P in (8, 16, 32, 64, 128):
                r, c, _ = curve(ckey, cnt, h, allow=(rk < P)[:, par]); out.append(fmt(rows_at(r, c, .9)))
            need = np.sort(np.take_along_axis(rk[:, par], o0[:, :L9], 1), axis=1)
            p95 = need[:, int(np.ceil(0.95 * L9)) - 1] + 1; p100 = need[:, -1] + 1
            ng = np.mean([len(np.unique(grp[np.argsort(-pk[q])[:p95[q]]])) for q in range(nq)])
            print(f"   {kn:40s} {' '.join(out)} | {p95.mean():6.1f} {p100.mean():6.1f} (L = {L9}, L/16 = {L9 / 16:.1f}) | {ng:.1f}", flush=True)

# ---------------------------------------------------------------- C: variance explained by a level, and the beam
if "C" in SECT:
    print("\nC. share f of the variance (over leaves) of the leaf centre score explained by the parent's centre score; "
          "share of parents that must be kept to hold 99% of the top leaves (by centre score), measured | predicted by (2.2)")
    def parent_of(child, parent):
        p = np.zeros(LAB[child].max() + 1, np.int64); p[LAB[child]] = LAB[parent]; return p
    for child, parent in (("C", "G"), ("a4", "C"), ("a16", "C"), ("a16", "a4"), ("a64", "a16"), ("a64", "C"), ("a16", "G"), ("a64", "G")):
        mu = st(child)[0]; mup = st(parent)[0]; p = parent_of(child, parent)
        Lc, Lp = mu.shape[1], mup.shape[1]
        f = (mup[:, p].var(1) / mu.var(1))
        line = f"   {parent:>3s} -> {child:4s} ({Lp} -> {Lc}): f mean {f.mean():.3f} (10th-90th pct {np.quantile(f, .1):.2f}-{np.quantile(f, .9):.2f});"
        rkp = np.argsort(np.argsort(-mup, axis=1), axis=1)
        for top in (0.002, 0.01, 0.05):
            L = max(1, int(round(top * Lc)))
            o = np.argsort(-mu, axis=1)[:, :L]
            ranks = np.take_along_axis(rkp[:, p], o, 1).ravel()
            meas = (np.quantile(ranks, 0.99) + 1) / Lp
            fm = float(f.mean()); zT = ND.inv_cdf(1 - top); ze = ND.inv_cdf(0.99)
            pred = 1 - ND.cdf(zT * np.sqrt(fm) - ze * np.sqrt(1 - fm))
            line += f" top {100 * top:g}%: {meas:.3f} | {pred:.3f};"
        print(line, flush=True)

# ---------------------------------------------------------------- D: sub-centres in the top coordinates of the group basis
if "D" in SECT:
    V = np.load(I + "V.npy")
    k = 10; truth = topk_rows(k)
    muC = st("C")[0]
    X = None
    print("\nD. sub-centre scores from the first r coordinates of the group's basis (q.s = q.m_c + sum_{i<r} (V'q)_i (V^-1 (s - m_c))_i)")
    for nm in ("d16", "a16", "a64"):
        Cs, cnt, rad = CEN[nm]; mu = st(nm)[0]; h = hits_of(nm, truth)
        par = np.zeros(len(cnt), np.int64); par[LAB[nm]] = rowc
        off = Cs.astype(np.float64) - CEN["C"][0][par]
        g = grp[par]
        exact = mu - muC[:, par]                                   # q.(s - m_c), nq x L
        res = {}
        rs = [r for r in (2, 4, 8, 16, 32, 64, 128) if r <= V.shape[1]]
        approx = {r: np.zeros_like(exact) for r in rs}
        for gg in range(V.shape[0]):
            w = np.where(g == gg)[0]
            if len(w) == 0:
                continue
            Vg = V[gg].astype(np.float64)
            z = np.linalg.solve(Vg, off[w].T).T                     # coordinates of the offsets
            p = Q.astype(np.float64) @ Vg                           # query coordinates
            for r in rs:
                approx[r][:, w] = p[:, :r] @ z[:, :r].T
        r0, c0, _ = curve(spread_key(mu, rad, 0.4), cnt, h)
        line = (f"   {nm}: rms of q.(s - m_c) {np.sqrt((exact ** 2).mean()):.4f}, sd over clusters of q.m_c {muC.std(1).mean():.4f}; exact sub-centre scores: "
                f"rows to .9 = {fmt(rows_at(r0, c0, .9))}%.  r: residual variance share, rows to .9:")
        for r in rs:
            ev = ((exact - approx[r]) ** 2).mean() / (exact ** 2).mean()
            rr, cc, _ = curve(spread_key(muC[:, par] + approx[r], rad, 0.4), cnt, h)
            line += f"  r={r}: {ev:.3f}, {fmt(rows_at(rr, cc, .9))}%"
        print(line, flush=True)

# ---------------------------------------------------------------- E: approximate threshold queries
if "E" in SECT:
    print("\nE. approximate threshold queries: answer = rows scoring at least t, t set per query for an answer of the given share. "
          "Share of rows (%) read to mean recall .8 / .9 / .95")
    for share in (1e-4, 1e-3, 1e-2, 1e-1):
        m = int(round(share * N))
        truth = topk_rows(m)
        t = np.array([S[q, truth[q]].min() for q in range(nq)])[:, None]
        for nm in ("C", "a16", "a64"):
            mu, var, cnt = st(nm); rad = CEN[nm][2]; h = hits_of(nm, truth); sg = np.sqrt(var)
            keys = {"centre score": mu, "spread f'=0.4": spread_key(mu, rad, 0.4)}
            best = None
            for f in (-1.0, -0.6, -0.3, -0.15, 0.0, 0.15, 0.3, 0.5, 0.7, 1.0, 1.5):
                r, c, _ = curve(spread_key(mu, rad, f), cnt, h); v = rows_at(r, c, 0.9)
                if best is None or v < best[0]:
                    best = (v, f)
            keys[f"spread best f' ({best[1]})"] = spread_key(mu, rad, best[1])
            keys["zeta(t), exact sigma"] = -(t - mu) / np.maximum(sg, 1e-9)
            keys["(t - mu) / rad (no constant)"] = -(t - mu) / np.maximum(rad[None], 1e-6)
            keys["clairvoyant"] = h / np.maximum(cnt, 1)[None]
            for kn, key in keys.items():
                r, c, _ = curve(key, cnt, h)
                print(f"  answer {100 * share:g}%  {nm:4s} {kn:52s} {fmt(rows_at(r, c, .8))} {fmt(rows_at(r, c, .9))} {fmt(rows_at(r, c, .95))}", flush=True)

# ---------------------------------------------------------------- F: visible sets
if "F" in SECT:
    rng = np.random.default_rng(7)
    X = np.load(I + "Xs.npy", mmap_mode="r")
    seed_row = np.asarray(X[rng.integers(N)], np.float32)
    proj = np.concatenate([np.asarray(X[i:i + 100_000], np.float32) @ seed_row for i in range(0, N, 100_000)])
    sets = {"random 1%": np.sort(rng.choice(N, N // 100, replace=False)),
            "random 25%": np.sort(rng.choice(N, N // 4, replace=False)),
            "correlated 5% (rows nearest one corpus row)": np.sort(np.argpartition(-proj, N // 20)[:N // 20]),
            "correlated 1% (rows nearest one corpus row)": np.sort(np.argpartition(-proj, N // 100)[:N // 100])}
    print("\nF. visible sets, k = 10 among visible rows. Units probed in the order spread f'=0.4 (keys from whole-cluster centres and rad; "
          "units with no visible row skipped). Visible rows read to mean recall .8 / .9 / .95 | units probed at .9 | "
          "units holding a visible row / all units | cost at .9 in row-equivalents = visible rows + centres scored "
          "(flat: every centre; two-level: K + 16 x 2 x parents of the probed leaves)")
    for sn, vis in sets.items():
        truth = topk_rows(10, vis)
        for nm in ("G", "C", "a16", "a64"):
            mu, var, cntv = stats(nm, vis)                          # mu here is over visible rows; keys below use whole-leaf centres
            mu_all = st(nm)[0]; rad = CEN[nm][2]; h = hits_of(nm, truth)
            out = []
            for kn, key in (("whole-unit centres", spread_key(mu_all, rad, 0.4)), ("clairvoyant", h / np.maximum(cntv, 1)[None])):
                r, c, o = curve(key, cntv, h)
                j9 = int(np.searchsorted(c, 0.9))
                live = int((cntv > 0).sum()); L = len(cntv)
                v9 = rows_at(r, c, .9)
                if nm in ("a16", "a64"):
                    par = np.zeros(L, np.int64); par[LAB[nm]] = rowc
                    fan = 16 if nm == "a16" else 64
                    npar = np.mean([len(np.unique(par[o[q, :j9]])) for q in range(nq)])
                    cost = f"flat {v9 + live:9.0f}, two-level {v9 + K + fan * 2 * npar:9.0f}"
                else:
                    cost = f"flat {v9 + live:9.0f}"
                print(f"  {sn:44s} {nm:4s} {kn:20s} {rows_at(r, c, .8):9.0f} {v9:9.0f} {rows_at(r, c, .95):9.0f} | {j9:6d} | {live:6d} / {L:6d} | {cost}", flush=True)

# ---------------------------------------------------------------- G: what a probe costs besides rows
if "G" in SECT:
    k = 10; truth = topk_rows(k)
    print("\nG. unfiltered, k = 10: at mean recall .9 and .95, mean rows, clusters touched, groups touched; cost in row-equivalents "
          "= rows + 200 x clusters + h_g x groups for h_g = 2,500 and 5,500")
    for nm in ("C", "a16", "a64"):
        mu, var, cnt = st(nm); rad = CEN[nm][2]; h = hits_of(nm, truth)
        par = np.zeros(len(cnt), np.int64); par[LAB[nm]] = rowc
        key = spread_key(mu, rad, 0.4)
        r, c, o = curve(key, cnt, h)
        for rec in (0.9, 0.95):
            j = int(np.searchsorted(c, rec))
            ncl = np.mean([len(np.unique(par[o[q, :j]])) for q in range(nq)])
            ng = np.mean([len(np.unique(grp[par[o[q, :j]]])) for q in range(nq)])
            rw = rows_at(r, c, rec)
            print(f"   {nm:4s} recall {rec}: rows {rw:8.0f}, clusters {ncl:6.1f}, groups {ng:5.1f}; cost {rw + 200 * ncl + 2500 * ng:8.0f} / {rw + 200 * ncl + 5500 * ng:8.0f}", flush=True)
    # a group-aware order for clusters: greedy, a cluster in a group not yet opened is charged pen x sd(q) on its key
    for nm in ("C", "a16"):
        mu, var, cnt = st(nm); rad = CEN[nm][2]; h = hits_of(nm, truth)
        par = np.zeros(len(cnt), np.int64); par[LAB[nm]] = rowc
        g = grp[par]; key0 = spread_key(mu, rad, 0.4); sq = mu.std(1)
        for pen in (0.0, 0.25, 0.5, 1.0, 2.0):
            L = len(cnt); T = min(L, 400 if nm == "C" else 3000)
            o = np.zeros((nq, T), np.int64)
            for q in range(nq):
                opened = np.zeros(grp.max() + 1, bool)
                import heapq
                # lazy greedy: the unpenalised key is an upper bound of a leaf's current key
                heap = [(-key0[q, i], i) for i in range(L)]; heapq.heapify(heap); t = 0
                while t < T:
                    kk, i = heapq.heappop(heap)
                    cur = key0[q, i] - (0 if opened[g[i]] else pen * sq[q])
                    if cur < -kk - 1e-15:
                        heapq.heappush(heap, (-cur, i)); continue
                    o[q, t] = i; t += 1; opened[g[i]] = True
            rows = np.r_[0, np.cumsum(cnt[o], 1).mean(0)]; rec = np.r_[0, (np.cumsum(np.take_along_axis(h, o, 1), 1) / h.sum(1, keepdims=True)).mean(0)]
            line = f"   {nm:4s} group penalty {pen:4.2f} sd(q):"
            for rc in (0.9, 0.95):
                if rec[-1] < rc:
                    line += " (not reached)"; continue
                j = int(np.searchsorted(rec, rc))
                ncl = np.mean([len(np.unique(par[o[q, :j]])) for q in range(nq)])
                ng = np.mean([len(np.unique(g[o[q, :j]])) for q in range(nq)])
                rw = rows_at(rows, rec, rc)
                line += f"  recall {rc}: rows {rw:8.0f}, groups {ng:5.1f}, cost {rw + 200 * ncl + 2500 * ng:8.0f} / {rw + 200 * ncl + 5500 * ng:8.0f};"
            print(line, flush=True)

# ---------------------------------------------------------------- H: a sub-cluster key that borrows its cluster's spread
if "H" in SECT:
    k = 10; truth = topk_rows(k)
    muC, varC, cntC = st("C"); radC = CEN["C"][2]
    print("\nH. k = 10, unfiltered: sub-cluster keys with no state per sub-cluster beyond its centre and rad. "
          "Share of rows (%) to mean recall .8 / .9 / .95")
    for nm in ("a16", "d16", "a64"):
        mu, var, cnt = st(nm); rad = CEN[nm][2]; h = hits_of(nm, truth)
        par = np.zeros(len(cnt), np.int64); par[LAB[nm]] = rowc
        sC = np.sqrt(varC)[:, par]; rr = (rad / np.maximum(radC[par], 1e-6))[None]
        n0 = 20.0
        rsh = np.sqrt((cnt * rad ** 2 + n0 * radC[par] ** 2) / (cnt + n0))
        psis = (0.5, 1, 1.5, 2, 3, 4)
        fam = {"spread f'=0.4": [spread_key(mu, rad, 0.4)],
               "spread, rad shrunk to the cluster's (n0 = 20), best f'": [spread_key(mu, rsh, f) for f in (0.2, 0.4, 0.6, 0.8, 1.0, 1.5)],
               "mu_s + psi sigma_C(q), best psi": [mu + p * sC for p in psis],
               "mu_s + psi sigma_C(q) rad_s / rad_C, best psi": [mu + p * sC * rr for p in psis],
               "mu_s + psi sigma_C(q) rad~_s / rad_C (shrunk), best psi": [mu + p * sC * (rsh / np.maximum(radC[par], 1e-6))[None] for p in psis],
               "mu_s + psi sigma_s exact, best psi": [mu + p * np.sqrt(var) for p in (2, 2.5, 3, 3.5, 4)]}
        for kn, keys in fam.items():
            best = None
            for i, key in enumerate(keys):
                r, c, _ = curve(key, cnt, h); v = rows_at(r, c, .9)
                if best is None or v < best[0]:
                    best = (v, i, r, c)
            _, i, r, c = best
            print(f"   {nm:4s} {kn:58s} (choice {i}) {fmt(rows_at(r, c, .8))} {fmt(rows_at(r, c, .9))} {fmt(rows_at(r, c, .95))}", flush=True)

# ---------------------------------------------------------------- I: the spread from per-coordinate variances in the group basis
if "I" in SECT:
    V = np.load(I + "V.npy"); n = V.shape[1]
    Xm = np.load(I + "Xs.npy", mmap_mode="r")
    k = 10; truth = topk_rows(k)
    G = V.shape[0]
    rowg = grp[rowc]
    gst = np.searchsorted(rowg, np.arange(G + 1))
    P = np.stack([Q.astype(np.float64) @ V[g].astype(np.float64) for g in range(G)])      # G x nq x n query coordinates
    rs = [r for r in (8, 16, 32, 64) if r < n] + [n]
    print("\nI. k = 10, unfiltered: spread of a unit's scores from its per-coordinate variances in its group's basis, "
          "sigma^2 = sum_{i<r} p_i^2 v_i + (one scalar per unit) x sum_{i>=r} p_i^2 vbar_i (vbar the group's). "
          "Key mu + psi sigma, best psi. Share of rows (%) to mean recall .9, and the correlation of log sigma with the exact one")
    for nm in ("C", "d16", "a64"):
        lab = LAB[nm]; L = lab.max() + 1
        mu, var, cnt = st(nm); h = hits_of(nm, truth)
        Cs = CEN[nm][0]
        vz = np.zeros((L, n)); gl = np.zeros(L, np.int64); gl[lab] = rowg
        for g in range(G):
            i, j = gst[g], gst[g + 1]
            if i == j:
                continue
            inv = np.linalg.inv(V[g].astype(np.float64))
            for a in range(i, j, 50_000):
                b = min(j, a + 50_000)
                Z = (np.asarray(Xm[a:b], np.float32) - Cs[lab[a:b]]) @ inv.T.astype(np.float32)
                o = np.argsort(lab[a:b], kind="stable"); ls = lab[a:b][o]
                st0 = np.r_[0, np.where(np.diff(ls))[0] + 1]
                vz[ls[st0]] += np.add.reduceat((Z * Z)[o].astype(np.float64), st0, axis=0)
        vz /= np.maximum(cnt, 1)[:, None]
        vbar = np.zeros((G, n))
        for g in range(G):
            w = gl == g
            if w.any():
                vbar[g] = (vz[w] * cnt[w, None]).sum(0) / cnt[w].sum()
        best = min(rows_at(*curve(mu + p * np.sqrt(var), cnt, h)[:2], .9) for p in (2, 2.5, 3, 3.5, 4))
        line = f"   {nm:4s} exact sigma: {fmt(best)}%;"
        p2 = P ** 2
        for r in rs:
            sig2 = np.zeros((nq, L))
            for g in range(G):
                w = np.where(gl == g)[0]
                if len(w) == 0:
                    continue
                head = p2[g][:, :r] @ vz[w, :r].T
                if r < n:
                    sc = vz[w, r:].sum(1) / max(vbar[g, r:].sum(), 1e-30)
                    head += (p2[g][:, r:] @ vbar[g, r:])[:, None] * sc[None]
                sig2[:, w] = head
            sg = np.sqrt(np.maximum(sig2, 0))
            best = min(rows_at(*curve(mu + p * sg, cnt, h)[:2], .9) for p in (1, 1.5, 2, 2.5, 3, 3.5, 4))
            ok = (cnt > 1)[None] & (var > 0) & (sg > 0)
            cc = np.corrcoef(np.log(sg[ok]), 0.5 * np.log(var[ok]))[0, 1]
            line += f"  r={r}: {fmt(best)}%, corr {cc:.2f};"
        print(line, flush=True)
    # the group's shape alone: sigma_u(q)^2 = (rad_u / rad_g)^2 [sum_{i<r} lam_i (u_i.q)^2 + lbar (|q|^2 - sum_{i<r} (u_i.q)^2)],
    # (lam_i, u_i) the top r eigenpairs of the group's within-cluster residual covariance, lbar the mean of the rest
    Xf = None
    Cc = CEN["C"][0]
    eig = []
    for g in range(G):
        i, j = gst[g], gst[g + 1]
        Cv = np.zeros((n, n))
        for a in range(i, j, 50_000):
            b = min(j, a + 50_000)
            R = np.asarray(Xm[a:b], np.float32) - Cc[rowc[a:b]]
            Cv += (R.T @ R).astype(np.float64)
        l, u = np.linalg.eigh(Cv / max(j - i, 1))
        eig.append((l[::-1], u[:, ::-1]))
    Qd = Q.astype(np.float64); q2 = (Qd * Qd).sum(1)
    print("   the group's shape alone (top r eigenpairs of the group's residual covariance + isotropic rest), scaled by rad_u^2 / tr: "
          "share of rows (%) to recall .9 with key mu + psi sigma, best psi")
    for nm in ("C", "d16", "a64"):
        lab = LAB[nm]; L = lab.max() + 1
        mu, var, cnt = st(nm); h = hits_of(nm, truth); rad = CEN[nm][2]
        gl = np.zeros(L, np.int64); gl[lab] = rowg
        line = f"   {nm:4s}"
        for r in (0, 4, 16, 64, n):
            sig2 = np.zeros((nq, L))
            for g in range(G):
                w = np.where(gl == g)[0]
                if len(w) == 0:
                    continue
                l, u = eig[g]
                pr = (Qd @ u[:, :r]) ** 2
                s2 = pr @ l[:r] + (l[r:].mean() if r < n else 0.0) * (q2 - pr.sum(1))
                sig2[:, w] = s2[:, None] * (rad[w] ** 2 / l.sum())[None]
            sg = np.sqrt(np.maximum(sig2, 0))
            best = min(rows_at(*curve(mu + p * sg, cnt, h)[:2], .9) for p in (1, 1.5, 2, 2.5, 3, 3.5, 4, 5))
            ok = (cnt > 1)[None] & (var > 0) & (sg > 0)
            cc = np.corrcoef(np.log(sg[ok]), 0.5 * np.log(var[ok]))[0, 1]
            line += f"  r={r}: {fmt(best)}%, corr {cc:.2f};"
        print(line, flush=True)
