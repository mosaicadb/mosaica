"""Referee 1, check on A3.1 in its own model e_i = kappa sqrt(nu_i) xi_i. Small numpy; one core.

For a fixed offset Delta: is E[(Delta.e)^2 | u] = (rho w u)^2 ?  Reported as sum(x^2)/sum(pred^2) over all rows and
within quintiles of u, with x = Delta.e and pred = rho(q,c) w u. The claim says 'approximately'; this sizes it.
"""
import math
import numpy as np

rng = np.random.default_rng(9)
n, rows, chunk = 384, 100000, 5000

pi = np.exp(-np.linspace(0, 4, n)); pi /= pi.sum()
profiles = {
    "a equal": np.ones(n),
    "water-filled: 70% coded at one a, 30% uncoded below it": np.concatenate([np.ones(int(0.7 * n)), np.linspace(0.95, 0.05, n - int(0.7 * n))]),
    "a log-normal, sd 1 (not water-filled)": np.exp(rng.normal(0, 1, n)),
}


def draw_xi(kind, size):
    if kind == "normal":
        return rng.normal(0, 1, size)
    if kind == "uniform":
        return rng.uniform(-math.sqrt(3), math.sqrt(3), size)
    return rng.exponential(1, size) - 1            # skewed, mean 0, variance 1


print("  profile | xi | kappa | query :  rho(q,c)/rho_g ;  sum x^2 / sum pred^2 overall ; by quintile of u ;  mean x/pred ;  |x|/pred at 99.9% (normal 3.29)")
for pname, a in profiles.items():
    nu = a * a / pi
    D2 = np.sqrt(nu / pi)                        # D_i^2
    rho_g = math.sqrt((a * a).sum()) / a.sum()
    queries = {
        "on profile": rng.normal(0, 1, n) * np.sqrt(pi),
        "energy on the low-a coordinates": rng.normal(0, 1, n) * np.sqrt(pi) / a,
        "energy on the high-a coordinates": rng.normal(0, 1, n) * np.sqrt(pi) * a,
        "8 coordinates only": np.where(np.arange(n) % 48 == 5, 1.0, 0.0) * np.sqrt(pi),
    }
    for xi_kind in ("normal", "uniform", "skewed"):
        for kap in ("fixed", "log-normal 0.4"):
            for qname, dl in queries.items():
                if pname != "a equal" and xi_kind == "skewed" and qname != "on profile":
                    continue
                w2 = (D2 * dl * dl).sum()
                rho = math.sqrt((dl * dl * nu).sum() / (w2 * a.sum()))
                xs = []; ps = []; us = []
                for _ in range(rows // chunk):
                    kappa = np.ones(chunk) if kap == "fixed" else np.exp(rng.normal(0, 0.4, chunk))
                    e = kappa[:, None] * np.sqrt(nu)[None, :] * draw_xi(xi_kind, (chunk, n))
                    u = np.sqrt((e * e / D2[None, :]).sum(1))
                    xs.append(e @ dl); us.append(u); ps.append(rho * math.sqrt(w2) * u)
                x = np.concatenate(xs); p = np.concatenate(ps); u = np.concatenate(us)
                qs = np.quantile(u, [0.2, 0.4, 0.6, 0.8])
                b = np.searchsorted(qs, u)
                byq = " ".join(f"{(x[b == i] ** 2).sum() / (p[b == i] ** 2).sum():.3f}" for i in range(5))
                print(f"  {pname[:28]:28s} | {xi_kind:7s} | {kap:14s} | {qname:32s}: {rho / rho_g:5.3f} ; {(x * x).sum() / (p * p).sum():.3f} ; {byq} ; {np.mean(x / p):+.3f} ; {np.quantile(np.abs(x) / p, 0.999):.2f}")
