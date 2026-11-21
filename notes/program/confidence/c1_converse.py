"""C1: checks of the converse and of the shrunk-centre scheme (CLAIMS.md, C1.3).

Part 1  Monte Carlo of four intervals from one message (A1 plain, N1(b) naive shrunk, A4 shrunk centre, N1(c) pivot)
        at small d, where delta is large enough to count failures.
Part 2  The steepness of the sphere tail (Lemma B2) at the index's block sizes.
Part 3  Theorem B's finite-d bound against the achievable half-width (A4 with the cap-bound angle).
Small numpy/scipy only. Run: nice -n 10 taskset -c 5 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 c1_converse.py
"""
import numpy as np
from scipy.special import betainc, gammaln
from scipy.optimize import brentq


def T(d, x):
    """Pr[U >= x], U one coordinate of a uniform unit vector in R^d."""
    x = np.asarray(x, dtype=float)
    ax = np.clip(np.abs(x), 0, 1)
    t = 0.5 * betainc((d - 1) / 2, 0.5, 1 - ax * ax)
    return np.where(x >= 0, t, 1 - t)


def q(d, delta):
    return brentq(lambda x: np.log(T(d, x)) - np.log(delta), 1e-9, 1 - 1e-12)


def cap(d, theta):
    """Normalised measure of a cap of angle theta <= pi/2 on S^d in R^(d+1)."""
    return 0.5 * betainc(d / 2, 0.5, np.sin(theta) ** 2)


def H(qq, theta):
    return qq * np.sin(theta) / np.sqrt(np.cos(theta) ** 2 + qq * qq * np.sin(theta) ** 2)


def part1(seed=1):
    print("PART 1  Monte Carlo, one-sided failure rates (harmful side: truth - estimate > half-width)")
    rng = np.random.default_rng(seed)
    d, Rbits, delta = 12, 0.75, 0.01
    M = int(round(2 ** (Rbits * d)))
    qd = q(d, delta)
    code = rng.standard_normal((M, d + 1))
    code /= np.linalg.norm(code, axis=1, keepdims=True)
    print(f"  d={d} M={M} (R={Rbits}) delta={delta} q_d(delta)={qd:.4f}  codebook fixed, (y, Delta) rotated")
    print("  s = Delta.y ; columns: failure rate of each rule, then its mean half-width")
    print("  s       plain   naive   shrunkA4 pivot  | w_plain w_naive w_A4   w_pivot  mean k")
    n, batch = 400000, 4000
    for s in [0.0, 0.2, 0.4, 0.6, 0.8, -0.4]:
        fails = np.zeros(4)
        widths = np.zeros(4)
        ksum = 0.0
        for _ in range(n // batch):
            u = rng.standard_normal((batch, d + 1))
            u /= np.linalg.norm(u, axis=1, keepdims=True)
            v = rng.standard_normal((batch, d + 1))
            v -= (v * u).sum(1, keepdims=True) * u
            v /= np.linalg.norm(v, axis=1, keepdims=True)
            Dl = s * u + np.sqrt(1 - s * s) * v
            dots = u @ code.T
            idx = dots.argmax(1)
            cosf = dots[np.arange(batch), idx]
            x0 = code[idx]
            E0 = (Dl * x0).sum(1)
            sinf = np.sqrt(1 - cosf ** 2)
            tanf = sinf / cosf
            k = qd * qd * tanf ** 2
            ksum += k.mean()
            est = [E0 / cosf, cosf * E0, E0 / (cosf * (1 + k)), (E0 / cosf) / (1 + k)]
            hw = [qd * tanf, qd * sinf * cosf, qd * tanf / np.sqrt(1 + k),
                  qd * tanf * np.sqrt(np.maximum(1 + k - (E0 / cosf) ** 2, 0)) / (1 + k)]
            for j in range(4):
                fails[j] += np.count_nonzero(s - est[j] > hw[j])
                widths[j] += hw[j].sum()
        fr = fails / n
        w = widths / n
        print(f"  {s:+.2f}   {fr[0]:.4f}  {fr[1]:.4f}  {fr[2]:.4f}   {fr[3]:.4f} | "
              f"{w[0]:.4f}  {w[1]:.4f}  {w[2]:.4f} {w[3]:.4f}   {ksum / (n // batch):.3f}")
    print(f"  standard error of a rate near delta: {np.sqrt(delta * (1 - delta) / n):.5f}")
    print("  worst s for A4 is +sqrt(k/(1+k)); at mean k that is about", f"{np.sqrt(ksum/(n//batch)/(1+ksum/(n//batch))):.2f}")


def part2():
    print("\nPART 2  steepness: T_d(t*q)/T_d(q) with T_d(q) = delta (one-sided), delta = 1e-9/(1e9*4) = 2.5e-19")
    delta = 2.5e-19
    print("  d     q_d(delta)  c=q*sqrt(d)   t=0.99     t=0.98     t=0.95     t=0.90")
    for d in [95, 127, 255, 287, 767]:
        qd = q(d, delta)
        r = [float(T(d, t * qd) / delta) for t in (0.99, 0.98, 0.95, 0.90)]
        print(f"  {d:<5d} {qd:.4f}      {qd*np.sqrt(d):.3f}        " + "  ".join(f"{x:9.3g}" for x in r))
    print("  quantile ratio q_d(m*lam*delta)/q_d(delta), the cost of Lemma B2's Markov and union steps:")
    print("  d     x10      x100     x1000    x1e4")
    for d in [95, 127, 255, 287, 767]:
        qd = q(d, delta)
        print(f"  {d:<5d} " + "  ".join(f"{q(d, f*delta)/qd:.4f}" for f in (10, 100, 1e3, 1e4)))


def part3():
    print("\nPART 3  Theorem B's bound over the achievable w (both one-sided, delta = 2.5e-19), by block and rate")
    print("  achievable: A4 at the angle theta_1 with M*Omega_d(theta_1) = 2 ln d (random code, A3)")
    print("  bound: max over (m, q0, theta) of cos(eps_m) H_q0(theta) (1 - m delta/T_d(q0) - M Omega_d(theta))")
    print("  d    R     q       tan_R   k_R    plain q*tan_R  w*=H_q(th_R)  achievable  bound    bound/ach  bound/w*")
    delta = 2.5e-19
    for d in [95, 255]:
        qd = q(d, delta)
        for R in [0.25, 1.0, 2.0, 7.0]:
            lnM = R * d * np.log(2)
            thR = np.arcsin(2.0 ** (-R))

            def lncap(th):
                c = cap(d, th)
                if c > 1e-280:
                    return np.log(c) + lnM
                # F1: kappa_d sin^d/d <= Omega <= kappa_d sin^d/(d cos); cos is 1 to 1e-4 where this branch is used
                return gammaln((d + 1) / 2) - gammaln(d / 2) - 0.5 * np.log(np.pi) + d * np.log(np.sin(th)) - np.log(d) + lnM

            # theta with M*Omega = target
            def th_at(target):
                f = lambda th: lncap(th) - np.log(target)
                if f(np.pi / 2 - 1e-9) < 0:
                    return np.pi / 2
                return brentq(f, 1e-6, np.pi / 2 - 1e-9)

            th1 = th_at(2 * np.log(d))
            ach = H(qd, th1) if th1 < np.pi / 2 else 1.0
            best = 0.0
            for m in [5, 9, 17, 33, 65, 129]:
                eps = np.pi / (2 * (m - 1))
                for lam in [3, 5, 10, 20, 50, 100]:
                    q0 = q(d, min(m * lam * delta, 0.4))
                    for beta in [0.3, 0.1, 0.03, 0.01]:
                        th = th_at(beta)
                        val = np.cos(eps) * H(q0, th) * (1 - 1 / lam - beta)
                        best = max(best, val)
            # Corollary B6: E eta >= cos(eps_m) * integral over u of H_{x(u)}(theta(u)),
            # T_d(x(u)) = m delta/(rho u), M Omega_d(theta(u)) = (1-rho) u
            us = (np.arange(400) + 0.5) / 400
            best6 = 0.0
            for m in [5, 9, 17, 33]:
                eps = np.pi / (2 * (m - 1))
                for rho in [0.5, 0.8, 0.95]:
                    tot = 0.0
                    for u in us:
                        tail = m * delta / (rho * u)
                        x = q(d, tail) if tail < 0.49 else 0.0
                        tot += H(x, th_at((1 - rho) * u))
                    best6 = max(best6, np.cos(eps) * tot / len(us))
            tanR = np.tan(thR)
            wstar = H(qd, thR)
            print(f"  {d:<4d} {R:<5.2f} {qd:.4f}  {tanR:.4f}  {qd*qd*tanR*tanR:.3f}  {qd*tanR:.4f}         "
                  f"{wstar:.4f}        {ach:.4f}      {best:.4f}   {best/ach:.3f}      {best/wstar:.3f}"
                  f"   B6: {best6:.4f} {best6/ach:.3f} {best6/wstar:.3f}")


if __name__ == "__main__":
    part2()
    part3()
    part1()
