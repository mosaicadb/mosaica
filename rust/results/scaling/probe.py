# Approximate-mode probe counts (pure counting) on one fit, after annideas' sim_probe.py / sim_probe2.py:
#  (a) parents only, ordered by centre score + 0.2 x RMS radius (radius = sqrt(1 - |mean|^2) of the cluster's unit
#      vectors, the definition of sim_probe2.py's radof, used for parents and sub-clusters alike);
#  (b), (c) sub-clusters of about 62 / 250 items: plain k-means inside each parent (8 iterations, seed 1, as
#      prep_sim.py), two-level probing: parents ranked as in (a), the top P opened, only their sub-centres ranked (the
#      same key); for each target the P of fewest items is reported.
# Items scanned at fixed probes reaching mean recall 0.8 / 0.9 / 0.99 of the exact top k (k = 10 / 100 / 1000), image
# (50 scaling item queries) and text (first 100 of text_q) apart; at 0.99 also parents and groups opened.
# The vectors are read from the source file (SRC.npy, f16, row = entity) and normalised, so a fit's index directory
# need only keep perm.npy, cstart.npy and grp.npy.
# usage: probe.py SRC.npy FIT_DIR TRUTH.npy NAME OUTDIR   (TRUTH: entity ids of the exact top 1000 per query)
#        probe.py --truth SRC.npy TRUTH.npy               (exact top 1000 over SRC, as entity ids)
# Environment: PROBE_ROWS (a .npy of SRC rows: entity e is SRC row ROWS[e]), PROBE_ITEMQ (item queries; default the
# scaling item queries).
import os, sys, json, time, numpy as np
D = "/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data"
def unit(a): a = np.asarray(a, np.float32); return a / np.linalg.norm(a, axis=1, keepdims=True)
Q = np.concatenate([unit(np.load(os.environ.get("PROBE_ITEMQ", f"{D}/scale_item_q.npy"))[:50]), unit(np.load(f"{D}/text/text_q.npy")[:100])])
NQ = len(Q); KINDS = {"img": np.arange(50), "txt": np.arange(50, 150)}; KS = [10, 100, 1000]; TG = [0.8, 0.9, 0.99]
t0 = time.time()
class Sub:                                                           # SRC rows by entity (sorted reads)
    def __init__(self, X, rows): self.X, self.rows = X, rows; self.shape = (len(rows), X.shape[1])
    def __len__(self): return len(self.rows)
    def __getitem__(self, k):
        r = self.rows[np.arange(len(self.rows))[k]] if isinstance(k, slice) else self.rows[np.asarray(k)]
        o = np.argsort(r); out = np.empty((len(r), self.X.shape[1]), self.X.dtype); out[o] = self.X[r[o]]; return out
def load_src(p):
    X = np.load(p, mmap_mode="r")
    return Sub(X, np.load(os.environ["PROBE_ROWS"])) if os.environ.get("PROBE_ROWS") else X
if sys.argv[1] == "--truth":
    src, out = sys.argv[2], sys.argv[3]
    X = load_src(src); perm = np.arange(len(X))
    bs = np.full((NQ, 0), -np.inf, np.float32); bi = np.zeros((NQ, 0), np.int64)
    for i in range(0, len(X), 200_000):
        s = Q @ unit(X[i:i + 200_000]).T
        s = np.concatenate([bs, s], 1); ii = np.concatenate([bi, np.broadcast_to(np.arange(i, i + s.shape[1] - bs.shape[1]), (NQ, s.shape[1] - bs.shape[1]))], 1)
        top = np.argpartition(-s, 1000, axis=1)[:, :1000]
        bs = np.take_along_axis(s, top, 1); bi = np.take_along_axis(ii, top, 1)
    o = np.argsort(-bs, 1); np.save(out, perm[np.take_along_axis(bi, o, 1)]); print("truth", out, time.time() - t0); sys.exit()
src, idx, truth_p, name, outdir = sys.argv[1:6]
Xsrc = load_src(src); perm = np.load(f"{idx}/perm.npy")
class Ordered:                                                       # the source rows in the fit's row order, unit
    def __init__(self): self.shape = (len(perm), Xsrc.shape[1])
    def __len__(self): return len(perm)
    def __getitem__(self, sl):
        e = perm[sl]; o = np.argsort(e); out = np.empty((len(e), Xsrc.shape[1]), np.float32); out[o] = unit(Xsrc[e[o]]); return out
X = Ordered(); cs = np.load(f"{idx}/cstart.npy"); grp = np.load(f"{idx}/grp.npy")
N = len(X); K = len(cs) - 1
row_of = np.empty(N, np.int64); row_of[perm] = np.arange(N)
truth = row_of[np.load(truth_p)]                                    # rows of the exact top 1000
cl = np.repeat(np.arange(K), np.diff(cs))
C = np.zeros((K, X.shape[1]))
rng = np.random.default_rng(1)
def gsum(a, Xc, k):                                                  # per-label sums of rows, and counts
    o = np.argsort(a, kind="stable"); cnt = np.bincount(a, minlength=k); sums = np.zeros((k, Xc.shape[1]), Xc.dtype)
    nz = np.nonzero(cnt)[0]; st = np.r_[0, np.cumsum(cnt)][nz]
    sums[nz] = np.add.reduceat(Xc[o], st, axis=0); return sums, cnt
def km(Xc, k, it=8):
    n = len(Xc)
    if n <= k: return np.arange(n) % k
    c = Xc[rng.choice(n, k, replace=False)].copy()
    for _ in range(it):
        a = (-2 * Xc @ c.T + (c * c).sum(1)[None]).argmin(1)
        sums, cnt = gsum(a, Xc, k)
        empty = cnt == 0
        c = np.where(empty[:, None], Xc[rng.integers(n, size=k)], sums / np.maximum(cnt, 1)[:, None])
    return (-2 * Xc @ c.T + (c * c).sum(1)[None]).argmin(1)
subs = {62: [], 250: []}; subC = {62: [], 250: []}; subn = {62: [], 250: []}; subpar = {62: [], 250: []}
for c in range(K):
    Xc = np.asarray(X[cs[c]:cs[c + 1]], np.float32); C[c] = Xc.mean(0, dtype=np.float64)
    for sz in (62, 250):
        s = max(1, int(round(len(Xc) / sz))); a = km(Xc, s)
        cnt = np.bincount(a, minlength=s); keep = np.nonzero(cnt)[0]; ren = -np.ones(s, np.int64); ren[keep] = np.arange(len(keep))
        a = ren[a]; base = sum(len(x) for x in subn[sz])
        subs[sz].append(base + a)
        sums, _ = gsum(a, Xc.astype(np.float64), len(keep))
        subC[sz].append(sums / cnt[keep][:, None]); subn[sz].append(cnt[keep]); subpar[sz].append(np.full(len(keep), c))
print("centres and sub-clusters", time.time() - t0, flush=True)
radof = lambda C: np.sqrt(np.maximum(1 - (C * C).sum(1), 0))
qi = np.arange(NQ)[:, None]
def ranks(key):
    order = np.argsort(-key, 1); rank = np.empty_like(order); rank[qi, order] = np.arange(key.shape[1])[None]; return order, rank
def first_p(trk, k, tg, qs):                                         # fewest probes whose mean recall reaches tg
    h = np.bincount(trk[qs, :k].ravel(), minlength=trk.max() + 2).cumsum() / trk[qs, :k].size
    return int(np.searchsorted(h, tg - 1e-12) + 1) if h[-1] >= tg - 1e-12 else None
res = {"name": name, "N": int(N), "K": int(K), "G": int(grp.max() + 1), "a": {}, "b": {}, "c": {}}
key_p = (C @ Q.T).T + 0.2 * radof(C)[None]
order_p, rank_p = ranks(key_p)
cum_p = np.cumsum(np.diff(cs)[order_p], 1); trk_p = rank_p[qi, cl[truth]]
for kind, qs in KINDS.items():
    for k in KS:
        for tg in TG:
            p = first_p(trk_p, k, tg, qs); r = dict(items=float(cum_p[qs, p - 1].mean()), parents=p)
            if tg == 0.99: r["groups"] = float(np.mean([len(np.unique(grp[order_p[q, :p]])) for q in qs]))
            res["a"][f"{kind} k{k} r{tg}"] = r
for sch, sz in (("b", 62), ("c", 250)):
    sub = np.concatenate(subs[sz]); Cs = np.concatenate(subC[sz]); ns = np.concatenate(subn[sz]); par = np.concatenate(subpar[sz])
    sub_of_row = np.empty(N, np.int64); sub_of_row[:] = sub
    key_s = (Cs @ Q.T).T + 0.2 * radof(Cs)[None]
    nsub_par = np.bincount(par, minlength=K)
    res[sch]["subs"] = int(len(ns))
    best = {}
    for P in [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024]:
        if P > K: break
        ok = rank_p[:, par] < P
        order_s, rank_s = ranks(np.where(ok, key_s, -np.inf))
        cum_s = np.cumsum(ns[order_s], 1)
        ts = sub_of_row[truth]; trk = np.where(np.take_along_axis(ok, ts, 1), rank_s[qi, ts], 10 ** 9)
        reach = ok.sum(1)
        scored = float(K + nsub_par[order_p[:, :P]].sum(1).mean())
        for kind, qs in KINDS.items():
            for k in KS:
                for tg in TG:
                    rmax = int(reach[qs].max())
                    t = np.minimum(trk[qs, :k], rmax).ravel()           # unreachable items count at rank rmax (never found)
                    h = np.bincount(t, minlength=rmax + 1)[:rmax].cumsum() / t.size
                    if not len(h) or h[-1] < tg - 1e-12: continue
                    p = int(np.argmax(h >= tg - 1e-12) + 1)
                    it = float(cum_s[qs, np.minimum(p, reach[qs]) - 1].mean())
                    kk = f"{kind} k{k} r{tg}"
                    if kk not in best or it < best[kk]["items"]:
                        r = dict(items=it, P=P, sub_probes=p, centres_scored=scored)
                        if tg == 0.99:
                            pp = [np.unique(par[order_s[q, :min(p, reach[q])]]) for q in qs]
                            r["parents"] = float(np.mean([len(x) for x in pp])); r["groups"] = float(np.mean([len(np.unique(grp[x])) for x in pp]))
                        best[kk] = r
    res[sch].update(best)
json.dump(res, open(f"{outdir}/probe_{name}.json", "w"), indent=1)
print(name, "done", time.time() - t0, flush=True)
