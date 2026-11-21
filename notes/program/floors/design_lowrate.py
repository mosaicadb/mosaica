# The design at low rate (CLAIMS.md, D1): the dual-basis Cauchy-Schwarz certificate h = |Delta|_M * rho_hat, with
# rho = |r|_{M^-1} stored rounded up on a geometric grid of ratio q, against the floors at the entropy of what it stores.
# In eigen-coordinates |Delta|_M^2 and |r|_{M^-1}^2 both have the law of Q = sum_i sqrt(gamma_i) g_i^2.
# E sqrt(Q) is computed exactly from the Laplace transform; the law of rho from a fixed sample.
# Run:  nice -n 10 taskset -c 6 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 design_lowrate.py
import json
import math
import os
import numpy as np
from scipy import integrate
import floors_lib as fl

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
spec = json.load(open(os.path.join(ROOT, "agent1", "spec_v1.json")))
LN2 = math.log(2)
rng = np.random.default_rng(20261121)


def mean_sqrt_Q(a):
    """E sqrt(sum a_i g_i^2) = (1/(2 sqrt pi)) int_0^inf (1 - prod (1 + 2 t a_i)^(-1/2)) t^(-3/2) dt."""
    a = np.asarray(a, float)
    f = lambda s: (1 - math.exp(-0.5 * np.log1p(2 * math.exp(s) * a).sum())) * math.exp(-0.5 * s)   # t = e^s, dt = t ds
    val, err = integrate.quad(f, -60, 60, limit=400)
    return val / (2 * math.sqrt(math.pi))


for g in sorted(spec, key=lambda g: -spec[g]["items"])[:2]:
    gam = np.sort(np.array(spec[g]["lam"]) * np.array(spec[g]["Qe"]))[::-1]
    sp = fl.Spectrum(gam)
    a = np.sqrt(gam)                                   # weights of Q
    F = float(a.sum()); S2 = float(gam.sum()); neff = F * F / S2
    m1 = mean_sqrt_Q(a)                                # E |g''| = E |Delta|_M = E |r|_{M^-1}
    cv2 = F / (m1 * m1) - 1
    # sample of rho for the grid's entropy (chunks keep the memory small)
    NS, CH = 200000, 4000
    rho = np.concatenate([np.sqrt((rng.standard_normal((CH, len(a))) ** 2) @ a) for _ in range(NS // CH)])
    print("group %s: F = %.5f, n_eff = %.1f, sqrt(gamma_1)/F = %.4f; E|Delta|_M = %.5f = sqrt(F) * %.5f (sample: %.5f); "
          "cv^2 = %.5f, 1/(2 n_eff) = %.5f; product of means / F = %.5f"
          % (g, F, neff, a[0] / F, m1, m1 / math.sqrt(F), rho.mean(), cv2, 1 / (2 * neff), m1 * m1 / F))
    print("  grid ratio q | H(rho_hat) bits | E h / F  | W(H)/F  V(H)/F | E h / W(H)  E h / V(H) | bound q/(1+cv^2) / (1 - sqrt(gamma_1) H / F)")
    med = float(np.median(rho))
    for lq in (1 / 2, 1 / 4, 1 / 8, 1 / 16, 1 / 32, 1 / 64, 1 / 256):
        q = 2.0 ** lq
        for off in (0.0,):
            j = np.ceil(np.log(rho / med) / math.log(q) - off)            # rounded up on the grid med * q^(j + off)
            rh = med * q ** (j + off)
            assert (rh >= rho * (1 - 1e-12)).all()
            _, cnt = np.unique(j, return_counts=True); P = cnt / len(rho)
            H = float(-(P * np.log(P)).sum())
            Eh = m1 * float(rh.mean())
            w = sp.W_suffix(0, H); v = sp.V(H)[0]
            print("  2^(1/%-3d)    | %6.2f          | %.4f   | %.4f   %.4f | %.4f      %.4f     | %.4f"
                  % (round(1 / lq), H / LN2, Eh / F, w / F, v / F, Eh / w, Eh / v, q / (1 + cv2) / (1 - a[0] * H / F)))
    print("  floor for any code of b bits, as a share of F: " + "  ".join(
        "b=%d: W %.4f V %.4f (1 - sqrt(b ln2/n_eff) = %.4f)" % (b, sp.W_suffix(0, b * LN2) / F, sp.V(b * LN2)[0] / F, 1 - math.sqrt(b * LN2 / neff))
        for b in (1, 2, 4, 8, 16)))
    print()
