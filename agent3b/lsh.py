# Randomised bucket certificates (LSH) as a first pass: reads vs extra storage, from the measured cosine histogram
# (exact in expectation for data-oblivious hashes: collision probability depends only on the cosine), plus an
# empirical run with observed vs predicted misses. Verification of candidates is charged at the baseline per-item cost.
import numpy as np, json, sys, time
from scipy.special import ndtr
t0 = time.time()
def log(*a): print(f"[{time.time()-t0:6.0f}s]", *a, flush=True)
DD = sys.argv[1] if len(sys.argv) > 1 else "../data"
FILES = sys.argv[2:] if len(sys.argv) > 2 else ["e00", "e05", "e10", "e15"]
TAG = DD.strip("./").replace("/", "_")
hz = np.load(f"hist_{TAG}.npz"); edges, hist, N, NQ = hz["edges"], hz["hist"].astype(np.float64), int(hz["N"]), int(hz["NQ"])
cen = np.clip((edges[:-1] + edges[1:]) / 2, -0.9995, 0.9995); lo_edge = edges[:-1]
w = hist / (N * NQ); nzb = w > 0
TS = [0.95, 0.9, 0.8, 0.7]
n = int(json.load(open(f"base_{TAG}.json"))["n"]); base = json.load(open(f"base_{TAG}.json"))
NREF = 1e9; ID = 4.0
res = {}
# ---- 1. hyperplane LSH: k-bit keys, L tables ----
p = 1 - np.arccos(cen) / np.pi
p_lo = 1 - np.arccos(np.clip(lo_edge, -1, 1)) / np.pi           # conservative (lower) collision probability inside a bin
for t in TS:
    ch = np.load(f"costhist_{TAG}_{t}.npy") / (N * NQ)           # baseline bytes per item, by cosine bin
    pt = 1 - np.arccos(t) / np.pi
    for delta in (1e-6, 1e-12):
        rows = []
        for k in range(1, 81):
            L = int(np.ceil(np.log(delta) / np.log1p(-pt ** k)))
            pk = p ** k
            entries = L * (w * pk).sum(); pc = 1 - (1 - pk) ** L
            cand = (w * pc).sum(); verify = (ch * pc).sum()
            rows.append((k, L, ID * L, entries, cand, ID * entries + verify + L * 16 / NREF))
        rows = np.array(rows)
        for cap in (64, 256, 1024, 4096, 16384, 1e12):
            ok = rows[rows[:, 2] <= cap]
            if len(ok) == 0: continue
            b = ok[ok[:, 5].argmin()]
            res[f"hyper_t{t}_d{delta}_cap{int(min(cap, 1e9))}"] = {"k": int(b[0]), "L": int(b[1]), "storage_bytes_per_item": b[2], "entries_per_item": b[3],
                "candidates_share": b[4], "read_bytes_per_item": b[5], "read_pct": b[5] / (2 * n) * 100, "baseline_pct": base[str(t)]["baseline_pct"]}
            log("hyper", t, delta, "cap", cap, {a: (round(float(v), 4) if isinstance(v, float) else v) for a, v in res[f"hyper_t{t}_d{delta}_cap{int(min(cap, 1e9))}"].items()})
# ---- 2. idealised Gaussian cap filters: item in filter if g.x >= a, query probes filters with g.q >= b ----
xg, wg = np.polynomial.legendre.leggauss(400)
def P2(a, b, c):                                                # P(X>=a, Y>=b), corr c (array)
    x = a + (xg + 1) * 6.0; ww = wg * 6.0 * np.exp(-x * x / 2) / np.sqrt(2 * np.pi)
    c = np.atleast_1d(c)[:, None]
    return (ww[None] * ndtr(-(b - c * x[None]) / np.sqrt(1 - c * c))).sum(1)
cb = cen[nzb]; wb = w[nzb]
for t in TS:
    ch = (np.load(f"costhist_{TAG}_{t}.npy") / (N * NQ))[nzb]
    for delta in (1e-6, 1e-12):
        Lam = -np.log(delta); rows = []
        for a in np.arange(0.0, 8.01, 0.25):
            for b in np.arange(-1.0, 8.01, 0.25):
                pt = P2(a, b, np.array([t]))[0]
                if pt < 1e-200: continue
                F = Lam / pt; pc_ = P2(a, b, cb)
                stor = F * ndtr(-a); entries = F * (wb * pc_).sum(); pcand = -np.expm1(-F * pc_)
                read = ID * entries + (ch * pcand).sum() + F * ndtr(-b) * 16 / NREF
                rows.append((a, b, ID * stor, entries, (wb * pcand).sum(), read, F))
        rows = np.array(rows)
        for cap in (128, 256, 1024, 4096, 1e12):
            ok = rows[rows[:, 2] <= cap]
            if len(ok) == 0: continue
            r = ok[ok[:, 5].argmin()]
            key = f"cap_t{t}_d{delta}_cap{int(min(cap, 1e9))}"
            res[key] = {"a": r[0], "b": r[1], "storage_bytes_per_item": r[2], "entries_per_item": r[3], "candidates_share": r[4],
                        "read_bytes_per_item": r[5], "read_pct": r[5] / (2 * n) * 100, "filters": r[6], "baseline_pct": base[str(t)]["baseline_pct"]}
            log("capfilter", t, delta, "cap", cap, {a_: round(float(v), 4) for a_, v in res[key].items()})
json.dump(res, open(f"lsh_{TAG}.json", "w"), indent=1)
# ---- 3. empirical hyperplane LSH, k = 12 ----
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = unit(np.concatenate([np.load(f"{DD}/{f}.npy") for f in FILES]))
qi = np.load(f"clusters_{TAG}.npz")["qi"]; Q = X[qi]; true = X @ Q.T
k = 12; LMAX = 164; rng = np.random.default_rng(99)            # the secret seed; queries were fixed before it was drawn
H = rng.standard_normal((n, k * LMAX)).astype(np.float32)
pw = (1 << np.arange(k)).astype(np.uint16)
def keys(Y):
    out = np.empty((len(Y), LMAX), np.uint16)
    for i in range(0, len(Y), 10000):
        out[i:i+10000] = ((Y[i:i+10000] @ H > 0).reshape(-1, LMAX, k) * pw).sum(2)
    return out
KX = keys(X); KQ = keys(Q); log("keys done")
emp = {}
for t, L in ((0.95, 85), (0.9, 164), (0.95, 8), (0.9, 8)):
    obs = np.zeros(3); pred = np.zeros(3); nm = 0
    for q in range(NQ):
        coll = (KX[:, :L] == KQ[q, :L][None]).sum(1); c = np.clip(true[:, q], -1, 1); M = c >= t
        pk = (1 - np.arccos(c) / np.pi) ** k
        obs += [(M & (coll == 0)).sum(), (coll > 0).sum(), coll.sum()]
        pred += [((1 - pk[M]) ** L).sum(), (1 - (1 - pk) ** L).sum(), L * pk.sum()]
        nm += M.sum()
    emp[f"t{t}_L{L}"] = {"k": k, "L": L, "matches": int(nm), "missed_observed": int(obs[0]), "missed_predicted": float(pred[0]),
                         "candidates_observed_share": obs[1] / (N * NQ), "candidates_predicted_share": pred[1] / (N * NQ),
                         "entries_observed_per_item": obs[2] / (N * NQ), "entries_predicted_per_item": pred[2] / (N * NQ)}
    log("empirical", emp[f"t{t}_L{L}"])
res["empirical"] = emp
json.dump(res, open(f"lsh_{TAG}.json", "w"), indent=1)
