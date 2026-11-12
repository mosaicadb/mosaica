# Harness: owner's metric (percentage of the dataset read) for several codes and certificates on the
# 200k subset. usage: python3 h.py TAG [NDIM]   (NDIM = 0 keeps n = 768; otherwise global-PCA truncation)
import numpy as np, time, json, sys, math
ROOT = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
TAG = sys.argv[1]; NDIM = int(sys.argv[2]) if len(sys.argv) > 2 else 0
rng = np.random.default_rng(0); t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = unit(np.concatenate([np.load(ROOT + f"data/e{i:02d}.npy") for i in (0, 5, 10, 15)]))
if NDIM:
    s = X[rng.choice(len(X), 50_000, replace=False)]
    w, V = np.linalg.eigh((s.T @ s).astype(np.float64)); X = unit(X @ V[:, ::-1][:, :NDIM].astype(np.float32))
N, n = X.shape; FULL = 2 * n
K, G, NQ, CH = 200, 16, 100, 8000
TS = [0.95, 0.9, 0.8, 0.7]; DS = [0.0, 0.01, 0.05]
LV = [b for b in (0, 4, 8, 16, 32, 48, 96, 192, 384, 576) if b <= 0.4 * FULL]      # cumulative code bytes per level
MAXBITS = 8 * LV[-1]
qi = np.sort(rng.choice(N, NQ, replace=False)); Q = X[qi]

def kmeans(S, K, iters=12):
    C = S[rng.choice(len(S), K, replace=False)].copy()
    for it in range(iters):
        a = np.concatenate([(S[i:i+50_000] @ C.T).argmax(1) for i in range(0, len(S), 50_000)])
        M = np.zeros_like(C); np.add.at(M, a, S)
        cnt = np.bincount(a, minlength=K); empty = cnt == 0
        M[empty] = S[rng.choice(len(S), int(empty.sum()))]
        C = M / np.linalg.norm(M, axis=1, keepdims=True)
    return C

# ---------- scalar quantisers for a unit Gaussian ----------
STEP = {2: 0.9957, 3: 0.5860, 4: 0.3352, 5: 0.1881, 6: 0.1041, 7: 0.0569, 8: 0.0308}
GT = np.array([1, .3634, .1188, .03744, .01154, .00349, .00104, .0003043, .0000876])
def quant_uni(Z, bits):                         # the baseline: fixed-length uniform, clipped, not nested
    out = np.zeros_like(Z)
    for b in np.unique(bits):
        if b == 0: continue
        cols = np.where(bits == b)[0]; z = Z[:, cols]
        if b == 1: out[:, cols] = np.where(z >= 0, 0.7979, -0.7979)
        else:
            s = STEP[int(b)]; L = 2 ** int(b)
            out[:, cols] = (np.clip(np.floor(z / s), -L // 2, L // 2 - 1) + 0.5) * s
    return out
xs = np.linspace(-9, 9, 360001); pg = np.exp(-xs * xs / 2); pg /= pg.sum()
cp = np.concatenate([[0], np.cumsum(pg)]); cxp = np.concatenate([[0], np.cumsum(xs * pg)])
def cell(lo, hi):                               # mass and centroid of (lo, hi) under the unit Gaussian
    a, b = np.searchsorted(xs, lo), np.searchsorted(xs, hi); m = cp[b] - cp[a]
    return m, (cxp[b] - cxp[a]) / max(m, 1e-300)
def tables(kind):                               # nested fixed-length quantisers, depth 0..8: bounds, centroids, distortion
    T = {0: (np.array([]), np.array([0.0]))}; g = [1.0]; edges = [-np.inf, np.inf]
    for b in range(1, 9):
        if kind == "bitplane": edges = [-np.inf] + list(np.linspace(-3.94, 3.94, 2 ** b + 1)[1:-1]) + [np.inf]
        else:
            new = [edges[0]]
            for lo, hi in zip(edges[:-1], edges[1:]):
                t = cell(lo, hi)[1]
                for _ in range(60): t = 0.5 * (cell(lo, t)[1] + cell(t, hi)[1])
                new += [t, hi]
            edges = new
        mc = [cell(lo, hi) for lo, hi in zip(edges[:-1], edges[1:])]
        cents = np.array([c for _, c in mc]); T[b] = (np.array(edges[1:-1]), cents)
        g.append(1 - sum(m * c * c for m, c in mc))
    return T, np.array(g)
TAB = {k: tables(k) for k in ("bitplane", "tree")}
def quant_tab(Z, bits, T):
    out = np.zeros_like(Z)
    for b in np.unique(bits):
        if b == 0: continue
        cols = np.where(bits == b)[0]; bd, ce = T[int(b)]
        out[:, cols] = ce[np.searchsorted(bd, Z[:, cols])]
    return out
# escape code: depth 1 = sign (1 bit); depth 2 = unary index of |u| on a grid of width W (k+1 bits);
# depth j > 2 = one bit halving the cell. Every cell is bounded from depth 2 on, whatever |u| is.
W = 0.6745; EMAXD = 14
kk = np.arange(64); mc2 = [cell(k * W, (k + 1) * W) for k in kk]
CENT2 = np.array([c if m > 1e-12 else (k + 0.5) * W for k, (m, c) in zip(kk, mc2)])
P2 = np.array([2 * m for m, _ in mc2])
gE = [1.0, 0.3634, 1 - float((P2 * CENT2 ** 2).sum())]
for j in range(3, EMAXD + 1):
    wj = W * 2.0 ** (2 - j); idx = np.floor(np.abs(xs) / wj); gE.append(float((pg * (np.abs(xs) - (idx + 0.5) * wj) ** 2).sum()))
gE = np.array(gE); cE = np.ones(EMAXD + 1); cE[1] = float((P2 * (kk + 1)).sum())      # cE[b] = expected bits of step b -> b+1
log("model distortion: uniform", GT[1:5], "tree", TAB["tree"][1][1:5].round(5), "bitplane", TAB["bitplane"][1][1:5].round(5),
    "escape", gE[1:6].round(5), "bits of escape depth-2 step", round(cE[1], 3))
def recon_esc(U, dep):
    a = np.abs(U); wj = W * np.exp2(2.0 - np.maximum(dep, 2)).astype(np.float32); idx = np.floor(a / wj)
    val = (idx + 0.5) * wj
    m2 = dep == 2; val[m2] = CENT2[np.minimum(idx[m2], 63).astype(int)]
    val[dep == 1] = 0.7979; val[dep == 0] = 0
    return np.sign(U) * val

def steps(lam, Qe, g, cost, total, maxd):       # greedy step order; step = raise one coordinate by one depth
    b = np.zeros(len(lam), int); a = np.sqrt(Qe * lam); sg = np.sqrt(g); seq = []; spent = 0.0
    gain = a * (sg[0] - sg[1]) / cost[0]
    while spent < total and gain.max() > 0:
        i = int(gain.argmax()); seq.append(i); spent += cost[b[i]]; b[i] += 1
        gain[i] = a[i] * (sg[b[i]] - sg[b[i] + 1]) / cost[b[i]] if b[i] < maxd else -1
    return np.array(seq)

# ---------- accounting ----------
class Tracker:
    def __init__(self, name, over0, over, use=None):   # scalar bytes read at level 0 and at each later level; levels used
        self.name = name; self.use = list(range(len(LV))) if use is None else [LV.index(b) for b in use if b in LV]
        self.inc = {l: (over0 if l == 0 else LV[l] - LV[self.use[i - 1]] + over) for i, l in enumerate(self.use)}
        z = lambda: np.zeros((len(TS), len(DS)))
        self.cost = z(); self.viol = z(); self.full = z(); self.und = np.zeros((len(LV), len(TS))); self.hs = np.zeros((len(LV), len(TS)))
        self.A = np.zeros(len(TS)); self.pairs = 0
    def start(self, true):
        self.true = true; self.alive = np.ones((len(TS), len(DS)) + true.shape, bool); self.pairs += true.size
        for ti, t in enumerate(TS): self.A[ti] += (true >= t).sum()
    def level(self, l, est, h):
        if l not in self.inc: return
        ub = est + h; lb = est - h
        for ti, t in enumerate(TS):
            A = self.true >= t; rej = ub < t; self.hs[l, ti] += h[A].sum()
            for di, d in enumerate(DS):
                al = self.alive[ti, di]; self.cost[ti, di] += al.sum() * self.inc[l]
                acc = lb >= t - d
                self.viol[ti, di] += (al & rej & A).sum() + (al & acc & (self.true < t - d - 2e-5)).sum()
                al &= ~(acc | rej)
            self.und[l, ti] += self.alive[ti, 0].sum()
    def finish(self):
        for ti in range(len(TS)):
            for di in range(len(DS)):
                c = self.alive[ti, di].sum(); self.cost[ti, di] += c * FULL; self.full[ti, di] += c
    def out(self):
        return {"pct": (100 * self.cost / (self.pairs * FULL)).tolist(), "viol": self.viol.tolist(), "A": self.A.tolist(), "pairs": self.pairs,
                "und_per_match": (self.und / self.A).tolist(), "halfwidth": (self.hs / self.A).tolist(), "full_reads_per_match": (self.full / self.A[:, None]).tolist()}
# baseline accounting: 4 bytes of scalars per item, charged once (FINDINGS 6.6).
# honest accounting: level 0 reads m.r (2 B) and one norm (1 B); every later level reads its scale (2 B) and one norm (1 B),
# norms rounded up to a grid of ratio 2^(1/16) (one byte), only the weighted bound is available.
# H2: no scale, one norm byte per level. H3: scale rounded to 8 bits (the error is defined against the rounded scale), one norm byte.
S2 = (0, 8, 32, 96, 384); S3 = (0, 4, 16, 48, 192, 576)
TR = {nm: Tracker(nm, 4, 0) for nm in ("uni", "tree", "esc", "esc ideal")}
TR.update({"tree H3": Tracker("", 3, 2), "esc H2": Tracker("", 3, 1), "esc H3": Tracker("", 3, 2), "esc H3 S2": Tracker("", 3, 2, S2),
           "esc H3 S3": Tracker("", 3, 2, S3), "esc H2 S2": Tracker("", 3, 1, S2), "esc H2 S3": Tracker("", 3, 1, S3)})
def roundup(v): return np.exp2(np.ceil(np.log2(np.maximum(v, 1e-30)) * 16) / 16)

Chat = kmeans(X, K); assign = (X @ Chat.T).argmax(1)
Msum = np.zeros((K, n)); np.add.at(Msum, assign, X)
Mean = (Msum / np.maximum(np.bincount(assign, minlength=K), 1)[:, None]).astype(np.float32)
qm = Q @ Mean.T
pq = X[rng.choice(N, 2000, replace=False)]; pk = np.argsort(-(pq @ Chat.T), axis=1)[:, :4]
Mh = Mean / np.linalg.norm(Mean, axis=1, keepdims=True); grp = (Mh @ kmeans(Mh, G, 20).T).argmax(1)
log("N", N, "n", n, "levels", LV, "full", FULL)
SPEC = {}; ESCSTAT = []
for g in range(G):
    ks = np.where(grp == g)[0]
    if len(ks) == 0: continue
    kpos = -np.ones(K, int); kpos[ks] = np.arange(len(ks))
    items = np.where(grp[assign] == g)[0]
    Rs = X[items] - Mean[assign[items]]
    lam, V = np.linalg.eigh((Rs.T @ Rs).astype(np.float64) / len(Rs)); del Rs
    lam = np.maximum(lam[::-1], 1e-12).copy(); V = V[:, ::-1].astype(np.float32).copy(); sig = np.sqrt(lam).astype(np.float32)
    pm = grp[pk] == g
    if pm.sum() > 50:
        rows, cols = np.where(pm); Qe = (((pq[rows] - Mean[pk[rows, cols]]) @ V) ** 2).mean(0).astype(np.float64)
    else: Qe = lam.copy()
    SPEC[g] = {"lam": lam.tolist(), "Qe": Qe.tolist(), "items": int(len(items))}
    QV = Q @ V; MV = Mean[ks] @ V; Dl2 = (QV[None] - MV[:, None]) ** 2; dn2 = Dl2.sum(2).astype(np.float64)
    def weights(errvar):
        Dv = (((errvar + 1e-10) / (Qe + 1e-10)) ** 0.25).astype(np.float32)
        return Dv, (Dl2 * Dv ** 2).sum(2).astype(np.float64)
    sq = {"uni": steps(lam, Qe, GT, np.ones(9), MAXBITS, 8)}
    for k in ("bitplane", "tree"): sq[k] = steps(lam, Qe, TAB[k][1], np.ones(9), MAXBITS, 8)
    se = steps(lam, Qe, gE, cE, 1.5 * MAXBITS, EMAXD); ecost = np.cumsum(cE[[int((se[:i] == c).sum()) for i, c in enumerate(se)]]) if False else None
    dep_at = np.zeros((len(se) + 1, n), np.int8)
    for s_, c_ in enumerate(se): dep_at[s_ + 1] = dep_at[s_]; dep_at[s_ + 1, c_] += 1
    sdep = dep_at[np.arange(1, len(se) + 1), se]                 # depth reached by each step
    expc = np.cumsum(cE[sdep - 1])
    lev = {}
    for l, B in enumerate(LV):
        if l == 0: continue
        for k in ("uni", "bitplane", "tree"):
            gt = GT if k == "uni" else TAB[k][1]; bits = np.bincount(sq[k][:8 * B], minlength=n)
            lev[(k, l)] = (bits,) + weights(lam * gt[bits])
        lev[("esc", l)] = weights(lam * gE[dep_at[int((expc <= 8 * B).sum())]])
    D0, dw20 = weights(lam)
    for c0 in range(0, len(items), CH):
        ii = items[c0:c0 + CH]; x = X[ii]; a = assign[ii]; ap = kpos[a]
        true = x @ Q.T; z = (x - Mean[a]) @ V; mva = MV[ap]; u = z / sig
        base = (qm[:, a].T + (mva * z).sum(1)[:, None]).astype(np.float64)     # q.m + m.r, exact
        k2 = np.floor(np.abs(u) / W).astype(np.int32)
        C = np.ones((len(ii), len(se)), np.int32); s2 = np.where(sdep == 2)[0]; C[:, s2] = k2[:, se[s2]] + 1
        cum = np.cumsum(C, 1)
        Ci = C.astype(np.float32); Ci[:, s2] = -np.log2(P2[np.minimum(k2[:, se[s2]], 63)] + 1e-300); cumi = np.cumsum(Ci, 1); del C, Ci
        ESCSTAT.append((float(np.abs(u).max()), int((np.abs(u) > 3.94).sum()), u.size))
        for tr in TR.values(): tr.start(true)
        en = np.linalg.norm(z, axis=1).astype(np.float64)[:, None]; ew = np.linalg.norm(z / D0, axis=1).astype(np.float64)[:, None]
        h0 = np.minimum(np.sqrt(dn2[ap]) * en, np.sqrt(dw20[ap]) * ew) * 1.0001 + 2e-5
        h0h = np.sqrt(dw20[ap]) * roundup(ew) * 1.0001 + 5e-5
        for nm, tr in TR.items(): tr.level(0, base, h0h if " H" in nm else h0)
        def cert(c, Dv, dw2, scale):
            c = c.astype(np.float32); cn2 = (c * c).sum(1).astype(np.float64) + 1e-30
            gm = (z * c).sum(1) / cn2 if scale == "ls" else np.ones(len(c)) if scale == "one" else np.clip(np.round((z * c).sum(1) / cn2 * 128), 0, 255) / 128
            e = z - gm[:, None].astype(np.float32) * c
            en = np.linalg.norm(e, axis=1).astype(np.float64)[:, None]; ew = np.linalg.norm(e / Dv, axis=1).astype(np.float64)[:, None]
            est = base + gm[:, None] * (c @ QV.T - (c * mva).sum(1)[:, None]).astype(np.float64)
            return est, np.minimum(np.sqrt(dn2[ap]) * en, np.sqrt(dw2[ap]) * ew) * 1.0001 + 2e-5, np.sqrt(dw2[ap]) * roundup(ew) * 1.0001 + 5e-5
        for l in range(1, len(LV)):
            for k in ("uni", "tree"):
                bits, Dv, dw2 = lev[(k, l)]; c = (quant_uni(u, bits) if k == "uni" else quant_tab(u, bits, TAB[k][0])) * sig
                est, hb, _ = cert(c, Dv, dw2, "ls"); TR[k].level(l, est, hb)
                if k == "tree": est, _, hh = cert(c, Dv, dw2, "q8"); TR["tree H3"].level(l, est, hh)
            Dv, dw2 = lev[("esc", l)]
            c = recon_esc(u, dep_at[(cumi <= 8 * LV[l]).sum(1)]) * sig; est, hb, _ = cert(c, Dv, dw2, "ls"); TR["esc ideal"].level(l, est, hb)
            c = recon_esc(u, dep_at[(cum <= 8 * LV[l]).sum(1)]) * sig; est, hb, _ = cert(c, Dv, dw2, "ls"); TR["esc"].level(l, est, hb)
            est, _, hh = cert(c, Dv, dw2, "one")
            for nm in ("esc H2", "esc H2 S2", "esc H2 S3"): TR[nm].level(l, est, hh)
            est, _, hh = cert(c, Dv, dw2, "q8")
            for nm in ("esc H3", "esc H3 S2", "esc H3 S3"): TR[nm].level(l, est, hh)
        for tr in TR.values(): tr.finish()
    log("group", g, "items", len(items), "violations", {nm: int(tr.viol.sum()) for nm, tr in TR.items() if tr.viol.sum()})
    json.dump({"TS": TS, "DS": DS, "LV": LV, "n": n, "N": N, "NQ": NQ, "res": {nm: tr.out() for nm, tr in TR.items()},
               "max_abs_u": max(s[0] for s in ESCSTAT), "overflow_share": sum(s[1] for s in ESCSTAT) / sum(s[2] for s in ESCSTAT)},
              open(f"res2_{TAG}.json", "w"))
json.dump(SPEC, open(f"spec2_{TAG}.json", "w"))
print("levels", LV, "| max |u|", max(s[0] for s in ESCSTAT), "| share of coordinates beyond 3.94 sigma", sum(s[1] for s in ESCSTAT) / sum(s[2] for s in ESCSTAT))
for nm, tr in TR.items():
    o = tr.out()
    print(f"{nm:18s} viol {int(tr.viol.sum())} | % read d=0: " + " ".join(f"{v[0]:.3f}" for v in o["pct"]) + " | d=.01: " + " ".join(f"{v[1]:.3f}" for v in o["pct"])
          + " | d=.05: " + " ".join(f"{v[2]:.3f}" for v in o["pct"]) + " | full reads/match: " + " ".join(f"{v[0]:.3f}" for v in o["full_reads_per_match"]))
print("true matches, % of pairs:", " ".join(f"{100*a/TR['uni'].pairs:.3f}" for a in TR["uni"].A))
