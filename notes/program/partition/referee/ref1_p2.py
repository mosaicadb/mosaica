# Referee 1, P2.2 and P2.3: the scaling identity and the quoted figures, recomputed from their stated inputs.
import numpy as np
from scipy.optimize import brentq

rng = np.random.default_rng(3)
ln2 = np.log(2)


def Hstar(gam, w):
    if w >= np.sqrt(gam).sum():
        return 0.0, gam.max()
    lt = brentq(lambda lt: np.sqrt(np.minimum(gam, np.exp(lt))).sum() - w, -300, np.log(gam.max()))
    th = np.exp(lt)
    return 0.5 * np.maximum(np.log(gam / th), 0).sum(), th


print("== P2.2: H*(w; c gamma) = H*(w / sqrt(c); gamma), and the saving against (e/2) lam w ==")
D = 19.0; e = 2 / D; worst = 0; rel = []
for t in range(200):
    n = int(rng.integers(5, 400)); gam = np.exp(rng.normal(0, rng.uniform(0.3, 2.5), n))
    w = rng.uniform(0.05, 0.9) * np.sqrt(gam).sum()
    H1, th = Hstar(gam, w)
    H2, _ = Hstar(gam * 2 ** -e, w)
    H3, _ = Hstar(gam, w * 2 ** (e / 2))
    worst = max(worst, abs(H2 - H3))
    lam_w = w / np.sqrt(th)
    ka = (gam > th).sum()
    assert lam_w >= ka - 1e-9 and lam_w <= n + 1e-9
    rel.append(((H1 - H2) / ln2) / (e / 2 * lam_w))
print("worst |H*(w; 2^-e gamma) - H*(2^(e/2) w; gamma)|:", worst)
print("exact saving over (e/2) lam w: min %.4f median %.4f max %.4f (below 1: H* is convex in ln w)" % (min(rel), np.median(rel), max(rel)))
print("2^(1/19) =", 2 ** (1 / 19), "; 1000^(1/19) =", 1000 ** (1 / 19), "; n/D = 768/19 =", 768 / 19, "bits =", 768 / 19 / 8, "B")

print("-- exponents from the quoted rms residuals --")
print("K 1,000 -> 58,750 at N = 1M (rms 0.3765 -> 0.3034): mean-square exponent", -2 * np.log(0.3034 / 0.3765) / np.log(58.75), "D =", np.log(58.75) / -np.log(0.3034 / 0.3765))
print("K 31 -> 1,000 at 1,000 rows a cell (rms 0.507 -> 0.377): mean-square exponent", -2 * np.log(0.377 / 0.507) / np.log(1000 / 31), "D =", np.log(1000 / 31) / -np.log(0.377 / 0.507))
ks = np.array([1000, 3997, 15880, 58750]); r = np.array([0.3765, 0.3509, 0.3288, 0.3034])
print("step by step at N = 1M, mean-square exponent:", np.round(-2 * np.diff(np.log(r)) / np.diff(np.log(ks)), 3))

print("-- the table: lam w as the slope of nats against ln W, group 12 of the floors' section 7 --")
B = np.array([0, 4, 8, 16, 32, 96]); W = np.array([0.0453, 0.0379, 0.0327, 0.0258, 0.0180, 0.0068])
for i in range(5):
    lw = (B[i + 1] - B[i]) * 8 * ln2 / np.log(W[i] / W[i + 1])
    print(f"{B[i]:3d} to {B[i+1]:3d} B: lam w = {lw:6.1f}; saving per doubling {lw / D:5.2f} bits = {lw / D / 8:4.2f} B")
W2 = np.array([0.0934, 0.0775, 0.0666, 0.0518, 0.0352, 0.0126])
print("group 2 for comparison:", [round((B[i + 1] - B[i]) * 8 * ln2 / np.log(W2[i] / W2[i + 1]), 1) for i in range(5)])

print("-- against REPORT_CLUSTERSIZE: bytes per item, base (1,000 a cell) against P4 (4,000) --")
Nrows = 999_000
for name, b0, b1, e0, e1, fv0, fv1 in [("text 1%", 67.9, 71.6, 955e3, 979e3, 0.0073, 0.0075), ("text 50%", 348.3, 356.8, 998e3, 998e3, 0.1157, 0.1192)]:
    print(f"{name}: per item {b0} -> {b1}: {(b1 - b0) / 2:.2f} B per doubling; per item entering the levels "
          f"{b0 * Nrows / e0:.1f} -> {b1 * Nrows / e1:.1f}: {(b1 * Nrows / e1 - b0 * Nrows / e0) / 2:.2f} B per doubling; "
          f"bytes of f16 vectors in the change (share x 1,536 B): {(fv1 - fv0) * 1536:.1f} of {b1 - b0:.1f} B")

print("== P2.3 ==")
print("by bytes, c_K = 768, u = 1: S* =", [round(768 * ln2 / s) for s in (2.4, 0.8)], "; with 1,000 B of support table added:", [round(1768 * ln2 / s) for s in (2.4, 0.8)])
print("by time: c_K =", (3 + 8) * 1000 / 2, "to", (3 + 17) * 1000 / 2, "byte-equivalents =", (3 + 8) * 500 / 768, "to", (3 + 17) * 500 / 768, "centres; S* =",
      [round(c * ln2 / s) for c, s in ((5500, 2.4), (10000, 0.8))])
rows = {"text 0.01%": ((10.3, 4.1, 1.9), (20.1, 22.5, 24.9), 0.055), "text 1%": ((16.4, 6.9, 3.4), (55.8, 57.2, 58.5), 0.012), "item 1%": ((5.7, 2.8, 1.5), (9.6, 12.6, 15.8), 0.15)}
for k, (pc, pr, eps_claimed) in rows.items():
    a, b = pc[0], pr[0]
    eps2, eps4 = (pr[1] / pr[0] - 1) / 2, (pr[2] / pr[0] - 1) / 4
    eps = np.polyfit([0, 2, 4], np.array(pr) / pr[0], 1)[0]
    p = np.log(pc[0] / pc[2]) / np.log(16)
    S_claim = 1000 * a * ln2 / (b * eps_claimed)
    S_fit = 1000 * a * ln2 / (b * eps)
    S_p = 1000 * (p * a * ln2 / (b * eps)) ** (1 / p)
    # brute force over the three measured sizes
    tot = [pc[i] + pr[i] for i in range(3)]
    print(f"{k}: eps from 1,000->4,000 {eps2:.4f}, from 1,000->16,000 {eps4:.4f}, least squares {eps:.4f} (claimed {eps_claimed}); "
          f"S* with claimed eps {S_claim:,.0f}, with fitted eps {S_fit:,.0f}; per-cell phases fall as S^-{p:.2f}, not S^-1 "
          f"(1/S predicts {a / 4:.1f}, {a / 16:.1f}; measured {pc[1]}, {pc[2]}); S* with that exponent {S_p:,.0f}; "
          f"measured per-cell + per-row ms at 1,000 / 4,000 / 16,000: {tot[0]:.1f} / {tot[1]:.1f} / {tot[2]:.1f}")
print("with N: 1000^0.13 = %.3f, 1000^0.21 = %.3f; 6,500 and 17,000 scaled: %.0f to %.0f; rule at 1B: %d" %
      (1000 ** 0.13, 1000 ** 0.21, 6500 * 1000 ** 0.13, 17000 * 1000 ** 0.21, 2000 * 2 ** max(0, int(np.ceil(np.log2(1e9 / 1e6) / 2 - 0.5)))))
print("== P2.4 / P2.5 / P4 arithmetic spot checks ==")
for nm, v in [("yahoo", (0.58, 0.42)), ("LAION", (0.55, 0.41)), ("glove", (0.64, 0.51))]:
    eq = np.log(v[0] / v[1]) / np.log(64); print(nm, "e_q = %.4f, D = %.1f" % (eq, 2 / eq))
n = 768
print("bases per row at 1M, G = 64:", n * n * 64 / 1e6, "; centres:", n / 2000, "; support table:", 2 * 500 * 500 / 1e6, "; 16 directions:", 16 * n / 2000, "; a basis per 2,000-row cell:", n * n / 2000)
print("n^2 bytes as rows of code at 19 and 68 B:", n * n / 19, n * n / 68)
print("exchange rate: 19 log2(1.1) =", 19 * np.log2(1.1), "doublings")
for Dd in (19, 10, 5):
    print("balanced/optimal distortion, Gaussian, D =", Dd, ":", (1 - 2 / Dd) ** (-Dd / 2) / (1 + 2 / Dd) ** ((Dd + 2) / 2))
print("exp(z(2000)/sqrt(2*76)) =", np.exp(3.435 / np.sqrt(152)))
