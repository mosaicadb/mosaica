# E6: a better sub-1-bit code for a rotated block: pulse-position (index and sign of the largest of M rotated coordinates), against sign bits.
# Same estimator and same error law (error = |y| tan(phi) <w, Delta>, w uniform orthogonal to y) because the law holds for any codebook.
import numpy as np, math
rng = np.random.default_rng(4)
nb = 768
R = np.linalg.qr(rng.standard_normal((nb, nb)))[0].astype(np.float32)
Y = rng.standard_normal((2000, nb)).astype(np.float32) * (np.arange(1, nb + 1, dtype=np.float32) ** -0.5)
D = rng.standard_normal((1000, nb)).astype(np.float32) * (np.arange(1, nb + 1, dtype=np.float32) ** -0.5)
y = Y @ R.T; Dr = D @ R.T; l2 = np.linalg.norm(y, axis=1); dn = np.linalg.norm(Dr, axis=1); true = y @ Dr.T
print("code | bits | mean tan(phi) | rms error/(|y||D|) | max |error| in predicted sd | Shannon-limit tan(phi) at these bits")
def report(name, bits, cy):
    num = (cy * y).sum(1); cosp = num / (np.linalg.norm(cy, axis=1) * l2); A = l2 * np.sqrt(1 - cosp ** 2) / cosp
    err = (l2 * l2 / num)[:, None] * (cy @ Dr.T) - true; sd = A[:, None] * dn[None] / math.sqrt(nb - 1)
    c2 = 1 - 2 ** (-2 * bits / nb)
    print(f"{name:22s} | {bits:4d} | {(A / l2).mean():.3f} | {np.sqrt((err ** 2).mean() / ((l2[:, None] * dn[None]) ** 2).mean()):.4f} | {np.abs(err / sd).max():.2f} | {math.sqrt((1 - c2) / c2):.3f}")
for bits in (96, 240, 480):
    cy = np.zeros_like(y); cy[:, :bits] = np.sign(y[:, :bits]); report("sign bits", bits, cy)
    for M in (16, 64):
        j = int(math.log2(M)) + 1; ng = bits // j
        if ng * M > nb: continue
        blk = y[:, :ng * M].reshape(len(y), ng, M); am = np.abs(blk).argmax(2)
        cy = np.zeros_like(blk); np.put_along_axis(cy, am[:, :, None], np.sign(np.take_along_axis(blk, am[:, :, None], 2)), 2)
        full = np.zeros_like(y); full[:, :ng * M] = cy.reshape(len(y), -1); report(f"pulse position M={M}", ng * j, full)
