# Checks behind under.py's tables:
#  1. groups among the first P clusters of the flat spread order (the groups whose bases must be set up before
#     sub-centres can be scored in group coordinates, and the groups a group-limited probe is confined to);
#  2. how much of a sub-centre's score offset q.(m_sub - m_parent) the first T coordinates of the group's basis
#     carry (R^2 over query x sub-cluster pairs, sub-clusters of the 64 clusters first in the flat order);
#  3. the items ratio flat / x16 at recall 0.9 on the even and the odd queries (how stable 100 queries are).
# usage: python3 -I extras.py DS
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *
import numpy as np

D = DS(sys.argv[1])
K, d, Q = D.K, D.d, D.Q
Q64 = Q.astype(np.float64)
G = int(D.grp.max()) + 1
cs = cached(D.ds, "st_real", lambda: leaf_stats(D.X, D.a, K, 16, D.Mq))
qmK = Q64 @ D.Mq.astype(np.float64).T
sdK = qmK.std(1)[:, None]
flat = qmK + 0.4 * sdK * (cs["rad"] / sdn(cs["rad"]))[None]
o = np.argsort(-flat, 1, kind="stable")
print(f"== {D.name}: groups among the first P clusters (flat spread .4), mean over queries")
for P in (8, 16, 32, 64, 128, 256):
    if P < K:
        print(f"P {P:4d}: {np.mean([len(set(D.grp[o[i, :P]])) for i in range(D.nq)]):5.1f} of {G}")

V = np.load(os.path.join(D.fit, "V.npy")).astype(np.float64)
n_pad = V.shape[1]
Qp = np.zeros((D.nq, n_pad)); Qp[:, :d] = Q64
z = subsplit(D, 16)
sub, par = z["sub"], z["par"]
st = cached(D.ds, "st_sub16", lambda: leaf_stats(D.X, sub, len(par), 4))
delta = np.zeros((len(par), n_pad)); delta[:, :d] = st["mean"].astype(np.float64) - D.Mq[par].astype(np.float64)
TS = [t for t in (8, 16, 32, 64, 128, 256) if t < d]
num = {t: 0.0 for t in TS}; den = 0.0
top = np.zeros((D.nq, K), bool)
np.put_along_axis(top, o[:, :64], True, 1)
for g in range(G):
    ix = np.where(D.grp[par] == g)[0]
    if not len(ix):
        continue
    A = delta[ix] @ np.linalg.inv(V[g]).T
    Pg = Qp @ V[g]
    m = top[:, par[ix]]
    full = Pg @ A.T
    den += (full[m] ** 2).sum()
    for t in TS:
        num[t] += ((full - Pg[:, :t] @ A[:, :t].T)[m] ** 2).sum()
print("R^2 of the truncated offset, x16 sub-centres: " + ", ".join(f"T {t}: {1 - num[t] / den:.3f}" for t in TS))
print(f"s.d. of the offset q.(m_sub - m_parent) {np.sqrt(den / top[:, par].sum()):.4f}; s.d. of q.m_c over clusters {sdK.mean():.4f}")

sets = D.truth_knn()
tot = np.full(D.nq, float(KNN))
qm = Q64 @ st["mean"].astype(np.float64).T
skey = qm + 0.4 * sdK * (st["rad"] / sdn(st["rad"]))[None]
for nm, sel in (("even", np.arange(0, D.nq, 2)), ("odd", np.arange(1, D.nq, 2)), ("all", np.arange(D.nq))):
    out = []
    for key, a, L in ((flat, D.a, K), (skey, sub, len(par))):
        r = run(key[sel], counts(a, L, [sets[i] for i in sel]), np.bincount(a, minlength=L).astype(np.float64))
        out.append(summarise(r, tot[sel], D.N))
    print(f"{nm:5s} queries: items at 0.9 / 0.95 flat " + " / ".join(f"{out[0]['at'][t]['items']:.0f}" for t in (0.9, 0.95)) +
          " x16 " + " / ".join(f"{out[1]['at'][t]['items']:.0f}" for t in (0.9, 0.95)) +
          " ratio " + " / ".join(f"{out[0]['at'][t]['items'] / out[1]['at'][t]['items']:.2f}" for t in (0.9, 0.95)))
