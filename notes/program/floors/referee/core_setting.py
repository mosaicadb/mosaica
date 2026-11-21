# Referee check of the eigen-coordinate reduction of section 0 for Sigma_Q, Sigma_R that do not commute and are singular.
#   Delta . r = sum_{i<=m} sqrt(gam_i) g_i z_i   (to rounding, sample by sample),   cov(z) = I_m,   z independent of g.
import numpy as np
rng = np.random.default_rng(4)
n = 7
A = rng.normal(size=(n, 4)); SQ = A @ A.T                      # rank 4
Bm = rng.normal(size=(n, 5)); SR = Bm @ Bm.T                   # rank 5
print("commutator norm %.2f, rank SQ %d, rank SR %d" % (np.linalg.norm(SQ @ SR - SR @ SQ), 4, 5))
w, U = np.linalg.eigh(SQ); w = np.clip(w, 0, None); SQh = (U * np.sqrt(w)) @ U.T
gam, V = np.linalg.eigh(SQh @ SR @ SQh); o = np.argsort(gam)[::-1]; gam, V = gam[o], V[:, o]
m = int((gam > 1e-9 * gam[0]).sum())
N = 200000
r = rng.normal(size=(N, 5)) @ Bm.T                             # r ~ N(0, SR)
g0 = rng.normal(size=(N, n)); Delta = g0 @ SQh
z = (r @ SQh @ V[:, :m]) / np.sqrt(gam[:m]); g = g0 @ V
lhs = (Delta * r).sum(1); rhs = (np.sqrt(gam[:m]) * g[:, :m] * z).sum(1)
print("rank m = %d; max |Delta.r - sum sqrt(gam) g z| = %.2e; max |cov(z) - I| = %.3f (sampling error ~ %.3f)"
      % (m, np.abs(lhs - rhs).max(), np.abs(np.cov(z.T) - np.eye(m)).max(), 3 / np.sqrt(N)))
