# Shared functions for the design-consequences claims (C3, C4). Small numpy only.
import json, math, os, numpy as np
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..")
def spectra(name="spec_v1.json"):
    s = json.load(open(os.path.join(ROOT, "agent1", name)))
    out = {}
    for g in s:
        gam = np.sort(np.array(s[g]["lam"]) * np.array(s[g]["Qe"]))[::-1]
        out[int(g)] = (np.sqrt(np.maximum(gam, 1e-300)), s[g]["items"])   # amplitudes a_i = sqrt(gamma_i), descending
    return out
def wf(a, H):
    """Water-filling: least sum of a_i s_i with sum ln(1/s_i) <= H nats. Returns (W, level)."""
    if H <= 0: return float(a.sum()), float(a.max())
    lo, hi = 1e-300, float(a.max())
    for _ in range(200):
        lam = math.sqrt(lo * hi)
        if np.log(np.maximum(a / lam, 1)).sum() > H: lo = lam
        else: hi = lam
    return float(np.minimum(a, hi).sum()), hi
def wf_level(a, lam):
    """Nats used and sum left by a prefix with water level lam."""
    return float(np.log(np.maximum(a / lam, 1)).sum()), float(np.minimum(a, lam).sum())
# tan^2 phi of the block code at R bits per position
def t_ideal(R): return 1.0 / (4.0 ** R - 1.0) if R > 0 else float("inf")
# built code over the sphere bound, one rate (codes/out_built.txt): R = 1/4: 1.498; 1: 1.305; 2: 1.404-1.415; 3: 1.51; 4: 1.60; 6: 1.67; 7: 1.69
_RB = np.array([0.0, 0.25, 1, 2, 3, 4, 6, 7, 9]); _QB = np.array([1.498, 1.498, 1.305, 1.41, 1.51, 1.60, 1.67, 1.69, 1.69])
SCALE_UP = 1.0245                                   # stored sd over true sd (bounds3 scale_check.log)
def rho_built(R): return float(np.interp(R, _RB, _QB))
def t_built(R): return (rho_built(R) * SCALE_UP) ** 2 * t_ideal(R)
# built deterministic code over W: quant/REPORT.md gives 1.17-1.21 on real residuals with every stored bit counted
RHO_DET = 1.19
