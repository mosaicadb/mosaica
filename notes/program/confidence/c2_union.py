"""C2: checks for the union over N (CLAIMS.md, C2).

Part 1  kappa_{d-1} d / (kappa_d (d-1)) <= 1 + 2/d (used in Lemma C2).
Part 2  Corollary C3: gamma = N pi_2 / delta for a random code, by block size and bits; the least bits for gamma <= 0.02.
Part 3  Lemma C4: the largest error cosine chi_0 at which the pair bound is below delta * 1e-11.
Part 4  Monte Carlo of Theorem C on the equator corpus with a shared random codebook, small d:
        Pr[some item fails] against N*delta and 1-(1-delta)^N, and pi_2/delta^2, for few and many codewords.
Run: nice -n 10 taskset -c 5 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 c2_union.py
"""
import numpy as np
from scipy.special import gammaln, betainc
from scipy.optimize import brentq
from c1_converse import T, q, cap


def lnkappa(d):
    return gammaln((d + 1) / 2) - gammaln(d / 2) - 0.5 * np.log(np.pi)


def part1():
    worst = max(np.exp(lnkappa(d - 1) - lnkappa(d)) * d / (d - 1) / (1 + 2 / d) for d in range(4, 5001))
    print(f"PART 1  max over d = 4..5000 of [kappa_(d-1) d / (kappa_d (d-1))] / (1 + 2/d) = {worst:.6f}  (must be <= 1)")


def lncap(d, th, lnM):
    c = cap(d, th)
    if c > 1e-280:
        return np.log(c) + lnM
    return lnkappa(d) + d * np.log(np.sin(th)) - np.log(d) + lnM


def gamma_terms(d, bits, N, delta, g1):
    lnM = bits * np.log(2)
    Lam = np.log(2 * N / (g1 * delta))
    f = lambda th: lncap(d, th, lnM) - np.log(Lam)
    if f(np.pi / 2 - 1e-9) < 0:
        return Lam, np.nan, np.nan, np.inf
    th = brentq(f, 1e-9, np.pi / 2 - 1e-9)
    t2 = N * Lam ** 2 * delta
    t3 = (1 + 2 / d) * np.exp(np.log(N) + 2 * np.log(Lam) - lnM) / (np.sin(th) * np.cos(th))
    return Lam, th, t2, t3


def part2():
    N, delta, g1 = 1e9, 2.5e-19, 0.01
    print(f"\nPART 2  random code, N={N:.0e}, delta={delta}, gamma_1={g1}: gamma <= gamma_1 + term2 + term3")
    print("  d     bits   R=bits/d  Lambda  theta(deg)  term2     term3      gamma_bound")
    for d in [95, 127, 255, 287]:
        for bits in [24, 32, 40, 48, 52, 56, 64, 96, 192]:
            Lam, th, t2, t3 = gamma_terms(d, bits, N, delta, g1)
            if not np.isfinite(t3):
                print(f"  {d:<5d} {bits:<6d} {bits/d:<8.3f}  {Lam:.1f}    code too small for Lambda (theta would exceed 90 deg)")
                continue
            print(f"  {d:<5d} {bits:<6d} {bits/d:<8.3f}  {Lam:.1f}    {np.degrees(th):6.2f}     {t2:.2e}  {t3:.3e}  {g1+t2+t3:.4g}")
        lo = next(b for b in range(8, 400) if gamma_terms(d, b, N, delta, g1)[3] <= 0.01)
        print(f"  d={d}: least bits with term3 <= 0.01: {lo}")


def part3():
    delta = 2.5e-19
    print("\nPART 3  Lemma C4: T_(d-1)(q sqrt(2/(1+chi))) / delta, q = q_d(delta)")
    print("  d     q       chi=0       chi=0.2     chi=0.4     chi=0.6     chi=0.8     chi_0 (ratio = 1e-11)")
    for d in [95, 127, 255, 287]:
        qd = q(d, delta)

        def lnratio(chi):
            x = qd * np.sqrt(2 / (1 + chi))
            if x >= 1:
                return -np.inf
            # log of T_{d-1}(x): betainc may underflow; use log via the leading term when it does
            t = float(T(d - 1, x))
            if t > 1e-300:
                return np.log(t) - np.log(delta)
            return ((d - 2) / 2) * np.log(1 - x * x) - np.log(delta)  # upper bound of the tail up to a factor < 1
        vals = [lnratio(c) for c in (0, 0.2, 0.4, 0.6, 0.8)]
        chi0 = brentq(lambda c: lnratio(c) - np.log(1e-11), 0.0, 0.999) if lnratio(0.0) < np.log(1e-11) else float('nan')
        print(f"  {d:<5d} {qd:.4f}  " + "  ".join(("exp(%6.1f)" % v) if np.isfinite(v) else "    0      " for v in vals) + f"   {chi0:.3f}")


def part4(seed=5):
    print("\nPART 4  Monte Carlo of Theorem C: equator corpus, shared random codebook, plain rule, one-sided")
    rng = np.random.default_rng(seed)
    d, delta, N, trials, batch = 12, 0.01, 20, 100000, 500
    qd = q(d, delta)
    print(f"  d={d} delta={delta} q={qd:.4f} N={N} trials={trials}; independent items would give "
          f"1-(1-delta)^N = {1-(1-delta)**N:.4f}; N*delta = {N*delta:.2f}")
    print("  M      p_bar    Pr[some fail]  /(N*delta)  effective count  pi_2       pi_2/delta^2  Pr[F2|F1]")
    for M in [1, 2, 8, 64, 512]:
        code = rng.standard_normal((M, d + 1)); code /= np.linalg.norm(code, axis=1, keepdims=True)
        anyf = 0; f1 = 0; f12 = 0; ftot = 0
        for _ in range(trials // batch):
            D = rng.standard_normal((batch, 1, d + 1)); D /= np.linalg.norm(D, axis=2, keepdims=True)
            z = rng.standard_normal((batch, N, d + 1))
            z -= (z * D).sum(2, keepdims=True) * D
            z /= np.linalg.norm(z, axis=2, keepdims=True)
            dots = z.reshape(-1, d + 1) @ code.T
            idx = dots.argmax(1); cosf = dots[np.arange(batch * N), idx]
            Dx = (np.repeat(D[:, 0, :], N, axis=0) * code[idx]).sum(1)
            sinf = np.sqrt(np.maximum(1 - cosf ** 2, 0))
            fail = ((cosf > 0) & (-Dx > qd * sinf)).reshape(batch, N)
            anyf += fail.any(1).sum(); ftot += fail.sum()
            f1 += fail[:, 0].sum(); f12 += (fail[:, 0] & fail[:, 1]).sum()
        pbar = ftot / (trials * N); pu = anyf / trials; pi2 = f12 / trials
        print(f"  {M:<6d} {pbar:.5f}  {pu:.4f}         {pu/(N*pbar):.3f}       {pu/pbar:6.2f}           "
              f"{pi2:.2e}   {pi2/pbar**2:8.1f}      {f12/max(f1,1):.3f}")
    print("  (M = 1, 2: half the items have cos(phi) <= 0 and use the fallback, so p_bar is below delta)")


if __name__ == "__main__":
    part1(); part2(); part3(); part4()
