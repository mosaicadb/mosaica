# The price of one basis for a mixture of query families (CLAIMS.md, B4), on the set-up of the block-norm experiment
# (notes/program/stage0-block-norms: 200k BioCLIP-2, K = 200, G = 12, its fitted Sigma_Q for items, text and the mix).
# Reads that experiment's setup.npz and the 200k vectors (read-only); writes only in this directory.
#   stage 1: the residual covariance of each group, as stage0-block-norms/measure.py computes it (cached, float32, 28 MB)
#   stage 2: for a basis fitted on Sigma_w = (1-w) S_item/tr + w S_text/tr, the second-moment prediction of the mean
#            half-width of one weighted norm, sqrt(sum D^2 P_test) * sqrt(sum lam / D^2), per kind of test query,
#            against the widths measured there (w = 0 is its bound "a", w = 1/2 its bound "b2"); regrets against each
#            family's own optimum F; the angle delta between the families; the model of block norms.
# Run:  nice -n 10 taskset -c 6 env OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 python3 mixture_price.py
import math
import os
import sys
import numpy as np
from scipy import integrate

HERE = os.path.dirname(os.path.abspath(__file__))
BN = os.path.abspath(os.path.join(HERE, "..", "stage0-block-norms", "work"))
DD = "/home/joe/code/vexact-data/bioclip2-200k/"
CACHE = os.path.join(HERE, "work_sigmaR.npz")
EPS = 0.1
s0 = np.load(os.path.join(BN, "setup.npz"))
a, Mean, grp, cnt, Q, kind, OFF = s0["a"], s0["Mean"], s0["grp"], s0["cnt"], s0["Q"], s0["kind"], s0["OFF"]
Si, St = s0["Si"], s0["St"]
N, n, G = len(a), Q.shape[1], int(grp.max()) + 1
KINDS = ["text held-out", "text in-sample species", "items"]
KS = [0, 32, 256]
# measured mean half-widths (stage0-block-norms/work/report_measure.txt), bound a, b1, b2 at k = 0 / 32 / 256
MEAS = {0: {"a": (0.2968, 0.1779, 0.0256), "b1": (0.2691, 0.1669, 0.0250), "b2": (0.2338, 0.1302, 0.0172)},
        1: {"a": (0.2492, 0.1332, 0.0173), "b1": (0.2568, 0.1328, 0.0171), "b2": (0.2469, 0.1346, 0.0171)},
        2: {"a": (0.1982, 0.0862, 0.0077), "b1": (0.2305, 0.0912, 0.0079), "b2": (0.2092, 0.0934, 0.0092)}}


def stage1():
    F = [np.load(DD + "g%d.npy" % i, mmap_mode="r") for i in range(4)]
    SR = np.zeros((G, n, n)); Ng = np.zeros(G)
    for fi in range(4):
        for s in range(0, len(F[fi]), 4000):                      # small chunks: little memory traffic
            X = np.asarray(F[fi][s:s + 4000], dtype=np.float64); X /= np.linalg.norm(X, axis=1, keepdims=True)
            aa = a[OFF[fi] + s: OFF[fi] + s + len(X)]; R = X - Mean[aa]; gg = grp[aa]
            for g in np.unique(gg):
                Rg = R[gg == g]; SR[g] += Rg.T @ Rg; Ng[g] += len(Rg)
    SR /= Ng[:, None, None]
    np.savez(CACHE, SR=SR.astype(np.float32), Ng=Ng)


def ridge(S): return S + EPS * np.trace(S) / n * np.eye(n)


def sqrtm_pair(S):
    l, v = np.linalg.eigh(S); l = np.maximum(l, 1e-300); return (v * np.sqrt(l)) @ v.T, (v / np.sqrt(l)) @ v.T


def fid(SQ, SR):
    l, v = np.linalg.eigh(SQ); h = (v * np.sqrt(np.maximum(l, 0))) @ v.T
    w = np.linalg.eigvalsh(h @ SR @ h); return float(np.sqrt(np.maximum(w, 0)).sum())


def basis(Sfit, SR):
    """The dual pair of stage0-block-norms/measure.py for a fitted Sigma_Q, and the profile weights D from the same fit."""
    Aq, Aqi = sqrtm_pair(ridge(Sfit))
    gam, V = np.linalg.eigh(Aq @ SR @ Aq); gam = np.maximum(gam[::-1], 1e-14 * gam.max()); V = V[:, ::-1]
    VD = (Aq @ V) * gam ** -0.25; VQ = (Aqi @ V) * gam ** 0.25
    lam = np.einsum("ij,jk,ki->i", VD.T, SR, VD)
    return VQ, lam, gam


def profile(VQ, S): return np.maximum(np.einsum("ij,jk,ki->i", VQ.T, S, VQ), 1e-30)


def width(D, P, lam, k): return math.sqrt((D[k:] ** 2 * P[k:]).sum() * (lam[k:] / D[k:] ** 2).sum())


def mean_sqrt_Q(w):
    f = lambda s: (1 - math.exp(-0.5 * np.log1p(2 * math.exp(s) * w).sum())) * math.exp(-0.5 * s)
    return integrate.quad(f, -60, 60, limit=400)[0] / (2 * math.sqrt(math.pi))


if __name__ == "__main__":
    if not os.path.exists(CACHE):
        stage1()
    z = np.load(CACHE); SRs = z["SR"].astype(np.float64); Ng = z["Ng"]
    print("groups: sizes", Ng.astype(int).tolist())
    # second moments of the test queries, per kind, and of their offsets per group (Delta = P_c q, P_c the projection off the centre)
    Sk = [Q[kind == kk].T @ Q[kind == kk] / (kind == kk).sum() for kk in range(3)]
    def offsets_cov(S, g):
        out = np.zeros((n, n)); tot = 0.0
        for c in np.flatnonzero(grp == g):
            m = Mean[c]; mt = m / (m @ m); Sm_ = S @ mt
            out += cnt[c] * (S - np.outer(m, Sm_) - np.outer(Sm_, m) + (mt @ Sm_) * np.outer(m, m)); tot += cnt[c]
        return out / tot
    SQtest = [[offsets_cov(Sk[kk], g) for kk in range(3)] for g in range(G)]
    Sin, Stn = Si / np.trace(Si), St / np.trace(St)
    WS = [0.0, 0.05, 0.1, 0.2, 0.35, 0.5, 0.7, 0.85, 1.0]
    # pred[w][kind][k] = sum_g N_g * width; own[kind] = sum_g N_g F(test second moment, SR)
    pred = {w: np.zeros((3, len(KS))) for w in WS}; own = np.zeros(3); b1 = np.zeros((3, len(KS)))
    # the same evaluated on the fits themselves (item fit, text fit): what can be computed before any test query is seen
    fitp = {w: np.zeros((2, len(KS))) for w in WS}; fitown = np.zeros(2)
    delta = []; Fpair = []
    for g in range(G):
        SR = SRs[g]
        for w in WS:
            Sfit = (1 - w) * Sin + w * Stn
            VQ, lam, gam = basis(Sfit, SR)
            D = (lam / profile(VQ, Sfit)) ** 0.25
            for kk in range(3):
                P = profile(VQ, SQtest[g][kk])
                for j, k in enumerate(KS): pred[w][kk, j] += Ng[g] * width(D, P, lam, k)
            for ff, Sfam in enumerate((Si, St)):
                P = profile(VQ, Sfam)
                for j, k in enumerate(KS): fitp[w][ff, j] += Ng[g] * width(D, P, lam, k)
            if w == 0.0:                                          # bound b1: the data-only basis with the text-fitted profile
                Db1 = (lam / profile(VQ, St)) ** 0.25
                for kk in range(3):
                    P = profile(VQ, SQtest[g][kk])
                    for j, k in enumerate(KS): b1[kk, j] += Ng[g] * width(Db1, P, lam, k)
        for kk in range(3): own[kk] += Ng[g] * fid(SQtest[g][kk], SR)
        fitown += Ng[g] * np.array([fid(Si, SR), fid(St, SR)])
        # the angle between the two fitted families seen through SR, at equal weights of the trace-normalised fits
        f1, f2, fm = fid(Sin, SR), fid(Stn, SR), fid(0.5 * Sin + 0.5 * Stn, SR)
        delta.append((fm ** 2 - 0.5 * f1 ** 2 - 0.5 * f2 ** 2) / (f1 * f2)); Fpair.append((f1, f2, fm))
        print("  group %2d done: F(item fit) %.4f, F(text fit) %.4f, F(mix) %.4f, delta %.4f" % (g, f1, f2, fm, delta[-1])); sys.stdout.flush()
    Ntot = Ng.sum()
    print("\nsecond-moment prediction of the mean half-width (all groups, weighted by size) against the measurement; k = 0 / 32 / 256")
    for kk in range(3):
        pa, pb2, pb1 = pred[0.0][kk] / Ntot, pred[0.5][kk] / Ntot, b1[kk] / Ntot
        print("  %-24s a: predicted %s measured %s" % (KINDS[kk], np.round(pa, 4).tolist(), list(MEAS[kk]["a"])))
        print("  %-24s b2/a (mixed basis): predicted %s measured %s" % ("", np.round(pb2 / pa, 3).tolist(), [round(x / y, 3) for x, y in zip(MEAS[kk]["b2"], MEAS[kk]["a"])]))
        print("  %-24s b1/a (text weights, data basis): predicted %s measured %s" % ("", np.round(pb1 / pa, 3).tolist(), [round(x / y, 3) for x, y in zip(MEAS[kk]["b1"], MEAS[kk]["a"])]))
    print("\nthe trade-off: basis fitted on (1-w) item + w text (trace-normalised); predicted width / width of the data-only basis (w = 0), k = 0 / 32 / 256,")
    print("and at k = 0 the regret against the family's own optimum F(test second moment, Sigma_R)")
    for w in WS:
        row = "  w = %.2f:" % w
        for kk in range(3):
            r = pred[w][kk] / pred[0.0][kk]
            row += "  %s %.3f/%.3f/%.3f (regret %.3f)" % (("text", "species", "items")[kk], r[0], r[1], r[2], pred[w][kk, 0] / own[kk])
        print(row)
    print("\nthe same from the two fits alone (no test query): width on the item fit and on the text fit / width at w = 0, k = 0 / 32 / 256,")
    print("and at k = 0 the regret against F(fit, Sigma_R)")
    for w in WS:
        row = "  w = %.2f:" % w
        for ff in range(2):
            r = fitp[w][ff] / fitp[0.0][ff]
            row += "  %s %.3f/%.3f/%.3f (regret %.3f)" % (("item fit", "text fit")[ff], r[0], r[1], r[2], fitp[w][ff, 0] / fitown[ff])
        print(row)
    d = np.array(delta); Fp = np.array(Fpair)
    print("\nangle between the fitted families, delta = (F_mix^2 - F_1^2/2 - F_2^2/2)/(F_1 F_2) in [0, 1]: min %.4f, size-weighted mean %.4f, max %.4f"
          % (d.min(), float((d * Ng).sum() / Ntot), d.max()))
    print("mean-square price of one basis for the equal mixture, F_mix^2 / (F_1^2/2 + F_2^2/2): min %.4f, mean %.4f, max %.4f (bounds 1 and 2)"
          % tuple(f(Fp[:, 2] ** 2 / (0.5 * Fp[:, 0] ** 2 + 0.5 * Fp[:, 1] ** 2)) for f in (np.min, lambda x: float((x * Ng).sum() / Ntot), np.max)))
    # the model of block norms on Gaussian coordinates: per-coordinate limit 2/pi (1 + cv^2); J equal-share blocks
    print("\nblock norms on the data-only basis, items, k = 0: Gaussian model against the measurement (cvar J: .98 / .96 / .94 / .89; d: .63)")
    acc = {J: 0.0 for J in (2, 4, 8, 16)}; accd = 0.0; neffs = []
    for g in range(G):
        gam = np.load(os.path.join(BN, "g%02d.npz" % g))["gam_d"]; lam = np.sqrt(gam); F = lam.sum(); neffs.append(F * F / gam.sum())
        one = mean_sqrt_Q(lam) ** 2
        accd += Ng[g] * (2 / math.pi) * F / one
        cs = np.concatenate([[0], np.cumsum(lam)])
        for J in acc:
            e = np.concatenate([[0], np.searchsorted(cs, np.linspace(0, cs[-1], J + 1)[1:-1]), [len(lam)]])
            for i in range(1, J): e[i] = min(max(e[i], e[i - 1] + 1), len(lam) - (J - i))
            acc[J] += Ng[g] * sum(mean_sqrt_Q(lam[e[i]:e[i + 1]]) ** 2 for i in range(J)) / one
    print("  n_eff of the groups (data-only gamma): min %.0f, median %.0f, max %.0f" % (min(neffs), float(np.median(neffs)), max(neffs)))
    print("  predicted per-coordinate limit / one norm: %.3f;  J = 2 / 4 / 8 / 16 equal-share blocks / one norm: %s"
          % (accd / Ntot, " / ".join("%.3f" % (acc[J] / Ntot) for J in (2, 4, 8, 16))))
