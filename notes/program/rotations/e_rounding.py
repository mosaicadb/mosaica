"""E: a rounded copy of the rotation on the query side. Norms of the rounding error and the allowance as a share of
the random half-width c*sd, for f32, int16 and int8 at blocks of 64 to 768.

share = rho * sqrt(n-1) / (c * sin(phi)), rho a bound on |R~ - R| in operator norm (claim E1).
"""
import numpy as np
from lib import haar, built_code, estimate_error

C = 7.8
rng = np.random.default_rng(11)


def schatten_bound(N, log2p=6):
    """An upper bound on the operator norm: (tr (N^T N)^p)^(1/(2p)), p = 2^log2p, by repeated squaring in f64."""
    S = N.T @ N
    logscale = 0.0
    for _ in range(log2p):
        f = np.linalg.norm(S)
        S = S / f
        logscale = 2 * (logscale + np.log(f))
        S = S @ S
    # after the loop S_true = exp(logscale) * S with S_true = (N^T N)^(2^log2p)
    p = 2 ** log2p
    return np.exp((logscale + np.log(np.trace(S))) / (2 * p))


def rounded(R, kind, clip=None):
    if kind == "f32":
        return R.astype(np.float32).astype(np.float64)
    bits = {"int16": 15, "int8": 7}[kind]
    top = 2 ** bits - 1
    rng_ = np.abs(R).max() if clip is None else clip
    scale = top / rng_
    return np.clip(np.round(R * scale), -top, top) / scale


# tan(phi) of the built code by depth, on uniform directions (n = 256, one rise at that depth)
Z = rng.standard_normal((400, 256))
tans = {}
for d in range(1, 9):
    cell = built_code(Z, [d])
    _, _, _, tan = estimate_error(Z, cell, d, d)
    tans[d] = float(np.sqrt((tan ** 2).mean()))
print("built code, rms tan(phi) by depth 1..8:", " ".join(f"{tans[d]:.4f}" for d in range(1, 9)))
print(f"share of the random half-width = rho*sqrt(n-1)/(c*sin(phi)), c = {C}; shown at depths 1, 2, 4, 8")
print()
for n in (64, 128, 256, 336, 512, 688, 768):
    R = haar(n, rng)
    defect = np.linalg.norm(R.T @ R - np.eye(n))
    print(f"n = {n}: max|R_ij| = {np.abs(R).max():.4f} ({np.abs(R).max()*np.sqrt(n):.2f}/sqrt(n)); |R^T R - I|_F = {defect:.1e}")
    for kind in ("f32", "int16", "int8"):
        variants = [(None, "range = max entry")]
        if kind == "int8":
            variants += [(t / np.sqrt(n), f"range = {t}/sqrt(n), clipped") for t in (2.5, 3.0, 3.5)]
        for clip, label in variants:
            N = rounded(R, kind, clip) - R
            fro = np.linalg.norm(N)
            op = np.linalg.norm(N, 2)
            cert = schatten_bound(N)
            oneinf = np.sqrt(np.abs(N).sum(0).max() * np.abs(N).sum(1).max())
            sh = lambda rho, d: rho * np.sqrt(n - 1) / (C * np.sin(np.arctan(tans[d])))
            print(f"   {kind:5s} {label:28s}: op norm {op:.3e} (svd), certified {cert:.3e} (Schatten-128), "
                  f"sqrt(|N|_1 |N|_inf) {oneinf:.3e}, Frobenius {fro:.3e}")
            print(f"         share with certified op norm at depth 1/2/4/8: "
                  + " / ".join(f"{100*sh(cert, d):.3g}%" for d in (1, 2, 4, 8))
                  + ";  with Frobenius: " + " / ".join(f"{100*sh(fro, d):.3g}%" for d in (1, 2, 4, 8)))
    # floating-point error of the product R~ x: gamma_n * | |R~| |x| | <= gamma_n * sqrt(n) |x|
    for name, u in (("f64 accumulation", 2.0 ** -53), ("f32 accumulation", 2.0 ** -24)):
        gam = n * u / (1 - n * u)
        rho_fp = gam * np.sqrt(n)
        print(f"   product in {name}: bound gamma_n*sqrt(n) = {rho_fp:.2e}; share at depth 1/2/4/8: "
              + " / ".join(f"{100*rho_fp*np.sqrt(n-1)/(C*np.sin(np.arctan(tans[d]))):.2g}%" for d in (1, 2, 4, 8)))
    # what the rounding actually does to estimates, against the allowance (int8, depth 2, random queries)
    Rq = rounded(R, "int8")
    ys = rng.standard_normal((200, n)); qs = rng.standard_normal((200, n))
    Zs = ys @ R.T
    cell = built_code(Zs, [2])
    kappa, e, A, tan = estimate_error(Zs, cell, 2, 2)
    o = (e + Zs)  # kappa * codeword
    diff = np.abs((o * (qs @ (Rq - R).T)).sum(1))
    bound = np.linalg.norm(o, axis=1) * np.linalg.norm(Rq - R, 2) * np.linalg.norm(qs, axis=1)
    print(f"   int8, depth 2, 200 random pairs: actual change of the estimate / allowance: mean {np.mean(diff/bound):.3f}, max {np.max(diff/bound):.3f}")
    print()
