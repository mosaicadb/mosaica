import numpy as np, math, sys
sys.path.insert(0, __file__.rsplit('/', 1)[0])
from tailcheck import log_tail, c_exact
rng = np.random.default_rng(3)
# ---- B1. Theorem B(ii): the interval end from the quadratic, against a brute-force grid over t, all sign cases.
def roots(est, R, k):
    disc = (1 + k * k) * R * R - est * est
    return (est - k * math.sqrt(disc)) / (1 + k * k), (est + k * math.sqrt(disc)) / (1 + k * k)
bad2 = bad1 = bad1c = 0; ncase = 0; worst = 0.0
for _ in range(20000):
    R = rng.uniform(0.1, 2); k = rng.uniform(0.01, 1.5); est = rng.uniform(-1, 1) * R * math.sqrt(1 + k * k) * 0.999
    t = np.linspace(-R, R, 20001); rhs = k * np.sqrt(np.maximum(R * R - t * t, 0))
    two = np.abs(t - est) <= rhs; one = (t - est) <= rhs        # two-sided event; one-sided (upper-bound) event
    lo, hi = roots(est, R, k); ncase += 1
    if two.any():
        if abs(t[two].max() - hi) > 2e-4 * R or abs(t[two].min() - lo) > 2e-4 * R: bad2 += 1
    bf1 = t[one].max()
    if bf1 > hi + 2e-4 * R: bad1 += 1; worst = max(worst, (bf1 - hi) / R)
    hic = hi if est <= R else R
    if bf1 > hic + 2e-4 * R: bad1c += 1
print(f'B1: {ncase} random (est, R, kappa): two-sided set != [t-, t+] in {bad2} cases; one-sided upper end > t+ in {bad1} cases (all have est > R; worst excess {worst:.3f} R); with the cap "t_hi = R when est > R": {bad1c}')
# ---- B2. monotonicity of t+ in est: outward rounding of est is "up" only while est <= R
R, k = 1.0, 0.4
for est in (0.5, 0.9, 1.0, 1.02, 1.05, 1.07):
    print(f'   R=1 kappa=0.4 est={est:.2f}: t+ = {roots(est, R, k)[1]:.5f}')
# ---- B3. backstop gap: t+(tan phi) - t+(kappa) against (tan phi - kappa) R  [= (sqrt(m) - c) w sd]
viol = 0; tot = 0; wv = 0; region = {'est<0': [0, 0], '0<=est<=R': [0, 0], 'est>R': [0, 0]}
for _ in range(200000):
    R = 1.0; tp = rng.uniform(0.05, 1.5); k = rng.uniform(0.02, 1) * tp
    est = rng.uniform(-1, 1) * R * math.sqrt(1 + k * k)
    gap = roots(est, R, tp)[1] - roots(est, R, k)[1]; plain = (tp - k) * R
    key = 'est<0' if est < 0 else ('0<=est<=R' if est <= R else 'est>R'); region[key][1] += 1
    if gap > plain * (1 + 1e-9): region[key][0] += 1; wv = max(wv, gap / plain)
print('B3: backstop gap exceeds (sqrt(m)-c) w sd in', {k_: f'{v[0]}/{v[1]}' for k_, v in region.items()}, f'; worst ratio {wv:.1f}')
print('    example R=1, kappa=0.3, tan phi=0.5, est=-1.044: det end %.3f, randomised end %.3f, gap %.3f > plain gap 0.200' % (roots(-1.044, 1, .5)[1], roots(-1.0440, 1, .3)[1], roots(-1.044, 1, .5)[1] - roots(-1.044, 1, .3)[1]))
# ---- E. level-1 test with w_j but no centre term: effective constant and failure probability per event
for n, w in ((100, 0.80), (256, 0.93)):
    c = c_exact(n - 1, 1e-9 / 60); ce = c * w            # worst case: q_perp(r) ~ 1
    d = math.exp(log_tail(n - 1, ce / math.sqrt(n - 1)))
    print(f'E: n={n} c={c:.2f}, w={w}: width c*w*sd used with the error of the uncorrected estimator (sd*|q_perp| up to sd): effective constant {ce:.2f}, one-sided tail {d:.2e} against the budget {1e-9/60:.1e} ({d/(1e-9/60):.0f}x); k*L*2*delta = {60*d:.1e}')
# ---- C. integer kernel, section 1.4: the stated range budget
U, h, Rcap = 64, 7, 16000
Hmax = U * (2 ** h - 1); THmax = Hmax + Rcap
print(f'C: U={U} h={h} Rcap={Rcap}: stated budget 2U(2^h-1)+Rcap+64 = {2*Hmax+Rcap+64} <= 32767. Unmasked lanes allow |Hs|<={Hmax}, |TH|<={THmax}, RH<={Rcap}:')
print(f'   d = Hs - TH - RH can be {-Hmax - THmax - Rcap} (int16 min is -32768): leaves the range by {Hmax + THmax + Rcap - 32768}.  No-saturation needs 2U(2^h-1) + 2 Rcap + 66 <= 32767, i.e. Rcap <= {(32767 - 66 - 2 * Hmax) // 2}')
a = np.array([-Hmax], np.int16); b = np.array([THmax], np.int16); r = np.array([Rcap], np.int16)
wrap = (a - b - r)           # wrapping int16 arithmetic
print(f'   wrapping int16: Hs-TH-RH = {int(wrap[0])} (true {-Hmax-THmax-Rcap}); IN test "d + (g>>8) >= 0" is {bool(wrap[0] >= 0)} (true answer False)')
sat = lambda x: max(-32768, min(32767, x))
errs = 0
for _ in range(2000000 // 1000):
    Hs = rng.integers(-Hmax, Hmax + 1, 1000); TH = rng.integers(-THmax, THmax + 1, 1000); RH = rng.integers(0, Rcap + 1, 1000); g = rng.integers(-510, 16576, 1000)
    f = g >> 8
    for sgn in (-1, 1):
        true = (Hs - TH + sgn * RH + f) >= 0
        d1 = np.clip(Hs - TH, -32768, 32767); d2 = np.clip(d1 + sgn * RH, -32768, 32767); e = np.clip(d2 + f, -32768, 32767)
        errs += int(((e >= 0) != true).sum())
print(f'   with saturating ops in the order (Hs - TH) -/+ RH, then + (g>>8): sign mismatches {errs} in 4,000,000 (so saturation is harmless if every op saturates)')
# ---- Lemma R: |mulhrs(Theta, K) - m*Theta| with K = round(2^15 m), and with K off by a few units (K from a mulhi chain)
Th = rng.integers(-32768, 32768, 2000000); mm = rng.uniform(0, 1, 2000000); K = np.minimum(np.round(mm * 32768), 32767)
P = (Th * K + 16384) // 32768
print(f'Lemma R: K = round(2^15 m) clamp 32767: max |P - m Theta| = {np.abs(P - mm * Th).max():.3f} (claimed <= 1.5)')
for off in (1, 2, 3):
    P = (Th * np.maximum(K - off, 0) + 16384) // 32768
    print(f'   K low by {off} unit(s) (truncating mulhi chain): max |P - m Theta| = {np.abs(P - mm * Th).max():.3f}')
