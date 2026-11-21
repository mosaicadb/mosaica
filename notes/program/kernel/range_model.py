"""A numpy/float64 model of the cluster cut and the sorted range as built (rust/src/v2/q.rs and mod.rs at 8c0b316f),
written from a reading of the code, for the referee's attack. Python floats are IEEE doubles, so the query side is the
code's arithmetic line for line; the build side differs only in summation order.
"""
import math
from fractions import Fraction
import numpy as np

DA = 0.5 / 32767.0 + 1e-7
PI = math.pi


def f16(v):
    return np.asarray(v, dtype=np.float64).astype(np.float16).astype(np.float64)


def bound_up(v):
    g = math.ceil((v + 1e-9) * 32768.0) / 32768.0
    h = float(np.float32(g))
    return float(np.nextafter(np.float32(h), np.float32(np.inf))) if h < g else h


def bound_down(v):
    g = math.floor((v - 1e-9) * 32768.0) / 32768.0
    l = float(np.float32(g))
    return float(np.nextafter(np.float32(l), np.float32(-np.inf))) if l > g else l


class Index:
    """clusters: list of (centre m as f32-valued f64 array, rows X as f16-valued f64 array [nc, n]).
    anchors: None (the centre directions) or an array [A, n] of unit rows.
    clamp: the build's clamp of the key to [-1, 1] (True as built)."""

    def __init__(self, clusters, anchors=None, clamp=True):
        self.cl = []
        x2max = 0.0
        for m, X in clusters:
            m = np.asarray(m, dtype=np.float32).astype(np.float64)
            mn = math.sqrt(float(np.dot(m, m)))
            mhat = m / mn
            a = X @ mhat
            ac = np.clip(a, -1.0, 1.0) if clamp else a
            key = np.round(ac * 32767.0).astype(np.int64)
            order = np.lexsort((np.arange(len(key)), key))
            X, a, key = X[order], a[order], key[order]
            x2max = max(x2max, float(np.max(np.sum(X * X, axis=1) - 1.0)))
            self.cl.append(dict(m=m, mn=mn, mhat=mhat, X=X, a=a, key=key))
        self.x2 = 1.0 + max(x2max, 0.0) * (1.0 + 1e-9) + 1e-12
        self.anchors = np.array([c["mhat"] for c in self.cl]) if anchors is None else np.asarray(anchors, dtype=np.float64)
        A, K = len(self.anchors), len(self.cl)
        self.hi = np.zeros((A, K))
        self.lo = np.zeros((A, K))
        self.apm = np.zeros((A, K))
        for ai in range(A):
            for ci, c in enumerate(self.cl):
                px = c["X"] @ self.anchors[ai]
                self.hi[ai, ci] = bound_up(float(px.max()))
                self.lo[ai, ci] = bound_down(float(px.min()))
                self.apm[ai, ci] = float(np.dot(self.anchors[ai], c["mhat"]))


def prep(ix, q, anchor=None):
    """q is rounded to f16 as the code does. The anchor is the one of largest q.p unless given (any anchor is used
    by the code when the search is restricted to the live clusters)."""
    q = f16(q)
    qn = math.sqrt(float(np.sum(q * q)))
    betas = ix.anchors @ q
    a = int(np.argmax(betas)) if anchor is None else anchor
    beta = float(betas[a])
    rho = math.sqrt(max(qn * qn - beta * beta, 0.0)) * (1.0 + 1e-12) + 1e-12
    return dict(q=q, qn=qn, a=a, beta=beta, rho=rho)


def cut_params(ix, ci, pr):
    c = ix.cl[ci]
    qm = float(np.dot(pr["q"], c["m"]))
    cm = qm / c["mn"]
    s1 = math.sqrt(max(pr["qn"] ** 2 - cm * cm, 0.0)) * (1.0 + 1e-12) + 1e-12
    a, b, rho = pr["a"], pr["beta"], pr["rho"]
    hi, lo = ix.hi[a, ci], ix.lo[a, ci]
    bh, bl = (b * hi, b * lo) if b >= 0.0 else (b * lo, b * hi)
    rm = cm - b * ix.apm[a, ci]
    rp = math.sqrt(max(rho * rho - rm * rm, 0.0)) * (1.0 + 1e-12) + 1e-12
    return dict(cm=cm, s1=s1, bh=bh, bl=bl, rm=rm, rp=rp)


def sup_set(p, r, rr, t):
    qn = math.sqrt(p * p + r * r)
    if qn == 0.0:
        return (-rr, rr) if t <= 0.0 else None
    tau = t / (qn * rr)
    if tau > 1.0:
        return None
    if tau <= -1.0:
        return (-rr, rr)
    phi = math.atan2(r, p)
    psi = math.acos(tau)
    lo = max(phi - psi, 0.0)
    hi = min(phi + psi, PI)
    if lo > hi:
        return None
    return (rr * math.cos(hi), rr * math.cos(lo))


def inf_set(p, r, rr, t):
    qn = math.sqrt(p * p + r * r)
    if qn == 0.0:
        return (-rr, rr) if 0.0 < t else None
    tau = t / (qn * rr)
    if tau > 1.0:
        return (-rr, rr)
    if tau <= -1.0:
        return None
    phi = math.atan2(r, p)
    psi = math.acos(tau)
    lo = max(psi - phi, 0.0)
    hi = min(2.0 * PI - psi - phi, PI)
    if lo >= hi:
        return None
    return (rr * math.cos(hi), rr * math.cos(lo))


def isect(a, b):
    if a is None or b is None:
        return None
    lo, hi = max(a[0], b[0]), min(a[1], b[1])
    return (lo, hi) if lo <= hi else None


def sorted_range(ix, ci, cu, t, widen=None):
    """(u0, u1, d0, d1) as offsets in the cluster: [0,u0) and [u1,nc) out, [u0,d0) and [d1,u1) in, [d0,d1) undecided."""
    c = ix.cl[ci]
    rr = math.sqrt(ix.x2)
    eps = (1e-9 + DA) if widen is None else widen
    uset = isect(sup_set(cu["cm"], cu["s1"], rr, t - 1e-9), sup_set(cu["rm"], cu["rp"], rr, t - cu["bh"] - 1e-9))
    lset = isect(inf_set(cu["cm"], cu["s1"], rr, t + 1e-9), inf_set(cu["rm"], cu["rp"], rr, t - cu["bl"] + 1e-9))
    keyf = c["key"] / 32767.0

    def rows(iv):
        lo, hi = iv[0] - eps, iv[1] + eps
        a = int(np.searchsorted(keyf, lo, side="left"))     # first key not < lo
        b = int(np.searchsorted(keyf, hi, side="right"))    # first key not <= hi
        return a, max(a, b)

    if uset is None:
        return (0, 0, 0, 0)
    u0, u1 = rows(uset)
    if lset is None:
        return (u0, u1, u0, u0)
    l0, l1 = rows(lset)
    d0, d1 = max(l0, u0), min(l1, u1)
    return (u0, u1, u0, u0) if d0 >= d1 else (u0, u1, d0, d1)


def support_pre(ix, ci, pr, t):
    """'skip', 'in' or None, as q.rs support_pre."""
    rr = math.sqrt(ix.x2)
    a, b, rho = pr["a"], pr["beta"], pr["rho"]
    hi, lo = ix.hi[a, ci], ix.lo[a, ci]
    bh, bl = (b * hi, b * lo) if b >= 0.0 else (b * lo, b * hi)
    if bh + rho * rr < t - 1e-9:
        return "skip"
    if bl - rho * rr >= t + 1e-9:
        return "in"
    return None


def exact_scores(q, X):
    """Correctly rounded q.x (products of f16 values are exact doubles), and an exact comparison with t."""
    return np.array([math.fsum((q * x).tolist()) for x in X])


def ge_exact(q, x, t):
    s = sum(Fraction(float(a)) * Fraction(float(b)) for a, b in zip(q, x))
    return s >= Fraction(t)


def decisions(ix, ci, pr, t, widen=None):
    """Per row of the cluster: +1 decided in, -1 decided out, 0 undecided, from the cut and then the range."""
    nc = len(ix.cl[ci]["key"])
    pre = support_pre(ix, ci, pr, t)
    if pre == "skip":
        return np.full(nc, -1), "skip"
    if pre == "in":
        return np.full(nc, 1), "in"
    u0, u1, d0, d1 = sorted_range(ix, ci, cut_params(ix, ci, pr), t, widen)
    d = np.full(nc, -1)
    d[u0:u1] = 1
    d[d0:d1] = 0
    return d, "open"


def wrong_rows(ix, ci, pr, t, d):
    """Rows decided against the exact score."""
    c = ix.cl[ci]
    s = exact_scores(pr["q"], c["X"])
    bad = []
    for i in np.nonzero(d != 0)[0]:
        near = abs(s[i] - t) < 1e-12
        isin = ge_exact(pr["q"], c["X"][i], t) if near else s[i] >= t
        if (d[i] == 1) != isin:
            bad.append((int(i), float(s[i]), int(d[i])))
    return bad


def real_bounds(ix, ci, pr, a):
    """The real-valued bounds U = min(U1, U2), L = max(L1, L2) at keys a (float64, no slack, no rounding up)."""
    c = ix.cl[ci]
    q, mhat = pr["q"], c["mhat"]
    p = ix.anchors[pr["a"]]
    x2 = ix.x2
    cm = float(np.dot(q, mhat))
    qmv = q - cm * mhat
    s1 = math.sqrt(float(np.dot(qmv, qmv)))
    beta = float(np.dot(q, p))
    qp = q - beta * p
    rm = float(np.dot(qp, mhat))
    qpm = qp - rm * mhat
    rp = math.sqrt(float(np.dot(qpm, qpm)))
    px = c["X"] @ p
    bh, bl = max(beta * px.max(), beta * px.min()), min(beta * px.max(), beta * px.min())
    w = np.sqrt(np.maximum(x2 - a * a, 0.0))
    U = np.minimum(cm * a + s1 * w, bh + rm * a + rp * w)
    L = np.maximum(cm * a - s1 * w, bl + rm * a - rp * w)
    return L, U
