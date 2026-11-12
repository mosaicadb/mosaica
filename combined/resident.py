# Stored and resident bytes of the combined design, per item and fixed, at the sample size and extrapolated to N = 1e9
# (clusters of the same size, same G, same anchors). usage: python3 resident.py NAME [LD]
import numpy as np, sys
ROOT = "/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/combined/"
NAME = sys.argv[1]; LD = int(sys.argv[2]) if len(sys.argv) > 2 else 32
I = np.load(ROOT + f"idx_{NAME}/index.npz"); N, n, K, G = int(I["N"]), int(I["n"]), int(I["K"]), int(I["G"]); A = len(I["P"])
FULL = 2 * n; LV = [b for b in (0, 4, 8, 16, 32, 48, 96, 192, 384, 576) if b <= 0.4 * FULL]
BOUNDS = [0] + [b for b in (96, 224, 480, 992, 2016, 4064) if b <= n - 64] + [n]; J = len(BOUNDS) - 1
cum = [("a (sort key, gives m.r)", 2), ("level-0 norm", 1)]; tot = 3
for l in range(1, len(LV)):
    inc = LV[l] - LV[l - 1]
    if LV[l] <= LD: sc = 1
    else: sc = (2 * J if LV[l - 1] == LD else 0) + J       # upper bound: every block changes rate
    tot += inc + sc; cum.append((f"level {LV[l]} B ({'det' if LV[l] <= LD else 'rand'})", tot))
print(f"{NAME}: n {n}, N {N}, K {K}, G {G}, anchors {A}, full vector {FULL} B")
print("cumulative stored bytes per item through each level (codes + scalars):", [(k, v) for k, v in cum])
print("other per-item storage: full vector", FULL, "B (disk); id permutation 4 B (disk, read for returned items)")
per_cluster = n + 4 + 8 + 4 + 4                           # 8-bit centre + scale, start offset, |m|, a-range
for NN, lab in ((N, "sample"), (1e9, "N=1e9")):
    KK = K * NN / N
    fx = {"centres (8-bit) and cluster metadata": KK * per_cluster, "local bases, float32": G * n * n * 4,
          "per-level weights D, float32": G * len(LV) * n * 4, "escape step orders (2 B per step)": G * 1.5 * 8 * LV[-1] * 2,
          "rotation matrices (or regenerate from the public seed)": G * sum((b1 - b0) ** 2 for b0, b1 in zip(BOUNDS[:-1], BOUNDS[1:])) * 4,
          "anchors, 8-bit": A * (n + 4), "support table (hi, lo at 2 B), disk": A * KK * 4}
    print(f"\nfixed structures at {lab}: " + "; ".join(f"{k}: {v / 1e6:.1f} MB ({v / NN:.3f} B/item)" for k, v in fx.items()))
