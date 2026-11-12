# Small figures quoted in the write-up: pooled thresholds, energy shares by band, level-by-level cost, band products at 0 bytes.
import numpy as np, json
from harness import *
I = load(); X, Z, Mean, grp, V, lam, assign, Qi = (I[k] for k in ("X", "Z", "Mean", "grp", "V", "lam", "assign", "Qi"))
T = np.load("text_q.npy"); thr = np.load("thr_all.npy"); Ttr = np.load("text_train.npy"); Qt = text_profile(I, Ttr); Qmix = ((np.sqrt(Qi) + np.sqrt(Qt)) / 2) ** 2
print("pooled mean thresholds: item", np.round(thr[:100].mean(0), 3), "text", np.round(thr[100:].mean(0), 3))
w = np.bincount(grp[assign], minlength=len(lam)); ed = [0, 8, 32, 64, 128, 256, 512, 768]
band = lambda P: np.round(100 * sum(w[g] * np.add.reduceat(P[g], ed[:-1]) / P[g].sum() for g in range(len(lam))) / w.sum(), 1)
print("energy % by band", ed, "\n item offsets", band(Qi), "\n text offsets", band(Qt), "\n residual    ", band(lam))
def alloc_mse(gam, B):
    b = np.zeros(len(gam), int); val = gam * GT[b]
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - gam * GT[nb]; gain[b >= 8] = -1
        i = int(gain.argmax()); b[i] += 1; val[i] = gam[i] * GT[b[i]]
    return b
print("dims coded at 96 B by MSE allocation, per group: item profile", [int((alloc_mse(lam[g] * Qi[g], 768) > 0).sum()) for g in range(len(lam))],
      "mix profile", [int((alloc_mse(lam[g] * Qmix[g], 768) > 0).sum()) for g in range(len(lam))])
rb = np.load("res2_base.npz"); rs = np.load("res3b.npz"); cls = rs["cls"]; m = cls == "text"; N = int(rb["N"])
Ud = rb["U"][:, :, 0, :len(cls)][:, :, m].mean(2) / N; Us = rs["U_split-mix-opt"][:, :, m].mean(2) / N
for ti in (0, 1):
    print(f"text, answer size {SIZES[ti]}: share of items undecided after det levels {LEVELS}:", np.round(Ud[:, ti], 4).tolist(), "| after split levels 32/48/96/192/384:", np.round(Us[:, ti], 4).tolist())
    parts = [4, 4 * Ud[0, ti], 4 * Ud[1, ti], 8 * Ud[2, ti], 32 * Ud[3, ti], 16 * min(Ud[3, ti], Us[0, ti]), 48 * Us[1, ti], 96 * Us[2, ti], 192 * Us[3, ti], 1536 * Us[4, ti]]
    print("   bytes per item by step (scalars, 4, 8, 16, split 32, 48, 96, 192, 384, full):", np.round(parts, 2).tolist(), "total", round(sum(parts), 1))
rng = np.random.default_rng(3); acc = np.zeros(3); cnt = 0; be = [0, 8, 128, 768]
for j in range(0, len(T), 3):
    s = X @ T[j]; mi = np.sort(rng.choice(np.where(s >= thr[100 + j, 2])[0], 60, replace=False))
    for g in np.unique(grp[assign[mi]]):
        ii = mi[grp[assign[mi]] == g]; z = np.asarray(Z[ii]); D = (T[j][None] - Mean[assign[ii]]) @ V[g]
        acc += (np.sqrt(np.add.reduceat(D * D, be[:-1], axis=1)) * np.sqrt(np.add.reduceat(z * z, be[:-1], axis=1))).sum(0); cnt += len(ii)
print("text, 0 bytes: mean |Delta_b||r_b| for local dims 0-8, 8-128, 128-768:", np.round(acc / cnt, 4).tolist())
