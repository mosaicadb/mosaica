"""D: a random butterfly (log2 n layers of n/2 independent Givens rotations, layer l pairing i with i xor 2^l,
layer 0 first) and a random-pair Kac walk, against the code as built. y = e0, Delta = e1.

Butterfly: claim D2 says error = (kappa <o_odd, z_odd> - |z_odd|^2) / (|z_odd| |z_even|), a function of z alone.
"""
import numpy as np
from lib import built_code, estimate_error

rng = np.random.default_rng(5)
C = 7.8


def butterfly(X, rng, signs=True):
    """Apply one random butterfly per row of X (m, n). Independent uniform angles per pair and layer."""
    m, n = X.shape
    X = X.copy()
    l = 0
    idx = np.arange(n)
    while (1 << l) < n:
        lo = idx[(idx >> l) & 1 == 0]
        hi = lo | (1 << l)
        th = rng.uniform(0, 2 * np.pi, (m, lo.size))
        c, s = np.cos(th), np.sin(th)
        a, b = X[:, lo].copy(), X[:, hi].copy()
        X[:, lo] = c * a - s * b
        X[:, hi] = s * a + c * b
        l += 1
    return X


def run_butterfly(n, m, depth):
    Y = np.zeros((2, n)); Y[0, 0] = 1; Y[1, 1] = 1
    errs = []; pred_ok = []
    for _ in range(m // 2000):
        # apply the same butterfly to e0 and e1: stack and reuse the generator state
        state = rng.bit_generator.state
        Z = butterfly(np.broadcast_to(Y[0], (2000, n)), rng)
        rng.bit_generator.state = state
        W = butterfly(np.broadcast_to(Y[1], (2000, n)), rng)
        cell = built_code(Z, [depth])
        kappa, e, A, tan = estimate_error(Z, cell, depth, depth)
        err = (e * W).sum(1)
        o = e + Z
        odd = np.arange(n) % 2 == 1
        so = np.linalg.norm(Z[:, odd], axis=1); se = np.linalg.norm(Z[:, ~odd], axis=1)
        pred = ((o[:, odd] * Z[:, odd]).sum(1) - so ** 2) / (so * se)
        pred_ok.append(np.max(np.abs(np.abs(pred) - np.abs(err))))
        errs.append(err / (A / np.sqrt(n - 1)))
    r = np.concatenate(errs)
    print(f"butterfly n = {n}, depth {depth}, {r.size} draws: |error| equals the formula of D2 to {max(pred_ok):.1e}; "
          f"rms of error/sd = {np.sqrt((r**2).mean()):.2f} (law: 1); Pr[|error| > {C} sd] = {(np.abs(r) > C).mean():.3f}; "
          f"median |error|/sd = {np.median(np.abs(r)):.2f}; sqrt(n-1) = {np.sqrt(n-1):.2f}")


def run_kac(n, steps_per_n, m, depth):
    """Random-pair Kac walk with T = steps_per_n * n steps; y = e0, Delta = e1."""
    T = int(steps_per_n * n)
    Z = np.zeros((m, n)); Z[:, 0] = 1
    W = np.zeros((m, n)); W[:, 1] = 1
    rows = np.arange(m)
    for _ in range(T):
        i = rng.integers(0, n, m); j = (i + rng.integers(1, n, m)) % n
        th = rng.uniform(0, 2 * np.pi, m); c, s = np.cos(th), np.sin(th)
        for X in (Z, W):
            a, b = X[rows, i].copy(), X[rows, j].copy()
            X[rows, i] = c * a - s * b
            X[rows, j] = s * a + c * b
    untouched = np.mean(Z[:, 0] == 1.0)
    cell = built_code(Z, [depth])
    kappa, e, A, tan = estimate_error(Z, cell, depth, depth)
    r = (e * W).sum(1) / np.maximum(A / np.sqrt(n - 1), 1e-300)
    r[A < 1e-12] = 0
    print(f"Kac walk n = {n}, T = {steps_per_n} n = {T} steps, {m} draws: share with coordinate 0 never touched = {untouched:.4f} "
          f"(exp(-2T/n) = {np.exp(-2*T/n):.4f}); mean square of error/sd = {(r**2).mean():.2f}; Pr[|error| > 4 sd] = {(np.abs(r) > 4).mean():.2e} "
          f"(law 5.1e-05 at d = 255, 3.5e-05 at d = 63); max = {np.abs(r).max():.1f}")


if __name__ == "__main__":
    for n in (64, 256):
        for depth in (1, 2):
            run_butterfly(n, 20000, depth)
    for spn in (1, 2, 4, 8):
        run_kac(64, spn, 20000, 1)
