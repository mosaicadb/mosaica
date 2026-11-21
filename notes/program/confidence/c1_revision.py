"""C1 after the referee: recomputations asked for by referee-lower-bounds.md.

Part 1  The exact mean of H_q(phi) for the random code (the true achievable mean) and B6 as a rigorous left sum.
Part 2  The tax at finite w.
Part 3  B5: max over s of the sum of A4's two tails, in units of T_d(q).
Part 4  Monte Carlo redone: the naive shrunk rule with the half-width the averaged requirement allows
        (q_(d+1)(delta) sin phi); the pivot's upper end with and without the cap of Theorem 3.11; how often A4's
        upper end is above the plain one; the smallest of the three upper ends as one event.
Run: nice -n 10 taskset -c 5 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 c1_revision.py
"""
import numpy as np
from scipy.special import gammaln
from scipy.optimize import brentq
from c1_converse import T, q, cap, H


def lnkappa(d):
    return gammaln((d + 1) / 2) - gammaln(d / 2) - 0.5 * np.log(np.pi)


def lncap(d, th):
    c = cap(d, th)
    if c > 1e-280:
        return np.log(c)
    return lnkappa(d) + d * np.log(np.sin(th)) - np.log(d)


def part1():
    delta = 2.5e-19
    print("PART 1  delta = 2.5e-19, one-sided.  mean = E H_q(phi), Pr[phi > th] = (1 - Omega_d(th))^M (random code, A3)")
    print("  d    R     w*      'achievable'(2 ln d)  exact mean  B6 midpoint  B6 left sum  left/mean  left/w*")
    for d in [95, 255]:
        qd = q(d, delta)
        for R in [0.25, 1.0, 2.0, 7.0]:
            lnM = R * d * np.log(2)
            thR = np.arcsin(2.0 ** (-R))

            def th_at(target):
                f = lambda th: lncap(d, th) + lnM - np.log(target)
                if f(np.pi / 2 - 1e-9) < 0:
                    return np.pi / 2
                return brentq(f, 1e-7, np.pi / 2 - 1e-9)

            th1 = th_at(2 * np.log(d))
            # exact mean: integral of H'(th) Pr[phi > th] over (0, pi/2); H = 1 beyond
            lo, hi = th_at(1e-12), th_at(60.0)
            ths = np.linspace(lo, hi, 20001)
            surv = np.array([np.exp(-np.exp(lncap(d, t) + lnM)) for t in ths])   # (1-Om)^M = exp(-M Om) to 1e-9 here
            Hs = H(qd, ths)
            mean = H(qd, lo) + np.sum(0.5 * (surv[1:] + surv[:-1]) * np.diff(Hs)) + (1 - H(qd, hi)) * surv[-1]
            res = {}
            for rule in ("mid", "left"):
                best = 0.0
                K = 400
                us = (np.arange(K) + (0.5 if rule == "mid" else 0.0)) / K
                for m in [5, 9, 17, 33]:
                    eps = np.pi / (2 * (m - 1))
                    for rho in [0.5, 0.8, 0.95]:
                        tot = 0.0
                        for u in us:
                            if u == 0:
                                continue
                            tail = m * delta / (rho * u)
                            x = q(d, tail) if tail < 0.49 else 0.0
                            tot += H(x, th_at((1 - rho) * u))
                        best = max(best, np.cos(eps) * tot / K)
                res[rule] = best
            wstar = H(qd, thR)
            print(f"  {d:<4d} {R:<5.2f} {wstar:.4f}  {H(qd, th1):.4f}                {mean:.4f}      {res['mid']:.4f}       "
                  f"{res['left']:.4f}       {res['left']/mean:.3f}      {res['left']/wstar:.3f}")


def part2():
    print("\nPART 2  tax = 0.5 log2[(w^2 + q^2 (1-w^2)) / (w^2 + 1/d)], delta = 2.5e-19")
    print("  d     q       sqrt(1/d)  limit log2(q sqrt d)   w=0.3   w=0.1   w=0.03  w=0.01  w=0.003")
    for d in [95, 255, 767]:
        qd = q(d, 2.5e-19)
        tax = lambda w: 0.5 * np.log2((w * w + qd * qd * (1 - w * w)) / (w * w + 1 / d))
        print(f"  {d:<5d} {qd:.4f}  {1/np.sqrt(d):.4f}     {np.log2(qd*np.sqrt(d)):.3f}                 "
              + "   ".join(f"{tax(w):.3f}" for w in (0.3, 0.1, 0.03, 0.01, 0.003)))


def part3():
    print("\nPART 3  A4 run two-sidedly at q_d(delta): max over s of (upper tail + lower tail) / T_d(q), delta = 2.5e-19")
    for d, k in [(95, 0.191), (95, 1.381), (255, 0.090), (255, 0.018), (255, 0.001)]:
        qd = q(d, 2.5e-19)
        a, b = -k / (1 + k), np.sqrt(k) / qd / (1 + k)       # tan phi = sqrt(k)/q
        eta = np.sqrt(k / (1 + k))
        s = np.linspace(-0.9999, 0.9999, 40001)
        r = np.sqrt(1 - s * s) * b
        lt = np.logaddexp(np.log(np.maximum(T(d, (eta + s * a) / r), 1e-320)),
                          np.log(np.maximum(T(d, (eta - s * a) / r), 1e-320))) - np.log(2.5e-19)
        print(f"  d={d} k={k}: {np.exp(lt.max()):.4f}")


def part4(seed=11):
    print("\nPART 4  Monte Carlo, d=12 (n=13), M=512 random codewords, delta=0.01, 400,000 draws per row")
    rng = np.random.default_rng(seed)
    d, M, delta = 12, 512, 0.01
    qd, qd1 = q(d, delta), q(d + 1, delta)
    code = rng.standard_normal((M, d + 1)); code /= np.linalg.norm(code, axis=1, keepdims=True)
    n, batch = 400000, 4000
    print(f"  q_d={qd:.4f}  q_(d+1)={qd1:.4f}.  One-sided rates of 'truth above the upper end'.")
    print("  s        plain   naive*  A4      pivot   pivot+cap  min-of-3   frac[A4 end > plain end]")

    def run(s):
        f = np.zeros(6); above = 0
        for _ in range(n // batch):
            u = rng.standard_normal((batch, d + 1)); u /= np.linalg.norm(u, axis=1, keepdims=True)
            if s is None:                                   # Delta uniform and independent of y
                Dl = rng.standard_normal((batch, d + 1)); Dl /= np.linalg.norm(Dl, axis=1, keepdims=True)
                sv = (Dl * u).sum(1)
            else:
                v = rng.standard_normal((batch, d + 1)); v -= (v * u).sum(1, keepdims=True) * u
                v /= np.linalg.norm(v, axis=1, keepdims=True)
                Dl = s * u + np.sqrt(1 - s * s) * v; sv = np.full(batch, s)
            dots = u @ code.T; idx = dots.argmax(1); cosf = dots[np.arange(batch), idx]
            E0 = (Dl * code[idx]).sum(1); sinf = np.sqrt(1 - cosf ** 2); tanf = sinf / cosf
            k = qd * qd * tanf ** 2; Eh = E0 / cosf
            up_plain = Eh + qd * tanf
            up_naive = cosf * E0 + qd1 * sinf
            up_a4 = Eh / (1 + k) + np.sqrt(k / (1 + k))
            disc = 1 + k - Eh ** 2
            up_piv = Eh / (1 + k) + np.sqrt(k) * np.sqrt(np.maximum(disc, 0)) / (1 + k)
            up_cap = np.where(Eh >= 1, 1.0, np.where(disc >= 0, np.minimum(1.0, up_piv), 1.0))
            up_min = np.minimum(np.minimum(up_plain, up_a4), up_cap)
            for j, e in enumerate((up_plain, up_naive, up_a4, up_piv, up_cap, up_min)):
                f[j] += np.count_nonzero(sv > e)
            above += np.count_nonzero(up_a4 > up_plain)
        return f / n, above / n

    for s in [None, 0.0, 0.3, 0.6, 0.9, 0.99, -0.3, -0.9]:
        fr, ab = run(s)
        lab = "uniform" if s is None else f"{s:+.2f}  "
        print(f"  {lab}  " + "  ".join(f"{x:.4f}" for x in fr[:4]) + f"  {fr[4]:.4f}     {fr[5]:.4f}     {ab:.3f}")
    print("  * naive: centre cos(phi) E0, half-width q_(d+1)(delta) sin(phi): the rule an averaged requirement allows")
    print(f"  standard error near delta: {np.sqrt(delta/n):.5f}")


if __name__ == "__main__":
    part2(); part3(); part1(); part4()
