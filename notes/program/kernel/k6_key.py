# Referee check 6: m.r from the 16-bit key. The build stores round(clamp(x . mhat, -1, 1) * 32767) (mod.rs 1589); the
# query takes mr = mn32 * (key / 32767) - mn32^2 in f32 (q.rs 1286-1290) and the certificate allows
# |alpha| * mn * DA + 4e-6 for it, DA = 0.5/32767 + 1e-7 (q.rs 574). Two questions:
#  (a) the f32 evaluation against the exact mn * a_q - mn^2: is 1e-7 * mn enough, and if not what must 4e-6 absorb;
#  (b) the clamp: a row of f16-rounded norm above 1 lying along its centre has x . mhat > 1, and the key loses the excess.
import numpy as np

rng = np.random.default_rng(6)
DA = 0.5 / 32767.0 + 1e-7
f32 = np.float32

# (a) f32 evaluation of mr over every key and many centre norms
worst = 0.0
for mn in np.concatenate([rng.uniform(0.2, 1.0, 400), [1.0, 0.99999994, 0.5, 0.75]]):
    mn32 = f32(mn)
    keys = np.arange(-32767, 32768)
    a32 = (keys.astype(f32) / f32(32767.0)).astype(f32)
    mr32 = ((mn32 * a32).astype(f32) - (mn32 * mn32).astype(f32)).astype(f32)
    exact = mn * (keys / 32767.0) - mn * mn           # f64 on the f64 centre norm (c.mn)
    worst = max(worst, float(np.max(np.abs(mr32.astype(np.float64) - exact)) / mn))
print(f"(a) largest |mr_f32 - (mn a_q - mn^2)| / mn over all keys: {worst:.3e}; DA's float share is 1.0e-07")
print(f"    so 4e-6 must absorb |alpha| mn * {max(worst - 1e-7, 0):.2e}: enough while |alpha| mn <= {4e-6 / max(worst - 1e-7, 1e-30):.0f} (less what else 4e-6 covers: 3e-8 |q.m|)")


# (b) rows that sit on their cluster's centre: the cluster's mean is the row (a cluster of one row or of duplicates),
# stored int8 with one scale as the fit stores centres
def trial(n, kind, m):
    out = []
    for _ in range(m):
        if kind == "dense":
            x = rng.normal(size=n)
        elif kind == "spiky":      # a few large coordinates
            x = rng.normal(size=n) * (rng.random(n) < 8.0 / n)
            if not x.any():
                x[0] = 1.0
        x /= np.linalg.norm(x)
        x16 = x.astype(np.float16).astype(np.float64)
        sc = np.abs(x16).max() / 127.0
        cen = (np.round(x16 / sc) * sc).astype(f32).astype(np.float64)       # int8 times a scale, as f32
        mn = np.linalg.norm(cen)
        a = float(x16 @ (cen / mn))
        key = int(np.round(np.clip(a, -1.0, 1.0) * 32767.0))
        aq = key / 32767.0
        out.append((np.linalg.norm(x16) - 1.0, a - 1.0, abs(a - aq), mn))
    return np.array(out)


print("(b) rows equal to their cluster's mean (centre int8 with one scale): norm excess, x.mhat - 1, key error |a - a_q|")
print("    allowance: DA = %.3e per unit of |alpha| mn, plus 4e-6 absolute" % DA)
for n, kind, m in ((768, "dense", 4000), (256, "dense", 4000), (100, "dense", 4000), (32, "dense", 4000), (768, "spiky", 4000), (100, "spiky", 4000)):
    r = trial(n, kind, m)
    over = r[:, 2] > DA
    over2 = r[:, 2] * r[:, 3] > DA * r[:, 3] + 4e-6        # alpha = 1: beyond the whole fixed-term allowance for the key
    print(f"    n={n:4d} {kind:5s}: sd of |x|-1 {r[:, 0].std():.2e}; largest x.mhat-1 {r[:, 1].max():+.2e}; key error > DA in {over.mean() * 100:5.2f}% "
          f"(largest {r[:, 2].max():.2e}); beyond DA + 4e-6/mn at alpha = 1 in {over2.mean() * 100:5.2f}%")
