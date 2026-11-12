# E5: the sub-1-bit rotated block across block dimension (synthetic Gaussian vectors): k' sign bits of a Haar-rotated n_b-dim block.
# Predicted error sd / (|y| |Delta|) = tan(phi)/sqrt(n_b - 1) ~ sqrt(pi/(2k') - 1/n_b): depends on k', not on n_b.
import numpy as np, math
rng = np.random.default_rng(3)
print("n_b | k' bits | rms error/(|y||D|) | sqrt(pi/2k' - 1/n_b) | max |error| in per-item predicted sd (pairs) | randomised c=8 half-width / deterministic")
for nb in (128, 768, 4096):
    R = np.linalg.qr(rng.standard_normal((nb, nb)))[0].astype(np.float32)
    Y = rng.standard_normal((2000, nb)).astype(np.float32) * (np.arange(1, nb + 1, dtype=np.float32) ** -0.5)   # anisotropic items
    D = rng.standard_normal((1000, nb)).astype(np.float32) * (np.arange(1, nb + 1, dtype=np.float32) ** -0.5)
    y = Y @ R.T; Dr = D @ R.T; l2 = np.linalg.norm(y, axis=1); dn = np.linalg.norm(Dr, axis=1); true = y @ Dr.T
    for kk in (64, 256, 1024):
        if kk > nb: continue
        cy = np.sign(y[:, :kk]); num = np.abs(y[:, :kk]).sum(1); cosp = num / (math.sqrt(kk) * l2); A = l2 * np.sqrt(1 - cosp ** 2) / cosp
        est = (l2 * l2 / num)[:, None] * (cy @ Dr[:, :kk].T); err = est - true
        # the error is A * <w, Delta> with w uniform orthogonal to y: its sd is A * |P_perp Delta| / sqrt(n_b - 1) <= A |Delta| / sqrt(n_b-1)
        sd = A[:, None] * dn[None] / math.sqrt(nb - 1)
        print(f"{nb:5d} | {kk:5d} | {np.sqrt((err ** 2).mean() / ((l2[:, None] * dn[None]) ** 2).mean()):.4f} | {math.sqrt(math.pi / (2 * kk) - 1 / nb):.4f} | "
              f"{np.abs(err / sd).max():.2f} ({err.size}) | {8 * (A / l2).mean() / math.sqrt(nb - 1):.3f}")
