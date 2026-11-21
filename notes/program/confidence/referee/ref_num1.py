import numpy as np
from scipy.special import betainc, betaincinv, gammaln, log1p
from scipy.optimize import brentq
from scipy.integrate import quad

def T(d, x):
    x = float(x)
    if x >= 1: return 0.0
    if x <= -1: return 1.0
    t = 0.5 * betainc((d - 1) / 2, 0.5, 1 - x * x)
    return t if x >= 0 else 1 - t

def qd(d, delta):
    # T(d,x)=delta -> I_{1-x^2}((d-1)/2,1/2) = 2 delta
    v = betaincinv((d - 1) / 2, 0.5, 2 * delta)
    return np.sqrt(1 - v)

def lncap(d, th):
    """ln Omega_d(theta) on S^d in R^(d+1); log-space integration to avoid underflow."""
    c = 0.5 * betainc(d / 2, 0.5, np.sin(th) ** 2)
    if c > 1e-250:
        return np.log(c)
    # integrate kappa_d * int_0^th sin^(d-1) t dt in log space
    lk = gammaln((d + 1) / 2) - gammaln(d / 2) - 0.5 * np.log(np.pi)
    f = lambda t: np.exp((d - 1) * (np.log(np.sin(t)) - np.log(np.sin(th))))
    val, _ = quad(f, 0, th, epsabs=0, epsrel=1e-10)
    return lk + (d - 1) * np.log(np.sin(th)) + np.log(val)

def H(q, th):
    if th >= np.pi / 2: return 1.0
    return q * np.sin(th) / np.sqrt(np.cos(th) ** 2 + q * q * np.sin(th) ** 2)

delta = 2.5e-19
print("check quantiles: ", [(d, round(qd(d, delta), 4), round(qd(d, delta) * np.sqrt(d), 3)) for d in (95, 127, 255, 287, 767)])
print("self-check T(qd)=delta:", T(95, qd(95, delta)), T(255, qd(255, delta)))

def th_at(d, lnM, target):
    f = lambda th: lncap(d, th) + lnM - np.log(target)
    if f(np.pi / 2 - 1e-9) < 0: return np.pi / 2
    return brentq(f, 1e-7, np.pi / 2 - 1e-9, xtol=1e-13)

def row(d, R):
    q = qd(d, delta); lnM = R * d * np.log(2)
    thR = np.arcsin(2.0 ** (-R)); tanR = np.tan(thR)
    k = q * q * tanR ** 2
    plain = q * tanR; wstar = q / np.sqrt(4 ** R - 1 + q * q)
    th1 = th_at(d, lnM, 2 * np.log(d)); ach = H(q, th1)
    # exact mean of H_q(phi) for the random code: F(th)=1-(1-Omega)^M ; E H = int_0^{pi/2} H'(th) (1-F(th)) dth + P(phi>pi/2)*... use survival
    def surv(th):
        lc = lncap(d, th) + lnM          # ln(M Omega)
        # (1-Omega)^M = exp(M ln(1-Omega)) ~ exp(-M Omega) for tiny Omega
        Om = np.exp(lncap(d, th))
        if Om < 1e-8: return np.exp(-np.exp(lc))
        return np.exp(np.exp(lnM) * np.log1p(-Om)) if lnM < 700 else 0.0
    # E H = int_0^{pi/2} dH/dth * surv(th) dth  (H(0)=0), with H=1 beyond pi/2 included since H(pi/2)=1
    ths = np.linspace(1e-6, np.pi / 2, 4001)
    Hs = np.array([H(q, t) for t in ths]); Ss = np.array([surv(t) for t in ths])
    EH = np.sum(np.diff(Hs) * 0.5 * (Ss[1:] + Ss[:-1]))
    # B6, rigorous left-endpoint sum (integrand nondecreasing in u)
    best = (0, None)
    nu = 400
    for m in (5, 9, 17, 33):
        eps = np.pi / (2 * (m - 1))
        for rho in (0.5, 0.8, 0.95):
            tot_left = 0.0; tot_mid = 0.0
            for i in range(nu):
                for which in (0, 1):
                    u = (i + 0.5 * which) / nu
                    if u == 0: continue
                    tail = m * delta / (rho * u)
                    x = qd(d, tail) if tail < 0.5 else 0.0
                    val = H(x, th_at(d, lnM, (1 - rho) * u)) if x > 0 else 0.0
                    if which == 0: tot_left += val
                    else: tot_mid += val
            b_left = np.cos(eps) * tot_left / nu; b_mid = np.cos(eps) * tot_mid / nu
            if b_left > best[0]: best = (b_left, (m, rho, b_mid))
    # Theorem B single-point, optimised coarsely
    bestB = 0
    for m in (5, 9, 17, 33, 65):
        eps = np.pi / (2 * (m - 1))
        for lam in (3, 5, 10, 20, 50, 100):
            q0 = qd(d, m * lam * delta)
            for beta in (0.3, 0.1, 0.03, 0.01):
                bestB = max(bestB, np.cos(eps) * H(q0, th_at(d, lnM, beta)) * (1 - 1 / lam - beta))
    print(f"d={d} R={R}: q={q:.4f} k={k:.3f} plain={plain:.4f} w*={wstar:.4f} ach(H at theta1)={ach:.4f} "
          f"exact E H_q(phi) random code={EH:.4f}  ThmB point={bestB:.4f}  B6 left-sum={best[0]:.4f} (m,rho,mid)={best[1]}")
    print(f"     B6/ach={best[0]/ach:.3f}  B6/w*={best[0]/wstar:.3f}  B6/exactEH={best[0]/EH:.3f}  theta1={th1:.4f} thetaR={thR:.4f}")

row(95, 1.0)
row(255, 0.25)
row(95, 0.25)

# B4: ratio q_d(d^2 delta)/q_d(delta) along delta_d with ln(1/delta)/ln d growing
print("\nB4 quantile ratio q_d(d^2 delta)/q_d(delta):")
for d in (95, 255, 1000, 10000, 100000):
    for L in (5, 10, 30, 100):   # ln(1/delta) = L * ln d
        dl = np.exp(-L * np.log(d))
        if d * d * dl >= 0.5 or dl < 1e-300: continue
        print(f"  d={d} ln(1/delta)/ln d={L}: {qd(d, d*d*dl)/qd(d, dl):.4f}", end=";")
    print()

# two-sided A4: worst-s total failure at q, vs 2T(q)
print("\nA4 two-sided: max over s of the two tails, in units of T(q); 1.0 means one tail only matters")
for d, q, tanphi in ((12, 0.6339, 0.93), (95, 0.7565, 0.5774), (95, 0.7565, 1.5538), (255, 0.5186, 0.5774), (255, 0.5186, 0.2582), (255, 0.5186, 0.05)):
    k = q * q * tanphi ** 2; a = -k / (1 + k); b = tanphi / (1 + k); eta = np.sqrt(k / (1 + k))
    ss = np.linspace(-0.999, 0.999, 3999)
    tot = [T(d, (eta + s * a) / (np.sqrt(1 - s * s) * b)) + T(d, (eta - s * a) / (np.sqrt(1 - s * s) * b)) for s in ss]
    one = [T(d, (eta + s * a) / (np.sqrt(1 - s * s) * b)) for s in ss]
    i = int(np.argmax(one))
    print(f"  d={d} q={q} tan={tanphi} k={k:.3f}: max one-sided/T(q)={max(one)/T(d,q):.4f} at s={ss[i]:.3f} (pred {np.sqrt(k/(1+k)):.3f}); max two-sided/T(q)={max(tot)/T(d,q):.4f}; at s=0 two-sided/T(q)={tot[len(ss)//2]/T(d,q):.3g}")

# confidence tax
print("\nconfidence tax:")
for d in (95, 255, 767):
    q = qd(d, delta)
    for w in (0.3, 0.1, 0.03, 0.01, 0.001):
        tax = 0.5 * np.log2((w * w + q * q * (1 - w * w)) / (w * w + 1 / d))
        print(f"  d={d} q={q:.4f} c={q*np.sqrt(d):.3f} w={w}: tax={tax:.3f} (limit {np.log2(q*np.sqrt(d)):.3f}; 1/sqrt(d)={1/np.sqrt(d):.3f})")

# C3: bits needed
print("\nC3: last term (1+2/d) N Lam^2/(M sin cos) by bits")
N = 1e9; g1 = 0.01; Lam = np.log(2 * N / (g1 * delta)); print("  Lambda=", Lam, " middle term=", N * Lam ** 2 * delta)
for d in (95, 127, 255, 287):
    out = []
    for B in (40, 44, 48, 49, 50, 51, 52, 56, 64):
        lnM = B * np.log(2); th = th_at(d, lnM, Lam)
        last = (1 + 2 / d) * N * Lam ** 2 / (np.exp(lnM) * np.sin(th) * np.cos(th)) if th < np.pi/2 else np.inf
        # exact cap ratio instead of the F1 comparison
        exact = N * Lam * np.exp(lncap(d - 1, th)) if th < np.pi/2 else np.inf
        out.append(f"{B}b:{last:.3g}/{exact:.3g}")
    print(f"  d={d}: " + "  ".join(out) + "   (bound / with exact Omega_(d-1))")
# kappa ratio inequality
bad = [d for d in range(2, 5000) if np.exp(gammaln(d/2)*2 - gammaln((d+1)/2) - gammaln((d-1)/2)) * d/(d-1) > 1 + 2/d + 1e-12]
print("  kappa_(d-1) d/(kappa_d (d-1)) <= 1+2/d fails for d in:", bad[:10], " ; kappa_(d-1)/kappa_d>1 for:", [d for d in range(2, 5000) if np.exp(gammaln(d/2)*2 - gammaln((d+1)/2) - gammaln((d-1)/2)) > 1 + 1e-12][:5])

# C4: chi0
print("\nC4: chi0 with T_{d-1}(q sqrt(2/(1+chi))) = delta*1e-11")
for d in (95, 127, 255, 287):
    q = qd(d, delta)
    f = lambda chi: (np.log(max(T(d - 1, q * np.sqrt(2 / (1 + chi))), 1e-320)) - np.log(delta * 1e-11))
    try:
        chi0 = brentq(f, -0.99, 0.9999)
    except Exception as e:
        chi0 = None
    print(f"  d={d} q={q:.4f} chi0={chi0}  T at chi=0 / delta = {T(d-1, q*np.sqrt(2))/delta:.3g}")
