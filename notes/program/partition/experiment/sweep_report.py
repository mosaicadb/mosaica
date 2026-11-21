# Tables and fitted slopes for the sweep (item 7), from work/sweep/*.json.
import json, os, numpy as np
D = "work/sweep/"
KF = [50, 100, 200, 400, 800]
KS = [50, 100, 200]
F = {K: json.load(open(f"{D}sweep-full-K{K}.json")) for K in KF if os.path.exists(f"{D}sweep-full-K{K}.json")}
S = {K: json.load(open(f"{D}sweep-sub-K{K}.json")) for K in KS if os.path.exists(f"{D}sweep-sub-K{K}.json")}
SEED = json.load(open(f"{D}seed_spectra.json"))
KINDS = ["text", "species", "item"]
FRS = ["0.01%", "0.1%", "1%"]


def row(d, nm):
    return (f"| {nm} | {d['K']} | {d['pool_rows']} | {d['passes']} | {d['pool_rows_per_cell_min']} | {d['msq_resid_pool']:.5f} | "
            f"{d['Si_S1d_4']:.5f} | {d['Si_S1d_16']:.5f} | {d['Si_S1d_32']:.5f} | {d['Si_S1b_0.03']:.2f} | {d['Si_S1b_0.06']:.2f} | {d['Si_S1b_0.12']:.2f} | "
            f"{d['Sm_S1d_16']:.5f} | {d['Sm_S1b_0.06']:.2f} | {d['st_S23_text_0.01%']:.4f} | {d['st_S23_species_1%']:.4f} | {d['st_S23_item_0.01%']:.4f} | "
            f"{d['st_S23_item_1%']:.4f} | {d['st_S4a_mean_text']:.4f} | {d['st_S4a_mean_item']:.4f} | {d['st_viol_S2'] + d['st_viol_S3']} |")


print("| series | K | pool rows | Lloyd passes to < 0.1% | smallest cell (pool rows) | mean sq. residual (pool) | S1d 4 B | S1d 16 B | S1d 32 B | S1b .03 | S1b .06 | S1b .12 | S1d 16 B (Sm) | S1b .06 (Sm) | S2+3 text .01% | S2+3 species 1% | S2+3 item .01% | S2+3 item 1% | S4a text | S4a item | violations |")
print("|---|" + "---|" * 20)
for K, d in F.items():
    print(row(d, "150k"))
for K, d in S.items():
    print(row(d, "37.5k"))


def slope(x, y):
    x = np.asarray(x, float); y = np.asarray(y, float)
    return float(np.polyfit(x, y, 1)[0])


print("\nS1b against log2 K (bytes per doubling of K, least squares; negative is a fall):\n")
print("| series | Ks | w* = 0.03 | w* = 0.06 | w* = 0.12 | S1d 16 B, relative change per doubling | S1b .06 with Sm |")
print("|---|---|---|---|---|---|---|")
for nm, ser in (("150k", F), ("37.5k", S)):
    Ks = sorted(ser)
    if len(Ks) < 2:
        continue
    lk = np.log2(Ks)
    sl = [slope(lk, [ser[K][f"Si_S1b_{w}"] for K in Ks]) for w in (0.03, 0.06, 0.12)]
    s16 = slope(lk, np.log([ser[K]["Si_S1d_16"] for K in Ks]))
    sm = slope(lk, [ser[K]["Sm_S1b_0.06"] for K in Ks])
    print(f"| {nm} | {Ks} | {sl[0]:+.2f} | {sl[1]:+.2f} | {sl[2]:+.2f} | {np.expm1(s16) * 100:+.2f}% | {sm:+.2f} |")

print("\nModel U (a): slope of ln γᵢ in ln K, 150k series, least squares over K = " + str(sorted(F)) + ":\n")
Ks = sorted(F)
G = np.array([F[K]["gamma"] for K in Ks])
lg = np.log(G)
sl = np.array([slope(np.log(Ks), lg[:, i]) for i in range(G.shape[1])])
print("| coordinates i | mean slope | median slope | smallest | largest |")
print("|---|---|---|---|---|")
for lo, hi in ((1, 20), (21, 100), (101, 300), (1, 300), (301, 768)):
    s = sl[lo - 1:hi]
    print(f"| {lo} to {hi} | {s.mean():+.4f} | {np.median(s):+.4f} | {s.min():+.4f} | {s.max():+.4f} |")
a, b = sl[:20].mean(), sl[100:300].mean()
print(f"\nRatio of mean slopes, i ≤ 20 over 100 < i ≤ 300: {a / b:.2f}")
if len(S) >= 2:
    Kss = sorted(S)
    lgs = np.log(np.array([S[K]["gamma"] for K in Kss]))
    sls = np.array([slope(np.log(Kss), lgs[:, i]) for i in range(lgs.shape[1])])
    print(f"Same at 37.5k over K = {Kss}: i ≤ 20 {sls[:20].mean():+.4f}, 100 < i ≤ 300 {sls[100:300].mean():+.4f}, i ≤ 300 {sls[:300].mean():+.4f}")
    Kc = [K for K in Kss if K in F]
    lgf = np.log(np.array([F[K]["gamma"] for K in Kc]))
    slf = np.array([slope(np.log(Kc), lgf[:, i]) for i in range(lgf.shape[1])])
    print(f"150k over the same K = {Kc}: i ≤ 20 {slf[:20].mean():+.4f}, 100 < i ≤ 300 {slf[100:300].mean():+.4f}, i ≤ 300 {slf[:300].mean():+.4f}")

print("\nModel U (b): ln γᵢ at equal K, 150k against 37.5k, against one doubling of K and the seed spread (mean over i ≤ 300 of |Δ ln γᵢ|):\n")
print("| K | 150k − 37.5k, mean Δ ln γ (signed) | mean |Δ| | one doubling at 150k, mean |ln γ(2K) − ln γ(K)| | seed spread at K = 200 (largest pairwise mean |Δ| among A1–A3) |")
print("|---|---|---|---|---|")
sd = [np.log(np.array(SEED[f"A{s}"]["gamma"])) for s in (1, 2, 3)]
ss = max(np.abs(sd[i][:300] - sd[j][:300]).mean() for i in range(3) for j in range(i + 1, 3))
for K in sorted(S):
    if K not in F:
        continue
    d = np.log(F[K]["gamma"])[:300] - np.log(S[K]["gamma"])[:300]
    dbl = np.abs(np.log(F[2 * K]["gamma"])[:300] - np.log(F[K]["gamma"])[:300]).mean() if 2 * K in F else float("nan")
    print(f"| {K} | {d.mean():+.4f} | {np.abs(d).mean():.4f} | {dbl:.4f} | {ss:.4f} |")

print("\nAssumption (d): a cell's own dual spectrum (row-weighted mean over cells, sorted) over the group's (W_P), at i:\n")
idx = [1, 2, 5, 10, 20, 50, 100, 200, 300, 500]
print("| series | K | " + " | ".join(f"i={i}" for i in idx) + " | Σ own / Σ group |")
print("|---|---|" + "---|" * (len(idx) + 1))
for nm, ser in (("150k", F), ("37.5k", S)):
    for K in sorted(ser):
        g = np.array(ser[K]["gamma"]); o = np.array(ser[K]["own_mean_spectrum"])
        print(f"| {nm} | {K} | " + " | ".join(f"{o[i - 1] / g[i - 1]:.3f}" for i in idx) + f" | {o.sum() / g.sum():.3f} |")

print("\nBytes per entering item, (S2+3) × (S1b at 0.06), Si:\n")
print("| series | K | " + " | ".join(f"{k} {f}" for k in KINDS for f in FRS) + " |")
print("|---|---|" + "---|" * 9)
for nm, ser in (("150k", F), ("37.5k", S)):
    for K in sorted(ser):
        d = ser[K]
        print(f"| {nm} | {K} | " + " | ".join(f"{d[f'st_S23_{k}_{f}'] * d['Si_S1b_0.06']:.2f}" for k in KINDS for f in FRS) + " |")
