"""Referee check of E1/E2: recompute rho and the allowance for f32 and int16 at n = 256, 768 and depths 1, 4, 8,
attack the bound with the worst query for each item, and count how often a code made from the rounded matrix
differs from the code made from the exact one."""
import numpy as np

C = 7.8
LAMS = (2.0, 2.3, 2.6, 3.0, 3.5, 4.2)
rng = np.random.default_rng(20261121)


def haar(n):
    q, r = np.linalg.qr(rng.standard_normal((n, n)))
    return q * np.sign(np.diag(r))


def code(Z, depth):
    """built code, one rise at `depth`: decoded values (m, n), chosen per row among LAMS by least tan^2."""
    m, n = Z.shape
    yn2 = (Z * Z).sum(1)
    rms = np.sqrt(yn2 / n)
    half = 2.0 ** (depth - 1)
    best = np.full(m, np.inf)
    out = np.zeros_like(Z)
    lam_idx = np.zeros(m, dtype=int)
    for li, lam in enumerate(LAMS):
        cell = np.clip(np.floor(Z / (lam * rms[:, None]) * half) + half, 0, 2 * half - 1)
        val = (cell + 0.5) / half - 1.0
        dot = (val * Z).sum(1)
        cn = (val * val).sum(1)
        obj = np.log(np.maximum(cn * yn2 - dot * dot, 1e-300) / np.maximum(dot * dot, 1e-300))
        upd = obj < best
        best[upd] = obj[upd]
        out[upd] = val[upd]
        lam_idx[upd] = li
    return out, lam_idx


def rounded(R, kind):
    if kind == "f32":
        return R.astype(np.float32).astype(np.float64)
    top = 32767
    scale = top / np.abs(R).max()
    return np.round(R * scale) / scale


for n in (256, 768):
    R = haar(n)
    d = n - 1
    Y = rng.standard_normal((300, n))
    Z = Y @ R.T
    print(f"n = {n}")
    for kind in ("f32", "int16"):
        Rt = rounded(R, kind)
        N = Rt - R
        rho = np.linalg.svd(N, compute_uv=False)[0]
        fro = np.linalg.norm(N)
        print(f"  {kind}: rho (svd) = {rho:.4e}, Frobenius = {fro:.4e}, max row 2-norm = {np.sqrt((N*N).sum(1)).max():.3e}")
        Zt = Y @ Rt.T
        for depth in (1, 4, 8):
            O, li = code(Z, depth)
            yn = np.linalg.norm(Z, axis=1)
            dot = (O * Z).sum(1)
            on = np.linalg.norm(O, axis=1)
            kappa = yn ** 2 / dot
            cosf = dot / (on * yn)
            tanf = np.sqrt(1 - cosf ** 2) / cosf
            secf = 1 / cosf
            # worst query for each item: Delta proportional to N^T o (unit norm)
            W = O @ N  # rows: N^T o
            wn = np.linalg.norm(W, axis=1)
            Dl = W / wn[:, None]
            # computed difference of the two estimates, f64 products, against a long double reference
            est_t = kappa * np.einsum("ij,ij->i", O, Dl @ Rt.T)
            est_e = kappa * np.einsum("ij,ij->i", O, Dl @ R.T)
            ref = (kappa.astype(np.longdouble) *
                   np.einsum("ij,ij->i", O.astype(np.longdouble), (Dl.astype(np.longdouble) @ N.T.astype(np.longdouble))))
            allow = rho * yn * secf * 1.0
            ratio = np.abs(est_t - est_e) / allow
            ratio_ref = np.abs(ref.astype(np.float64)) / allow
            sd = yn * tanf / np.sqrt(d)
            share_item = allow / (C * sd)
            rms_tan = np.sqrt((tanf ** 2).mean())
            share_rms = rho * np.sqrt(d) / (C * np.sin(np.arctan(rms_tan)))
            slot = kappa * np.sqrt(n) * rho  # the form proposed for the kernel's slot, per unit |Delta|
            # codes made from the rounded matrix
            Ot, lit = code(Zt, depth)
            differs = (np.abs(Ot - O).max(1) > 0)
            ncoord = (np.abs(Ot - O) > 0).sum(1)
            print(f"    depth {depth}: rms tan {rms_tan:.4f}; share at rms tan {100*share_rms:.3g}%; per-item share "
                  f"median {100*np.median(share_item):.3g}% max {100*share_item.max():.3g}%; "
                  f"worst-query change/allowance max {ratio.max():.3f} (long double {ratio_ref.max():.3f}); "
                  f"slot form / E1 form median {np.median(slot/allow):.2f} max {(slot/allow).max():.2f}")
            print(f"             code from rounded matrix differs for {differs.mean():.4f} of items; "
                  f"mean changed coordinates {ncoord.mean():.3f}; scale changed for {np.mean(li != lit):.4f}; "
                  f"mean 2^Q over items (Q = changed coordinates) {np.mean(2.0 ** np.minimum(ncoord, 60)):.4g}")
