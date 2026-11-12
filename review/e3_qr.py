# Is Q from np.linalg.qr(Gaussian) Haar? Check sign of Q[0,0] and of diag(R); effect on a coordinate-aligned item.
import numpy as np
rng = np.random.default_rng(1); nb = 96; T = 2000
q00 = []; rdiag_neg = []; u0 = []
for _ in range(T):
    Qm, R = np.linalg.qr(rng.standard_normal((nb, nb)))
    q00.append(Qm[0, 0]); rdiag_neg.append((np.diag(R) < 0).mean())
    # the design rotates with y -> y @ Qm.T = Qm y ; for y = e_j the rotated vector is column j of Qm. Its sign pattern feeds the 1-bit code.
    u0.append(Qm[:, 0])
q00 = np.array(q00); u0 = np.array(u0)
print("share of draws with Q[0,0] < 0:", (q00 < 0).mean(), " (Haar: 0.5)")
print("mean share of negative diag(R):", np.mean(rdiag_neg), " (sign-fixed Haar construction: 0)")
print("column 0 of Q: share of negative entries by position (first 6):", (u0 < 0).mean(0)[:6].round(3))
# after the standard fix
ok = []
for _ in range(T):
    Qm, R = np.linalg.qr(rng.standard_normal((nb, nb))); Qm = Qm * np.sign(np.diag(R))[None]; ok.append(Qm[0, 0])
print("after Q *= sign(diag R): share Q[0,0] < 0:", (np.array(ok) < 0).mean())
