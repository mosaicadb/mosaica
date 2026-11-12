# Export the learned build parameters of the numpy pipeline (combined/idx_<NAME> + the quant final configuration,
# DCODE=fr PROF=Qmix) for the Rust prototype. Nothing here encodes items: encoding happens in Rust.
# usage: python3 export.py NAME OUTDIR
#   writes: meta.json, cstart, grp, mq8 (int8 centres), msc, lam, V, qmix, frb (bits per coord per det level),
#   dl (weights per det level), dt (randomised whitening), ridx (block index sets), rates, tree tables, queries.
import numpy as np, sys, os, json, math
VEC = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/"
sys.path.insert(0, VEC + "combined")
from core import TREE, GTREE

NAME, OUT = sys.argv[1], sys.argv[2]
os.makedirs(OUT, exist_ok=True)
IDX = VEC + f"combined/idx_{NAME}/"
I = dict(np.load(IDX + "index.npz"))
N, n, K, G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"])
DLV = [4, 8, 12, 16, 24, 32]
RLV = [40, 48, 56, 64, 72, 80, 96, 112, 128, 144, 160, 192, 224, 256]
BOUNDS = [0] + [b for b in (96, 224, 480, 992, 2016, 4064) if b <= n - 64] + [n]; J = len(BOUNDS) - 1
NR = len(RLV); EPS, NTARGET = 1e-9, 1e9
L1 = math.log(2 * NTARGET * NR / EPS); CSG = math.sqrt(2 * L1)
MENU = [0, 0.125, 0.25, 0.5, 1, 2, 3, 4, 5, 6, 7, 8]
def Tof(r): return np.inf if r == 0 else (np.pi / (2 * r) - 1 if r < 1 else GTREE[int(r)] / (1 - GTREE[int(r)]))
def bits_of(r, nb): return int(r * nb)
def design(S, nb, budgets, menu, c):            # copied from quant/run2.py
    ri = [0] * len(S); out = []; used = 0
    def h(r):
        T = np.array([Tof(menu[k]) for k in r]); cod = np.array(r) > 0
        return S[~cod].sum() + c * math.sqrt(((S ** 2 * np.where(cod, T, 0)) / (nb - 1))[cod].sum()) if cod.any() else S.sum()
    for B in budgets:
        while True:
            best = None; h0 = h(ri)
            for j in range(len(S)):
                for k2 in range(ri[j] + 1, len(menu)):
                    db = bits_of(menu[k2], nb[j]) - bits_of(menu[ri[j]], nb[j])
                    if db <= 0 or used + db > B: continue
                    r2 = list(ri); r2[j] = k2; gn = (h0 - h(r2)) / db
                    if best is None or gn > best[0]: best = (gn, j, k2, db)
            if best is None or best[0] <= 0: break
            ri[best[1]] = best[2]; used += best[3]
        out.append([menu[k] for k in ri])
    return out

lamA, QmA = I["lam"], I["Qmix"]
FRB = np.zeros((G, len(DLV), n), np.uint8); DL = np.zeros((G, len(DLV), n), np.float64)
DT = np.zeros((G, n), np.float64); RIDX = np.zeros((G, n), np.int32); RATES = np.zeros((G, NR, J), np.float64)
for g in range(G):
    lam, Qd = lamA[g], QmA[g]
    a_ = np.sqrt(Qd * lam); sg = np.sqrt(GTREE); bcur = np.zeros(n, int); gain = a_ * (sg[0] - sg[1]); used = 0
    for l, b in enumerate(DLV):
        while used < 8 * b:
            i = int(gain.argmax())
            if gain[i] <= 0: break
            bcur[i] += 1; used += 1; gain[i] = a_[i] * (sg[bcur[i]] - sg[bcur[i] + 1]) if bcur[i] < 8 else -1
        FRB[g, l] = bcur
        DL[g, l] = ((lam * GTREE[bcur] + 1e-10) / (Qd + 1e-10)) ** 0.25
    lres = lam * GTREE[FRB[g, -1]]
    DT[g] = ((lres + 1e-10) / (Qd + 1e-10)) ** 0.25
    ai = np.sqrt(lres * Qd); order = np.argsort(-ai, kind="stable"); RIDX[g] = order
    idx = [order[BOUNDS[j]:BOUNDS[j + 1]] for j in range(J)]
    Sj = np.array([ai[ix].sum() for ix in idx]); nb = np.array([len(ix) for ix in idx])
    RATES[g] = np.array(design(Sj, nb, [8 * (B - DLV[-1]) for B in RLV], MENU, CSG))
    if g == 0: print("group 0 rates", RATES[g].tolist(), flush=True)

msc = I["msc"].astype(np.float64); Mq = I["Mq"].astype(np.float64)
mq8 = np.round(Mq / msc[:, None]).astype(np.int8)
assert np.abs(mq8 * msc[:, None] - Mq).max() < 1e-6
tree_edges = np.zeros((9, 256), np.float64); tree_cent = np.zeros((9, 256), np.float64)
for b in range(1, 9):
    e, c = TREE[b]; tree_edges[b, :len(e)] = e; tree_cent[b, :len(c)] = c
qcls = (I["qcls"] == "text").astype(np.int8)
text = np.load(VEC + "agent2/text_q.npy").astype(np.float32)
sv = lambda k, a: np.save(OUT + f"/{k}.npy", np.ascontiguousarray(a))
sv("cstart", I["cstart"].astype(np.int64)); sv("grp", I["grp"].astype(np.int64)); sv("mq8", mq8); sv("msc", msc)
sv("lam", lamA.astype(np.float64)); sv("V", I["V"].astype(np.float32)); sv("qmix", QmA.astype(np.float64))
sv("frb", FRB); sv("dl", DL); sv("dt", DT); sv("ridx", RIDX); sv("rates", RATES)
sv("tree_edges", tree_edges); sv("tree_cent", tree_cent); sv("gtree", GTREE.astype(np.float64))
sv("perm", np.load(IDX + "perm.npy").astype(np.int64)); sv("qi", I["qi"].astype(np.int64)); sv("text_q", text)
json.dump(dict(N=N, n=n, K=K, G=G, DLV=DLV, RLV=RLV, BOUNDS=BOUNDS, CSG=CSG, L1=L1, xs=IDX + "Xs.npy"), open(OUT + "/meta.json", "w"))
print("exported", NAME, "N", N, "n", n, "K", K, "G", G, "c", CSG, flush=True)
