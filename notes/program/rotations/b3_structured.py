"""B3: k rounds H D_k ... H D_1 with fixed inputs built on a subgroup, against the code as built (depth 1).

G = {0..t-1} (a subgroup of the index group under xor), V = G-perp = multiples of t. The inputs are flat indicators.
The first k-1 rounds are 'structured': the signs on the current support equal a character up to a global sign, an
event whose probability is known exactly. After them y and Delta sit on one coset of G with |G| = t, and the last
round is a t-dimensional one-round problem whose error is measured against the n-dimensional spread (claim B3).

Output per configuration: log2 of the probability of the structure, the conditional law of error/sd, and the
resulting lower bound on Pr[|error| > c sd] for the fixed pair.
"""
import sys
import numpy as np
from lib import fwht, built_code, estimate_error

rng = np.random.default_rng(23)
C = 7.8


def chars_table(n):
    k = np.arange(n)
    pc = np.zeros((n, n), dtype=np.int64)
    x = k[:, None] & k[None, :]
    while x.any():
        pc += x & 1
        x >>= 1
    return 1.0 - 2.0 * (pc % 2)


def run(n, t, k, total=200_000, chunk=4000, depth=1, chars=None):
    nv = n // t
    if chars is None:
        chars = chars_table(n)
    types = ["double" if (k - 1 - j) % 2 == 0 else "shared" for j in range(1, k)]
    a0 = np.zeros(n); b0 = np.zeros(n)
    if (k - 1) % 2 == 0:  # start on the same coset of G
        a0[:t] = 1; b0[:t] = chars[1, :t]
    else:  # start on two cosets of V
        a0[0:n:t] = 1; b0[1:n:t] = 1
    a0 /= np.linalg.norm(a0); b0 /= np.linalg.norm(b0)
    assert abs(a0 @ b0) < 1e-12
    cnt = {c: 0 for c in (4.0, 6.0, C, 10.0)}
    ms = 0.0; mx = 0.0; log2p = 0.0
    for it in range(total // chunk):
        a = np.broadcast_to(a0, (chunk, n)).copy()
        b = np.broadcast_to(b0, (chunk, n)).copy()
        lp = 0.0
        for ty in types:
            D = rng.choice((-1.0, 1.0), (chunk, n))
            sa = a[0] != 0
            sz = int(sa.sum())
            c = int(rng.integers(0, n))
            sg = rng.choice((-1.0, 1.0), (chunk, 1))
            D[:, sa] = chars[c][sa][None, :] * sg
            lp += 1 + np.log2(sz) - sz
            if ty == "double":
                sb = b[0] != 0
                assert not (sa & sb).any() and int(sb.sum()) == sz
                target = np.abs(fwht(a[0] * D[0])) > 1e-9
                for c2 in range(n):
                    if (np.abs(fwht(b[0] * np.where(sb, chars[c2], 0.0))) > 1e-9)[target].all():
                        break
                else:
                    raise RuntimeError("no character brings Delta onto the item's coset")
                sg2 = rng.choice((-1.0, 1.0), (chunk, 1))
                D[:, sb] = chars[c2][sb][None, :] * sg2
                lp += 1 - sz
            else:
                assert ((b[0] != 0) == sa).all()
            a = fwht(a * D); b = fwht(b * D)
            a[np.abs(a) < 1e-9] = 0.0; b[np.abs(b) < 1e-9] = 0.0
        # before the last round: one coset of G, flat, orthogonal
        sa = a[0] != 0
        assert int(sa.sum()) == t and ((b[0] != 0) == sa).all(), (int(sa.sum()), int((b[0] != 0).sum()))
        log2p = lp
        D = rng.choice((-1.0, 1.0), (chunk, n))
        z = fwht(a * D); w = fwht(b * D)
        cell = built_code(z, [depth])
        kappa, e, A, tan = estimate_error(z, cell, depth, depth)
        r = (e * w).sum(1) / np.maximum(A / np.sqrt(n - 1), 1e-300)
        r[A < 1e-12] = 0.0
        for c_ in cnt:
            cnt[c_] += int((np.abs(r) > c_).sum())
        ms += float((r * r).sum()); mx = max(mx, float(np.abs(r).max()))
    p = cnt[C] / total
    bound = log2p + (np.log2(p) if p > 0 else -np.inf)
    print(f"n = {n:4d}  t = {t:3d}  |V| = {nv:3d}  k = {k}: log2 P(structure) = {log2p:6.1f}; given it: mean square of error/sd = {ms/total:6.2f} "
          f"[(n-1)/(t-1) = {(n-1)/max(t-1,1):.2f}], max {mx:5.2f}, Pr[>4] {cnt[4.0]/total:.3f}, Pr[>6] {cnt[6.0]/total:.3f}, "
          f"Pr[>{C}] {p:.4f}, Pr[>10] {cnt[10.0]/total:.4f}  =>  Pr[|error| > {C} sd] >= 2^{bound:.1f}")
    sys.stdout.flush()


if __name__ == "__main__":
    ch = {n: chars_table(n) for n in (16, 64, 256)}
    print("one round (no conditioning): y = 1_G/sqrt(t), Delta = chi_1 1_G/sqrt(t)")
    for t in (2, 4, 8, 16, 32, 64, 128):
        run(256, t, 1, chars=ch[256])
    print("two to six rounds")
    for (n, t, k) in [(256, 16, 2), (256, 32, 2), (256, 64, 2), (256, 16, 3), (256, 32, 3), (256, 8, 3),
                      (256, 16, 4), (256, 32, 4), (256, 64, 4), (256, 16, 5), (256, 32, 5), (256, 32, 6),
                      (64, 8, 2), (64, 8, 3), (64, 8, 4), (64, 8, 5), (64, 8, 6), (64, 8, 7), (64, 8, 8), (64, 16, 4), (64, 16, 6),
                      (16, 4, 2), (16, 4, 3), (16, 4, 4), (16, 4, 6), (16, 4, 8)]:
        run(n, t, k, chars=ch[n])
    for (n, t, k) in [(512, 32, 2), (512, 32, 3), (1024, 32, 2), (1024, 64, 2), (1024, 32, 3)]:
        run(n, t, k, total=40_000, chunk=2000)
