"""Referee rebuild of B1 and of the B3/B4 lower bounds, written without the theorist's lib.py."""
import numpy as np
try:
    from scipy import special, optimize
    HAVE_SCIPY = True
except Exception:
    HAVE_SCIPY = False

rng = np.random.default_rng(7001)
LAMS = (2.0, 2.3, 2.6, 3.0, 3.5, 4.2)


def hadamard(n):
    k = np.arange(n)
    x = k[:, None] & k[None, :]
    pc = np.zeros((n, n), dtype=np.int64)
    while x.any():
        pc += x & 1
        x >>= 1
    return (1.0 - 2.0 * (pc % 2)) / np.sqrt(n)


def code(Z, depth):
    """Decoded values of the built code at one depth per position (depth an int or an array over positions)."""
    m, n = Z.shape
    depth = np.broadcast_to(np.asarray(depth), (n,))
    half = 2.0 ** (depth - 1)
    yn2 = (Z * Z).sum(1)
    rms = np.sqrt(yn2 / n)
    best = np.full(m, np.inf)
    out = np.zeros_like(Z)
    for lam in LAMS:
        cell = np.clip(np.floor(Z / (lam * rms[:, None]) * half) + half, 0, 2 * half - 1)
        val = (cell + 0.5) / half - 1.0
        dot = (val * Z).sum(1)
        cn = (val * val).sum(1)
        obj = np.log(np.maximum(cn * yn2 - dot * dot, 1e-300) / np.maximum(dot * dot, 1e-300))
        upd = obj < best
        best[upd] = obj[upd]
        out[upd] = val[upd]
    return out


def err_vec(Z, depth):
    O = code(Z, depth)
    yn2 = (Z * Z).sum(1)
    kappa = yn2 / (O * Z).sum(1)
    e = kappa[:, None] * O - Z
    return e, np.linalg.norm(e, axis=1)


def sphere_c(d, delta):
    f = lambda c: 0.5 * special.betainc((d - 1) / 2, 0.5, 1 - c * c / d) - delta
    return optimize.brentq(f, 1.0, np.sqrt(d) * 0.999, xtol=1e-12)


print("== B1: R = H D, y = e_a + e_b ==")
for n in (64, 256):
    H = hadamard(n)
    for (a, b) in ((0, 1), (1, 2), (5, 44)):
        s = a ^ b
        y = np.zeros(n); y[a] = 1; y[b] = 1
        for depth in (1, 2, 3):
            m = 2000
            D = rng.choice((-1.0, 1.0), (m, n))
            Z = (D * y) @ H  # H symmetric
            z_tamper = np.abs(Z) < 1e-12
            Z[z_tamper] = 0.0
            e, A = err_vec(Z, depth)
            E = (e @ H) * D
            pred = np.zeros((m, n))
            pred[:, 0] += A / np.sqrt(2) * D[:, 0]
            pred[:, s] += -A / np.sqrt(2) * D[:, a] * D[:, b] * D[:, s]
            dev = np.abs(E - pred).max()
            tan = A / np.sqrt(2)
            sd = A / np.sqrt(n - 1)
            if a == 0:
                q = np.zeros(n); q[0] = 1; q[b] = -1; q /= np.sqrt(2)
            else:
                q = np.zeros(n); q[0] = 1; q[s] = 1; q /= np.sqrt(2)
            r = (E @ q) / sd
            qs = np.zeros(n); qs[s] = 1
            qs_perp = qs - (qs @ y) / 2 * y
            rs = (E @ qs) / (sd * max(np.linalg.norm(qs_perp), 1e-300))
            print(f" n={n} a,b={a},{b} depth {depth}: max |E - formula| {dev:.1e}; tan phi {tan[0]:.4f} (all equal: {np.ptp(tan) < 1e-12}); "
                  f"error/sd values {np.unique(np.round(r, 3))}, freq of +max {np.mean(r > 1):.3f}, -max {np.mean(r < -1):.3f}; "
                  f"query e_s alone |error|/(sd |Delta_perp|) {np.unique(np.round(np.abs(rs), 3))}")
    # mixed depths within the block: does the failure need a uniform depth?
    depth = np.where(np.arange(n) % 2 == 0, 1, 2)
    y = np.zeros(n); y[1] = 1; y[2] = 1
    D = rng.choice((-1.0, 1.0), (2000, n))
    Z = (D * y) @ H
    Z[np.abs(Z) < 1e-12] = 0.0
    e, A = err_vec(Z, depth)
    E = (e @ H) * D
    r0 = E[:, 0] / (A / np.sqrt(n - 1))
    print(f" n={n} mixed depths 1,2 alternating, y = e1 + e2, query e0: |error|/sd min {np.abs(r0).min():.2f} max {np.abs(r0).max():.2f}")

print()
print("== B3b: the t-dimensional one-round problem, both zero rules ==")


def xt_samples(t, total, exhaustive=False):
    """X_t for item 1/sqrt(t), query chi_1/sqrt(t), depth-1 code. Returns (X with zeros coded up, X with zeros
    contributing nothing), each standardised by A/sqrt(t-1)."""
    Ht = hadamard(t)
    chi = 1.0 - 2.0 * (np.arange(t) & 1)
    outs_up = []; outs_no = []
    if exhaustive:
        idx = np.arange(2 ** t, dtype=np.int64)
        batches = np.array_split(idx, max(1, len(idx) // 65536))
    else:
        batches = [None] * (total // 100000)
    for bt in batches:
        if bt is None:
            eps = rng.choice((-1.0, 1.0), (100000, t))
        else:
            eps = 1.0 - 2.0 * ((bt[:, None] >> np.arange(t)[None, :]) & 1)
        zeta = np.round((eps @ Ht) * np.sqrt(t)) / t  # exact multiples of 1/t
        xi = ((eps * chi) @ Ht) / np.sqrt(t)
        o = np.where(zeta >= 0, 0.5, -0.5)
        kappa = (zeta * zeta).sum(1) / (o * zeta).sum(1)
        e = kappa[:, None] * o - zeta
        A = np.linalg.norm(e, axis=1)
        up = (e * xi).sum(1)
        no = (np.where(zeta != 0, e, 0.0) * xi).sum(1)
        outs_up.append(up / (A / np.sqrt(t - 1)))
        outs_no.append(no / (A / np.sqrt(t - 1)))
    return np.concatenate(outs_up), np.concatenate(outs_no)


cache = {}
def xt(t):
    if t not in cache:
        cache[t] = xt_samples(t, 6_000_000, exhaustive=(t <= 16))
    return cache[t]


for t in (4, 8, 16, 32, 64):
    up, no = xt(t)
    print(f" t={t:3d} ({'exhaustive' if t <= 16 else '6e6 draws'}): mean square up {np.mean(up**2):.3f}, none {np.mean(no**2):.3f}; "
          f"max |X| up {np.abs(up).max():.3f}, none {np.abs(no).max():.3f}")

print()
print("== B4 rebuilt: log2 lower bound = log2 P(structure) + log2[(t/n) P_up + (1 - t/n) P_none], k >= 2 ==")
if HAVE_SCIPY:
    cs = {n: sphere_c(n - 1, 2.5e-19) for n in (256, 512, 1024)}
    print(" one-coordinate sphere quantile at 2.5e-19:", {n: round(c, 3) for n, c in cs.items()})
else:
    cs = {256: 8.282, 512: 8.589, 1024: 8.748}
for (n, t, k) in [(256, 64, 2), (256, 32, 2), (256, 16, 2), (256, 16, 3), (256, 32, 3), (256, 32, 4), (256, 16, 4),
                  (256, 64, 4), (256, 32, 5), (512, 64, 2), (512, 32, 2), (512, 32, 3), (512, 64, 3), (512, 32, 4), (1024, 64, 2), (1024, 32, 2), (1024, 64, 3)]:
    V = n // t
    pV = (1 + np.log2(V) - V) + (1 - V)
    pG = 1 + np.log2(t) - t
    lp = pV * (k // 2) + pG * ((k - 1) // 2)
    up, no = xt(t)
    line = f" n={n:5d} t={t:3d} |V|={V:3d} k={k}: log2 P(structure) {lp:7.1f};"
    for label, c in (("c=7.8", 7.8), (f"own c={cs[n]:.3f}", cs[n]), (f"1.2 x own c", 1.2 * cs[n])):
        thr = c * np.sqrt((t - 1) / (n - 1))
        p_up = np.mean(np.abs(up) > thr); p_no = np.mean(np.abs(no) > thr)
        mix = (t / n) * p_up + (1 - t / n) * p_no
        n_no = int(np.sum(np.abs(no) > thr)); n_up = int(np.sum(np.abs(up) > thr))
        b = lp + (np.log2(mix) if mix > 0 else -np.inf)
        line += f" {label}: P_up {p_up:.2e} ({n_up}) P_none {p_no:.2e} ({n_no}) -> 2^{b:.1f};"
    print(line)

print()
print("== full n-dimensional check of the reduction, n = 256, k = 2, start on two cosets of V ==")
n = 256
H = hadamard(n)
for t in (16, 32):
    V = n // t
    m = 200_000
    res = []
    for _ in range(m // 20000):
        mm = 20000
        c1 = rng.integers(0, n, mm)
        k_idx = np.arange(n)
        # after the structured round: a'' = s1 1_{c1+G}/sqrt(t), b'' = s2 chi_1 1_{c1+G}/sqrt(t)  (G = {0..t-1})
        ingrp = ((k_idx[None, :] ^ c1[:, None]) < t)
        a2 = ingrp / np.sqrt(t)
        b2 = ingrp * (1.0 - 2.0 * (k_idx[None, :] & 1)) / np.sqrt(t)
        D = rng.choice((-1.0, 1.0), (mm, n))
        Z = (a2 * D) @ H; W = (b2 * D) @ H
        Z[np.abs(Z) < 1e-12] = 0.0
        e, A = err_vec(Z, 1)
        res.append((e * W).sum(1) / (A / np.sqrt(n - 1)))
    r = np.concatenate(res)
    up, no = xt(t)
    f = np.sqrt((n - 1) / (t - 1))
    print(f" t={t}: full pipeline mean square {np.mean(r*r):.3f}; prediction {(t/n)*np.mean((f*up)**2) + (1-t/n)*np.mean((f*no)**2):.3f}")
    for c in (4.0, 6.0, 7.8, 8.282):
        pred = (t / n) * np.mean(np.abs(f * up) > c) + (1 - t / n) * np.mean(np.abs(f * no) > c)
        print(f"    Pr[|error| > {c} sd]: full {np.mean(np.abs(r) > c):.5f}, t-dimensional mixture {pred:.5f}")
# verify B3a once: the structured signs do send 1_V and 1_{V+1} where claimed
t = 16; V = n // t
k_idx = np.arange(n)
a = (k_idx % t == 0) / np.sqrt(V); b = (k_idx % t == 1) / np.sqrt(V)
bad = 0
for c1 in range(0, n, 7):
    chi = hadamard(n)[c1] * np.sqrt(n)
    D = rng.choice((-1.0, 1.0), n)
    D[k_idx % t == 0] = chi[k_idx % t == 0] * rng.choice((-1.0, 1.0))
    D[k_idx % t == 1] = chi[k_idx % t == 1] * rng.choice((-1.0, 1.0))
    za = (a * D) @ H; zb = (b * D) @ H
    ing = (k_idx ^ c1) < t
    ok = np.allclose(np.abs(za), ing / np.sqrt(t)) and np.allclose(np.abs(zb), ing / np.sqrt(t)) and abs(za @ zb) < 1e-9
    bad += (not ok)
print(f" B3a check, 37 characters: {bad} mismatches")

print()
print("== unconditional two rounds, n = 256, y = 1 on 0..7, Delta = 1 on 8..15, depth 1 ==")
y = np.zeros(n); y[:8] = 1; y /= np.linalg.norm(y)
q = np.zeros(n); q[8:16] = 1; q /= np.linalg.norm(q)
tot = 600_000
cnt = {5.0: 0, 6.0: 0, 7.0: 0, 7.8: 0}
ms = 0.0
for _ in range(tot // 20000):
    D1 = rng.choice((-1.0, 1.0), (20000, n)); D2 = rng.choice((-1.0, 1.0), (20000, n))
    Z = (((y * D1) @ H) * D2) @ H
    W = (((q * D1) @ H) * D2) @ H
    Z[np.abs(Z) < 1e-12] = 0.0
    e, A = err_vec(Z, 1)
    r = (e * W).sum(1) / (A / np.sqrt(n - 1))
    ms += (r * r).sum()
    for c in cnt:
        cnt[c] += int((np.abs(r) > c).sum())
print(f" {tot} draws: mean square {ms/tot:.3f}; counts above 5, 6, 7, 7.8 sd: {[cnt[c] for c in cnt]} "
      f"(law expects {tot*3.15e-7:.2f}, {tot*5.25e-10:.1e}, {tot*1.91e-13:.1e}, {tot*9.35e-17:.1e})")
