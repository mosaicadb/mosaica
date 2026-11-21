"""Referee 2, A4.4/A4.5: the law of a row's error given its height in the author's Model C, in population.
s - mu = R + aG + eG, a - mu = (1-k) R + aG, e = k R + eG; R exponential of scale b = 1/lam (sigma = 1), aG, eG normal.
All quantities centred as the author's script centres them. Per setting: bulk statistics, then by height x:
median y/x, mean y, and at the threshold (rows with x in a thin bin at zeta) the keep probability of
  plain psi tau;  (zeta + psi0 sqrt f) tau;  psi0 sqrt f tau + (1-f) T;  kappa_q T + psi0 s_res  (A4.5 item 5).
Then the same at the largest kappa the model allows (eG = 0). Synthetic."""
import math, sys
import numpy as np
LAM = float(sys.argv[1]) if len(sys.argv) > 1 else 2.2
rng = np.random.default_rng(7)
N, CH = 24_000_000, 2_000_000

def run(name, f, kap, meas):
    b = 1.0 / LAM; vG = 1 - b*b
    one_m_fG = ((1 - f) - kap*kap*b*b) / vG
    if one_m_fG < -1e-9: print(f"{name}: infeasible"); return
    one_m_fG = max(one_m_fG, 0.0)
    va, ve = (1 - one_m_fG) * vG, one_m_fG * vG
    tau = math.sqrt(1 - f); sres = math.sqrt((1-kap)**2 * ve + kap**2 * va)
    c0 = ((1-kap)*ve - kap*va) / b                               # tail intercept of E[e | s], my derivation
    xs, ys = [], []
    cov_ae = var_a = 0.0
    for _ in range(N // CH):
        R = rng.exponential(b, CH) - b
        aG = rng.normal(0, math.sqrt(va), CH); eG = rng.normal(0, math.sqrt(ve), CH) if ve > 0 else np.zeros(CH)
        a = (1-kap)*R + aG; e = kap*R + eG; s = a + e
        cov_ae += (a*e).mean(); var_a += (a*a).mean()
        keep = s > 0.5
        xs.append(s[keep].astype(np.float32)); ys.append((e[keep]/tau).astype(np.float32))
    x = np.concatenate(xs); y = np.concatenate(ys)
    cov_ae /= N // CH; var_a /= N // CH
    print(f"\n{name}: f {f}, kappa_q {kap:.3f} (largest allowed {tau*LAM:.3f}), kappa_q/sqrt(1-f) {kap/tau:.2f}; bulk rho_es {(tau*tau+cov_ae)/tau:.2f}; "
          f"Cov(a,e)/tau^2 {cov_ae/tau**2:.2f} (so actual over form 1 = {1/math.sqrt(1+2*cov_ae/tau**2):.2f}); slope of s on a, all rows {1+cov_ae/var_a:.3f}; "
          f"s_res/(sqrt f tau) {sres/(math.sqrt(f)*tau):.2f}; tail line y = {kap/tau:.2f} x {c0/tau:+.2f}")
    print(f"   measured for this index: {meas}")
    line = "   median y/x by height:"
    for lo, hi in ((0.5, 2), (2, 3), (3, 4), (4, 5), (5, 6)):
        m_ = (x >= lo) & (x < hi)
        line += f"  x {lo}-{hi}: {np.median(y[m_]/x[m_]):.2f} (n {int(m_.sum())})"
    print(line)
    for zeta in (3.0, 4.0):
        m_ = (x >= zeta) & (x < zeta + 0.05); yy = y[m_]
        line = f"   at the threshold, zeta {zeta:.0f} ({int(m_.sum())} rows): mean y {yy.mean():.2f}, sd {yy.std():.2f}; plain psi 4: {np.mean(yy <= 4):.3f};"
        line += "  (zeta + psi0 sqrt f) tau, psi0 0/1/2/3: " + "/".join(f"{np.mean(yy <= zeta + p*math.sqrt(f)):.3f}" for p in (0, 1, 2, 3)) + ";"
        line += "  kappa 1-f, psi0 2/3/4: " + "/".join(f"{np.mean(yy <= p*math.sqrt(f) + zeta*tau):.3f}" for p in (2, 3, 4)) + ";"
        line += "  kappa_q T + psi0 s_res, psi0 0/1/2: " + "/".join(f"{np.mean(yy*tau <= kap*zeta + p*sres):.3f}" for p in (0, 1, 2))
        line += f" (with my intercept {c0/sres:+.2f} s_res added: " + "/".join(f"{np.mean(yy*tau <= kap*zeta + c0 + p*sres):.3f}" for p in (0, 1, 2)) + ")"
        print(line)

sets = (("iNaturalist, 12", 0.92, 0.36, "median r 0.33, median y/x 1.17, slope 1.21; y/x 1.06 at x 0.5-2 rising to 1.3 at x 4-5; (zeta+psi0 sqrt f) tau keeps 0.918/0.975 at psi0 2/3; plain psi 4 0.818"),
        ("iNaturalist, 1", 0.86, 0.50, "median r 0.47, median y/x 1.21, slope 1.24; keeps 0.918/0.972; plain 0.824"),
        ("yahoo", 0.82, 0.39, "median r 0.36, median y/x 0.94, slope 0.99; keeps 0.976/0.992; plain 0.803; kappa 1-f at psi0 2/3/4: 0.47-0.73 / 0.77-0.90 / 0.934"),
        ("LAION, 64", 0.71, 0.50, "median r 0.46, median y/x 0.89, slope 0.92; keeps 0.969/0.985; plain 0.809"),
        ("LAION, 8", 0.67, 0.58, "median r 0.55, median y/x 0.96, slope 0.98; keeps 0.964/0.981; plain 0.790"))
print(f"Model C in population, tail rate {LAM} a spread. The author's kappa_q, then the largest the model allows.")
for name, f, kap, meas in sets:
    run(name, f, kap, meas)
    run(name + " at the largest kappa_q", f, math.sqrt(1 - f) * LAM * 0.9999, meas)
