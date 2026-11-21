"""Referee 2, A4.1: the mean of cos^2 against the ratio of second moments, under A3's diagonal model.
e_i = sqrt(nu_i) xi_i, D_i^4 = nu_i/pi_i, so e~_i = sqrt(a_i) xi_i; Delta~_i^2 = a_i on the profile.
Prints sqrt(mean cos^2)/rho and sqrt(ratio of moments)/rho, rho = ||a||_2/||a||_1. Synthetic."""
import numpy as np
rng = np.random.default_rng(3)
n, rows = 384, 20000
def run(name, a):
    rho2 = (a**2).sum() / a.sum()**2
    dt = np.sqrt(a) * rng.choice([-1.0, 1.0], n)          # an offset on the profile, weighted
    dh = dt / np.linalg.norm(dt)
    out = []
    for law in ("normal", "one scale per row, lognormal 0.5"):
        xi = rng.normal(size=(rows, n))
        k = np.exp(0.5 * rng.normal(size=(rows, 1))) if law != "normal" else 1.0
        et = k * np.sqrt(a) * xi
        u2 = (et**2).sum(1); num = (et @ dh)**2
        out.append(f"{law}: rms of ratio / rho {np.sqrt((num/u2).mean()/rho2):.3f}, ratio of moments / rho {np.sqrt(num.sum()/u2.sum()/rho2):.3f}")
    print(f"{name:34s} rho {np.sqrt(rho2):.4f}  " + ";  ".join(out))
run("equal a_i", np.ones(n))
a = np.ones(n); a[int(0.7*n):] = np.linspace(0.9, 0.05, n - int(0.7*n)); run("water-filled, 70% coded", a)
run("log-normal, sd 1", np.exp(rng.normal(size=n)))
run("log-normal, sd 2", np.exp(2*rng.normal(size=n)))
