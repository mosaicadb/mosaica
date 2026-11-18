# Synthetic check for policy.md section 11 (the revision after the replay): a table that is right in the pool and
# wrong per query by a common factor. Score-level simulation, nothing timed.
#   taskset -c 0-2 python3 -I policy_check2.py
# 40 groups x 10 clusters of 2,000 rows; the model knows (mu_c, sigma_c); the truth of query q has sigma_c x gamma_q,
# ln gamma_q ~ N(0, 0.1^2), which moves the count of rows above a tail threshold by a per-query factor kappa_q.
# Arms differ only in the values given to the plan (groups with a setup cost, best-first, stop at a price).
import numpy as np
from multiprocessing import Pool
from scipy.special import ndtr

G_, CPG, NROW, KNN, TOP = 40, 10, 2000, 10, 200
C_ROW, C_CL, C_GRP = 0.03, 8.0, 40.0
Kc = G_ * CPG; grp_of = np.repeat(np.arange(G_), CPG); WC = C_CL + C_ROW * NROW
NQ = 150


def sf(x):
    return ndtr(-x)


def best_action(v, opened, done):
    best = (-1.0, None)
    dens = v / WC
    for g in range(G_):
        ids = np.flatnonzero((grp_of == g) & ~done)
        if len(ids) == 0:
            continue
        if opened[g]:
            i = ids[np.argmax(dens[ids])]
            if dens[i] > best[0]:
                best = (dens[i], i)
        else:
            o = ids[np.argsort(-dens[ids])]
            r = np.cumsum(v[o]) / (C_GRP + WC * np.arange(1, len(o) + 1))
            if r.max() > best[0]:
                best = (float(r.max()), o[0])
    return best


def root(f, lo, hi):
    for _ in range(40):
        mid = (lo + hi) / 2
        lo, hi = (mid, hi) if f(mid) > 0 else (lo, mid)
    return (lo + hi) / 2


def make(seed):
    rng = np.random.default_rng(seed)
    mu = np.repeat(rng.normal(0, 0.8, G_), CPG) + rng.normal(0, 0.6, Kc)
    sg = 0.3 * np.exp(rng.normal(0, 0.3, Kc))
    gam = float(np.exp(rng.normal(0, 0.1)))
    top = np.sort(mu[:, None] + gam * sg[:, None] * rng.normal(size=(Kc, NROW)), axis=1)[:, -TOP:]
    return mu, sg, gam, top


def static_curve(v, top, tau_true, lams, denom):
    """One greedy pass on fixed values; for each price, recall and cost where the best index first falls to it."""
    done = np.zeros(Kc, bool); opened = np.zeros(G_, bool); cost = 0.0; hit = 0
    hits = (top >= tau_true).sum(1)
    out = []; li = 0
    while li < len(lams):
        idx, i = best_action(v, opened, done)
        while li < len(lams) and (i is None or idx <= lams[li]):
            out.append((hit / denom, cost)); li += 1
        if i is None or li >= len(lams):
            break
        g = grp_of[i]
        if not opened[g]:
            opened[g] = True; cost += C_GRP
        done[i] = True; cost += WC; hit += hits[i]
    while len(out) < len(lams):
        out.append((hit / denom, cost))
    return out


def adaptive(mode, lam, mu, sg, top, tau_true, a_prior):
    done = np.zeros(Kc, bool); opened = np.zeros(G_, bool); cost = 0.0
    lo, hi = mu.min() - 6, mu.max() + 6
    while True:
        found = top[done].ravel()
        if mode == "epochs":
            tau = root(lambda t: (found >= t).sum() + NROW * sf((t - mu[~done]) / sg[~done]).sum() - KNN, lo, hi)
            v = NROW * sf((tau - mu) / sg)
        else:  # kappa: a per-query factor learned from the units read, Gamma(a, a) prior
            def kap(t):
                return (a_prior + (found >= t).sum()) / (a_prior + NROW * sf((t - mu[done]) / sg[done]).sum())
            tau = root(lambda t: (found >= t).sum() + kap(t) * NROW * sf((t - mu[~done]) / sg[~done]).sum() - KNN, lo, hi)
            v = kap(tau) * NROW * sf((tau - mu) / sg)
        idx, i = best_action(v, opened, done)
        if i is None or idx <= lam:
            break
        g = grp_of[i]
        if not opened[g]:
            opened[g] = True; cost += C_GRP
        done[i] = True; cost += WC
    return (top[done] >= tau_true).sum() / KNN, cost


LAMS = [10 ** x for x in np.linspace(-1.5, -4.5, 13)]


def one(seed):
    mu, sg, gam, top = make(seed)
    tau_true = np.sort(top.ravel())[-KNN]
    lo, hi = mu.min() - 6, mu.max() + 6
    tau_star = root(lambda t: NROW * sf((t - mu) / sg).sum() - KNN, lo, hi)
    vt = NROW * sf((tau_true - mu) / sg)
    kappa = KNN / vt.sum()
    res = {"tau* once (shares)": static_curve(NROW * sf((tau_star - mu) / sg), top, tau_true, LAMS, KNN),
           "true tau, raw values": static_curve(vt, top, tau_true, LAMS, KNN),
           "true tau, shares": static_curve(KNN * vt / vt.sum(), top, tau_true, LAMS, KNN),
           "oracle (true sigma, true tau)": static_curve(NROW * sf((tau_true - mu) / (gam * sg)), top, tau_true, LAMS, KNN),
           "clairvoyant (true counts)": static_curve((top >= tau_true).sum(1).astype(float), top, tau_true, LAMS, KNN),
           "re-solved after every read": [adaptive("epochs", l, mu, sg, top, tau_true, 0) for l in LAMS],
           "kappa learned (a = 0.3)": [adaptive("kappa", l, mu, sg, top, tau_true, 0.3) for l in LAMS],
           "kappa learned (a = 3)": [adaptive("kappa", l, mu, sg, top, tau_true, 3.0) for l in LAMS]}
    # threshold query: t for an answer of 100 rows; values as counts against values as shares
    t = np.sort(top.ravel())[-100]; A = (top >= t).sum()
    vc = NROW * sf((t - mu) / sg)
    thr = {"counts n P(t), price on answers": static_curve(vc / 100 * KNN, top, t, LAMS, A),
           "shares n P(t) / Lambda(t), price on recall": static_curve(KNN * vc / vc.sum(), top, t, LAMS, A),
           "clairvoyant shares": static_curve(KNN * (top >= t).sum(1) / A, top, t, LAMS, A)}
    return np.log(kappa), res, thr, np.log(100 / vc.sum())


def at(curve, r):
    rec, cost = curve[:, 0], curve[:, 1]
    j = np.searchsorted(rec, r)
    if j >= len(rec):
        return float("nan")
    if j == 0:
        return cost[0]
    return cost[j - 1] + (cost[j] - cost[j - 1]) * (r - rec[j - 1]) / max(rec[j] - rec[j - 1], 1e-12)


if __name__ == "__main__":
    with Pool(3) as p:
        out = p.map(one, range(NQ))
    lk = np.array([o[0] for o in out]); lk2 = np.array([o[3] for o in out])
    print(f"{NQ} queries. ln kappa at k = 10: mean {lk.mean():+.2f}, sd {lk.std():.2f}, 10% to 90% [{np.quantile(lk, .1):+.2f}, {np.quantile(lk, .9):+.2f}]; "
          f"at answers of 100: sd {lk2.std():.2f}")
    print("k-NN, k = 10: mean cost (microseconds) at mean recall 0.8 / 0.9 / 0.95, one global price swept")
    base = None
    for name in out[0][1]:
        c = np.mean([np.array(o[1][name]) for o in out], axis=0)
        row = [at(c, r) for r in (0.8, 0.9, 0.95)]
        base = base or row
        print(f"   {name:34s} {row[0]:7.0f} {row[1]:7.0f} {row[2]:7.0f}   relative to the first: {row[0] / base[0]:.2f} {row[1] / base[1]:.2f} {row[2] / base[2]:.2f}")
    print("threshold queries, answers of 100 rows: mean cost at mean recall 0.8 / 0.9 / 0.95")
    base = None
    for name in out[0][2]:
        c = np.mean([np.array(o[2][name]) for o in out], axis=0)
        row = [at(c, r) for r in (0.8, 0.9, 0.95)]
        base = base or row
        print(f"   {name:44s} {row[0]:7.0f} {row[1]:7.0f} {row[2]:7.0f}   relative to the first: {row[0] / base[0]:.2f} {row[1] / base[1]:.2f} {row[2] / base[2]:.2f}")
