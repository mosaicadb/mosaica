"""B2: two and more rounds R = H D_k ... H D_1 against the code as built (depth 1 unless said).

(a) n = 16, two rounds, exhaustive over D_2 for y = e_1, Delta = e_2: the exact law of error/sd against the sphere law.
(b) n = 256, two rounds, unconditional Monte Carlo for several fixed pairs: tail of error/sd against the sphere law.
(c) n = 256 and 64, k rounds, conditional on k-1 structured rounds (each of exactly known probability): the
    conditional failure rate at 7.8 sd, hence a lower bound on the unconditional failure probability.
"""
import sys
import numpy as np
from scipy.special import betainc
from lib import fwht, built_code, estimate_error

rng = np.random.default_rng(7)
C = 7.8
THR = np.array([3.0, 4.0, 5.0, 6.0, 7.0, 7.8, 9.0, 11.0])


def law_two_sided(c, d):
    """Pr[|sqrt(d) U| > c], U a coordinate of a uniform unit vector in R^d."""
    s2 = np.minimum(c * c / d, 1.0)
    return betainc((d - 1) / 2, 0.5, 1 - s2)


def char(c, n):
    k = np.arange(n)
    return 1.0 - 2.0 * (np.array([bin(int(c) & int(i)).count("1") for i in k]) % 2)


def ratio_batch(y, Dl, Ds, depth=1):
    """error/sd for a batch: y, Dl fixed vectors (n,), Ds a list of sign arrays (m, n), first round first."""
    a = np.broadcast_to(y, Ds[0].shape)
    b = np.broadcast_to(Dl, Ds[0].shape)
    for D in Ds:
        a = fwht(a * D)
        b = fwht(b * D)
    cell = built_code(a, [depth])
    kappa, e, A, tan = estimate_error(a, cell, depth, depth)
    n = y.size
    Dperp = Dl - (Dl @ y) / (y @ y) * y
    sd = A * np.linalg.norm(Dperp) / np.sqrt(n - 1)
    return (e * b).sum(1) / np.maximum(sd, 1e-300), A


def part_a():
    n = 16
    y = np.zeros(n); y[1] = 1
    Dl = np.zeros(n); Dl[2] = 1
    m = 1 << n
    D2 = 1.0 - 2.0 * ((np.arange(m)[:, None] >> np.arange(n)[None, :]) & 1)
    D1 = np.ones((m, n))
    r, A = ratio_batch(y, Dl, [D1, D2])
    print(f"(a) n = 16, two rounds, y = e1, Delta = e2, all 2^16 choices of D2 (D1 changes only signs): "
          f"mean square of error/sd = {np.mean(r**2):.4f} (law: 1), max |error/sd| = {np.abs(r).max():.3f} (law: at most {np.sqrt(n-1):.3f})")
    for c in (1.5, 2.0, 2.5, 3.0, 3.4, 3.7):
        print(f"     Pr[|error| > {c} sd]: exact {np.mean(np.abs(r) > c):.3e}   sphere law {law_two_sided(c, n-1):.3e}   ratio {np.mean(np.abs(r) > c)/law_two_sided(c, n-1):.2f}")


def part_b(total=2_000_000, chunk=20000):
    n = 256
    pairs = {}
    y = np.zeros(n); y[:8] = 1; Dl = np.zeros(n); Dl[8:16] = 1
    pairs["y = 1 on 0..7, Delta = 1 on 8..15"] = (y, Dl)
    y = np.zeros(n); y[:16] = 1; Dl = np.zeros(n); Dl[16:32] = 1
    pairs["y = 1 on 0..15, Delta = 1 on 16..31"] = (y, Dl)
    y = np.zeros(n); y[1] = 1; Dl = np.zeros(n); Dl[2] = 1
    pairs["y = e1, Delta = e2"] = (y, Dl)
    y = np.zeros(n); y[0] = 1; y[1] = 1; Dl = np.zeros(n); Dl[0] = 1; Dl[1] = -1
    pairs["y = e0 + e1, Delta = e0 - e1"] = (y, Dl)
    g = np.random.default_rng(3)
    y = g.standard_normal(n); Dl = g.standard_normal(n)
    pairs["control: y, Delta fixed Gaussian vectors"] = (y, Dl)
    print(f"(b) n = {n}, two rounds, depth 1, {total} draws per pair; Pr[|error| > c sd] at c = {list(THR)}")
    print("     sphere law (d = 255):", " ".join(f"{law_two_sided(c, n-1):.2e}" for c in THR))
    for name, (y, Dl) in pairs.items():
        cnt = np.zeros(len(THR)); ms = 0.0; mx = 0.0
        for _ in range(total // chunk):
            Ds = [rng.choice((-1.0, 1.0), (chunk, n)) for _ in range(2)]
            r, A = ratio_batch(y, Dl, Ds)
            cnt += (np.abs(r)[:, None] > THR[None, :]).sum(0)
            ms += (r * r).sum(); mx = max(mx, np.abs(r).max())
        print(f"     {name}: mean square {ms/total:.3f}, max {mx:.2f}; ", " ".join(f"{c/total:.2e}" for c in cnt))
        sys.stdout.flush()


def part_c(total=400_000, chunk=20000):
    print("(c) k rounds, the first k-1 structured: D_j on the current support equal to a character (times a sign).")
    print("    P(structure) is exact; the conditional rate is Monte Carlo; the product bounds Pr[|error| > 7.8 sd] from below.")
    for n, r in ((256, 4), (256, 3), (64, 3), (1024, 5)):
        V = 1 << r
        y = np.zeros(n); y[:V] = 1
        Dl = np.zeros(n); Dl[V:2 * V] = 1
        chars = np.array([char(c, n) for c in range(n)])
        for k in (2, 3, 4, 5, 6):
            if n == 1024 and k > 3:
                continue
            tot = total if n <= 256 else 100_000
            cnt = 0; cnt6 = 0; ms = 0.0; mx = 0.0; log2p = 0.0
            for it in range(tot // chunk):
                a = np.broadcast_to(y, (chunk, n)).copy()
                b = np.broadcast_to(Dl, (chunk, n)).copy()
                lp = 0.0
                for j in range(k):
                    D = rng.choice((-1.0, 1.0), (chunk, n))
                    if j < k - 1:
                        # each row of a is a signed indicator of a coset; put a random character on its support
                        sz = int((a[0] != 0).sum())
                        assert ((a != 0).sum(1) == sz).all()
                        cs = rng.integers(0, n, chunk)
                        sg = rng.choice((-1.0, 1.0), (chunk, 1))
                        D = np.where(a != 0, chars[cs] * sg, D)
                        lp += (np.log2(sz) + 1) - sz
                    a = fwht(a * D)
                    b = fwht(b * D)
                    if j < k - 1:
                        a[np.abs(a) < 1e-9] = 0.0
                log2p = lp
                cell = built_code(a, [1])
                kappa, e, A, tan = estimate_error(a, cell, 1, 1)
                sd = A * np.linalg.norm(Dl) / np.sqrt(n - 1)
                rr = (e * b).sum(1) / np.maximum(sd, 1e-300)
                cnt += (np.abs(rr) > C).sum(); cnt6 += (np.abs(rr) > 6.0).sum(); ms += (rr * rr).sum(); mx = max(mx, np.abs(rr).max())
            p = cnt / tot
            print(f"    n = {n}, |V| = {V}, k = {k}: log2 P(structure) = {log2p:.0f}; conditional: mean square of error/sd {ms/tot:.2f}, "
                  f"max {mx:.1f}, Pr[>6 sd] {cnt6/tot:.2e}, Pr[>7.8 sd] {p:.2e}  =>  Pr[fail] >= 2^{log2p + (np.log2(p) if p > 0 else -np.inf):.1f}")
            sys.stdout.flush()


if __name__ == "__main__":
    which = sys.argv[1] if len(sys.argv) > 1 else "abc"
    if "a" in which:
        part_a()
    if "b" in which:
        part_b()
    if "c" in which:
        part_c()
