# Every proved floor on the measured spectra of agent1/spec_v1.json (the two largest groups, as agent1/theory.py),
# at 0, 4, 8, 16, 32 and 96 code bytes, beside the widths measured in agent1/theory.log (Gaussian data with these
# spectra, independent Gaussian offsets; three stored scalars of 16 bits each are charged, as theory.py does).
# Then the same floors at the stored-bit counts of quant/floor2.py's levels, and the non-Gaussian correction W(H + J).
#   T1, T2, T3: Theorems 1-3 of proofcheck/CONSOLIDATED_PROOFS.md [proved, refereed]
#   Phi: Proposition A (Conjecture A)      W: Theorem F1      V: Theorem F2 (best head set among top-k)
# Run:  nice -n 10 taskset -c 6 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 floors_eval.py
import json
import math
import os
import numpy as np
import floors_lib as fl

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
spec = json.load(open(os.path.join(ROOT, "agent1", "spec_v1.json")))
LN2 = math.log(2)
SCAL = 48
# measured mean half-widths from agent1/theory.log: code bits -> (uniform code, escape code)
MEAS = {"12": {0: (0.0724, 0.0724), 32: (0.0534, 0.0539), 64: (0.0446, 0.0445), 128: (0.0344, 0.0339), 256: (0.0238, 0.0229), 768: (0.0095, 0.0087)},
        "2": {0: (0.1520, 0.1520), 32: (0.1115, 0.1128), 64: (0.0928, 0.0924), 128: (0.0707, 0.0694), 256: (0.0479, 0.0459), 768: (0.0180, 0.0164)}}

for g in sorted(spec, key=lambda g: -spec[g]["items"])[:2]:
    gam = np.sort(np.array(spec[g]["lam"]) * np.array(spec[g]["Qe"]))[::-1]
    sp = fl.Spectrum(gam)
    F = sp.F; S2 = float(gam.sum())
    print("group %s: %d items, n = %d, F = sum sqrt(gamma) = %.5f, sum gamma = %.4e, n_eff = %.1f, sqrt(gamma_1)/F = %.4f"
          % (g, spec[g]["items"], len(gam), F, S2, F * F / S2, math.sqrt(gam[0]) / F))
    print("  bytes | bits charged |   T1     T2     T3   |  Phi   |   W (F1)  k  |   V (F2)  head k, tail nats | best old -> best new | "
          "uniform  escape | escape/V  escape/W  escape/best old")
    for B in (0, 32, 64, 128, 256, 768):
        bits = B + SCAL; H = bits * LN2
        t1, k1 = fl.T1(gam, H); t2 = fl.T2(gam, H); t3 = fl.T3(gam, bits)
        ph, _ = fl.Phi(gam, H); w, th, kw = fl.W(gam, H); v, kv, u = sp.V(H)
        old = max(t1, t2, t3); new = max(v, w, ph, old)
        mu, me = MEAS[g][B]
        assert me >= new * 0.999 and mu >= new * 0.999, "a measured width is below a proved floor"
        print("  %5d | %5d        | %.4f %.4f %.4f | %.4f | %.4f %4d | %.4f %4d  %7.2f        | %.4f -> %.4f     | %.4f  %.4f | %.3f     %.3f     %.3f"
              % (B // 8, bits, t1, t2, t3, ph, w, kw, v, kv, u, old, new, mu, me, me / v, me / w, me / old))
    # the same without charging the scalars: the floor for a code of exactly that many bits
    print("  floors with no scalar bits charged (H = code bits): bytes: W / V")
    print("   " + "   ".join("%d: %.4f / %.4f" % (B // 8, fl.W(gam, B * LN2)[0], sp.V(B * LN2)[0]) for B in (0, 32, 64, 128, 256, 768)))
    # the levels of quant/floor2.py ("after": 4-bit norms; stored bits = 8b + 16 + 4*level)
    print("  at the stored bits of quant/floor2.py's levels (4, 8, 12, 16, 24, 32 B): bits: T1 / W / V, and V/W, W/T1")
    for i, b in enumerate((4, 8, 12, 16, 24, 32)):
        bits = 8 * b + 16 + 4 * (i + 1); H = bits * LN2
        t1 = fl.T1(gam, H)[0]; w = fl.W(gam, H)[0]; v = sp.V(H)[0]
        print("    %2d B, %3d bits: %.4f / %.4f / %.4f   V/W = %.4f   W/T1 = %.4f" % (b, bits, t1, w, v, v / w, w / t1))
    # a non-Gaussian residual with negentropy J has the Gaussian floor at rate H + J (CLAIMS.md, F3)
    print("  non-Gaussian residuals: W(H + J) / W(H) for J = 23, 62, 80 bits (entropy/REPORT.md, BioCLIP-2; these J are lower bounds)")
    for B in (0, 32, 64, 128, 256, 768):
        H = (B + SCAL) * LN2
        w0 = fl.W(gam, H)[0]
        print("    %2d B: " % (B // 8) + "  ".join("J=%d: %.3f" % (J, fl.W(gam, H + J * LN2)[0] / w0) for J in (23, 62, 80)))
    # small-rate expansions used in CLAIMS.md
    for bts in (1, 4, 8, 16):
        H = bts * LN2
        w = fl.W(gam, H)[0]; ph = fl.Phi(gam, H)[0]
        print("  %2d bits: W = F - %.5f (bounds: sqrt(gamma_1) H = %.5f, sqrt(H sum gamma) = %.5f); Phi = F - %.5f (bound sqrt(H sum gamma)); "
              "Theorem 2 = %.5f" % (bts, F - w, math.sqrt(gam[0]) * H, math.sqrt(H * S2), F - ph, fl.T2(gam, H)))
    print()
