# Candidate E: E8.
#  (a) A check against the published 2-bit figure (QuIP#'s E8P, MSE 0.089 in QTIP's Table 1): the 65,536 points of
#      least norm of E8 + (1/4, ..., 1/4), scaled, nearest-point encoding. This codebook is ball-shaped and is NOT a sum
#      of nibble tables; it is here to check the simulation and to show what shaping adds.
#  (b) The table-decodable form: per 8 positions, the upper bit planes as today (one-bit fields, 4 to a nibble) and the
#      lowest plane one nibble that indexes the [8,4,4] extended Hamming code. Rate b - 1/2 bits per position for b
#      planes. Cube-shaped, per-item scale. One lookup per nibble; the Hamming nibble's table covers 8 positions.
#  (c) D4 the same way: the lowest plane of 4 positions from 3 bits (even weight), rate b - 1/4. A 3-bit field shares
#      its nibble with a one-bit field of another position.
import itertools
import math
import sys
import numpy as np
from clib import *

G = np.array([[1, 1, 1, 1, 0, 0, 0, 0], [1, 1, 0, 0, 1, 1, 0, 0], [1, 0, 1, 0, 1, 0, 1, 0], [1, 1, 1, 1, 1, 1, 1, 1]])
HAM = np.array([(np.array(c) @ G) % 2 for c in itertools.product([0, 1], repeat=4)], dtype=np.float64)   # 16 x 8
SPC = np.array([c for c in itertools.product([0, 1], repeat=4) if sum(c) % 2 == 0], dtype=np.float64)    # 8 x 4


def e8_ball(npts=65536, shift=0.25, cut=11.6):
    """The npts points of least norm of E8 + shift (E8 = integer or half-integer vectors with even sum)."""
    out = []
    for half in (0.0, 0.5):
        vals = np.arange(-4, 4) + half
        part = np.zeros((1, 0))
        for _ in range(8):
            ext = np.concatenate([np.concatenate([part, np.full((len(part), 1), v)], axis=1) for v in vals])
            n2 = ((ext + shift) ** 2).sum(1)
            part = ext[n2 <= cut]
        s = part.sum(1)
        part = part[np.abs(s / 2 - np.round(s / 2)) < 1e-9]
        out.append(part + shift)
    P = np.concatenate(out)
    n2 = (P * P).sum(1)
    order = np.argsort(n2, kind="stable")[:npts]
    return P[order], float(n2[order[-1]]), len(P)


def encode_codebook(X, C, chunk=100):
    """Nearest codeword of C (K x k) for rows of X (float32 work, chunked)."""
    C32 = C.astype(np.float32)
    cn = (C32 * C32).sum(1)
    idx = np.zeros(len(X), dtype=np.int64)
    for i in range(0, len(X), chunk):
        idx[i: i + chunk] = (cn[None, :] - 2.0 * X[i: i + chunk].astype(np.float32) @ C32.T).argmin(1)
    return idx


def lattice_plane(Y, planes, code, lams):
    """(b), (c): per-item scale lam in `lams`; a position's value is the cell midpoint of `planes` bits on [-lam, lam)
    whose lowest bits over each sub-block form a word of `code` (rows of 0/1). Returns the best tan^2 per item."""
    nv, m = Y.shape
    k = code.shape[1]
    nb = m // k
    rms = np.sqrt((Y * Y).mean(1, keepdims=True))
    best = np.full(nv, np.inf)
    half = 2.0 ** (planes - 1)
    for lam in lams:
        v = (Y / (lam * rms))[:, : nb * k].reshape(nv * nb, 1, k)   # in units where the grid is on [-1, 1)
        u = v * half + half                                          # cell coordinate in [0, 2^planes)
        # for each code word h: nearest integer cell index q with q = h mod 2, clamped to the range
        h = code[None, :, :]
        q = 2.0 * np.round((u - 0.5 - h) / 2.0) + h
        q = np.clip(q, h, 2.0 * half - 2.0 + h)
        err = ((u - 0.5 - q) ** 2).sum(2)
        pick = err.argmin(1)
        qb = np.take_along_axis(q, pick[:, None, None], 1)[:, 0, :]
        val = (qb + 0.5) / half - 1.0
        c = np.zeros_like(Y)
        c[:, : nb * k] = val.reshape(nv, nb * k)
        best = np.minimum(best, tan2(c, Y))
    return best


def built_mixed(Y, planes, share):
    """The built code with `share` of the positions at `planes` bits and the rest at planes - 1 (same rate as the
    lattice plane code)."""
    m = Y.shape[1]
    d = np.full(m, planes - 1, dtype=np.int64)
    d[: int(round(share * m))] = planes
    t2, _ = built_encode(Y, [d])
    return t2[:, 0]


def coset_reps():
    """16 representatives of Z^8 / (2Z^8 + Hamming), of least weight, with signs balanced."""
    seen, reps = {}, []
    cands = [np.zeros(8)]
    for i in range(8):
        e = np.zeros(8)
        e[i] = 1.0 if i % 2 == 0 else -1.0
        cands.append(e)
    for i in range(8):
        for j in range(i + 1, 8):
            e = np.zeros(8)
            e[i], e[j] = 1.0, -1.0
            cands.append(e)
    words = {tuple(h.astype(int)) for h in HAM}
    for c in cands:
        key = min(tuple(((np.abs(c).astype(int) + np.array(w)) % 2).tolist()) for w in words)
        if key not in seen:
            seen[key] = True
            reps.append(c)
    assert len(reps) == 16
    return np.array(reps)


def nested_e8(Y, planes, lams, emph):
    """(d) Two rises, planes - 1/2 and planes + 1/2 bits per position. The coarse code is the E8 plane code with
    `planes` planes; the next rise adds two nibbles per 8 positions: a coset representative r_k (which completes the
    coarse code's lowest plane) and a Hamming word for the new lowest plane, so the fine cell is 2 (q + r_k) + h'.
    Search: for each of the 16 coarse points (the nearest in each Hamming coset), all 256 refinements; the pair with
    the least emph[0] errA / dA + emph[1] errB / dB is kept. Returns tan^2 per item at the two rises."""
    nv, m = Y.shape
    nb = m // 8
    reps = coset_reps()
    rms = np.sqrt((Y * Y).mean(1, keepdims=True))
    hA, hB = 2.0 ** (planes - 1), 2.0 ** planes
    dA, dB = (1.0 / hA) ** 2 / 12 * 8, (1.0 / hB) ** 2 / 12 * 8     # nominal sub-block errors of the two grids
    best = np.full(nv, np.inf)
    out = np.zeros((nv, 2))
    for lam in lams:
        v = (Y / (lam * rms))[:, : nb * 8].reshape(nv * nb, 8)
        uA = v * hA + hA - 0.5                                        # coarse cell coordinate
        bestobj = np.full(nv * nb, np.inf)
        bA = np.zeros((nv * nb, 8))
        bB = np.zeros((nv * nb, 8))
        for h in HAM:
            qA = np.clip(2.0 * np.round((uA - h) / 2.0) + h, h, 2.0 * hA - 2.0 + h)
            vA = (qA + 0.5) / hA - 1.0
            eA = ((v - vA) ** 2).sum(1)
            eBmin = np.full(nv * nb, np.inf)
            vBbest = np.zeros((nv * nb, 8))
            for r in reps:
                base = 2.0 * (qA + r)
                vB = (base[:, None, :] + HAM[None, :, :] + 0.5) / hB - 1.0     # (sub-blocks, 16, 8)
                eB = ((v[:, None, :] - vB) ** 2).sum(2)
                k = eB.argmin(1)
                e = eB[np.arange(len(k)), k]
                bt = e < eBmin
                eBmin[bt] = e[bt]
                vBbest[bt] = vB[np.arange(len(k)), k][bt]
            obj = emph[0] * eA / dA + emph[1] * eBmin / dB
            bt = obj < bestobj
            bestobj[bt] = obj[bt]
            bA[bt] = vA[bt]
            bB[bt] = vBbest[bt]
        cA, cB = np.zeros_like(Y), np.zeros_like(Y)
        cA[:, : nb * 8] = bA.reshape(nv, nb * 8)
        cB[:, : nb * 8] = bB.reshape(nv, nb * 8)
        t2 = np.stack([tan2(cA, Y), tan2(cB, Y)], axis=1)
        o = np.log(t2).sum(1)
        bt = o < best
        best[bt] = o[bt]
        out[bt] = t2[bt]
    return out


if __name__ == "__main__":
    what = sys.argv[1:] or ["a", "b"]
    if "a" in what:
        C, top, total = e8_ball()
        print(f"(a) E8 + 1/4: {len(C)} points of least norm (largest norm^2 {top:.3f}; {total} points enumerated)")
        X = np.random.default_rng(7).standard_normal((4000, 8))
        best = None
        for s in (0.80, 0.85, 0.90, 0.95, 1.00, 1.05, 1.10):
            Cs = C / (math.sqrt((C * C).sum(1).mean() / 8) * s)   # rms per coordinate of the codebook = 1 / s
            idx = encode_codebook(X[:1500], Cs)
            D = float(((X[:1500] - Cs[idx]) ** 2).mean())
            if best is None or D < best[0]:
                best = (D, s)
        s = best[1]
        Cs = C / (math.sqrt((C * C).sum(1).mean() / 8) * s)
        idx = encode_codebook(X, Cs)
        D = float(((X - Cs[idx]) ** 2).mean())
        # centroid condition: the gain that minimises the error
        g = float((X * Cs[idx]).sum() / (Cs[idx] ** 2).sum())
        Dg = float(((X - g * Cs[idx]) ** 2).mean())
        print(f"    2 bits per position: MSE {D:.4f} at codebook scale {s}; with the best gain {Dg:.4f}; published E8P 0.089; "
              f"Lloyd-Max 0.1175; D(R) 0.0625")
        m = 256
        Y = sphere(120, m, 2000 + m)
        rms = np.sqrt((Y * Y).mean(1, keepdims=True))
        idx = encode_codebook((Y / rms).reshape(-1, 8), Cs)
        t = math.sqrt(tan2(Cs[idx].reshape(120, m), Y).mean())
        bt2, _ = built_encode(Y, [depths(2, m)])
        b = math.sqrt(bt2.mean())
        print(f"    blocks of {m}: tan {t:.4f}; built {b:.4f}; ratio {t / b:.3f}; / bound {t / math.sqrt(bound_tan2(2)):.3f}")
    if "b" in what:
        lams = [1.6, 1.8, 2.0, 2.3, 2.6, 3.0, 3.5, 4.2]
        for m in (96, 256):
            Y = sphere(600, m, 2000 + m)
            for name, code, loss in (("E8: 2Z^8 + Hamming nibble", HAM, 0.5), ("D4: 2Z^4 + even-weight 3 bits", SPC, 0.25)):
                for planes in (2, 3, 4, 5):
                    R = planes - loss
                    t = math.sqrt(lattice_plane(Y, planes, code, lams).mean())
                    b = math.sqrt(built_mixed(Y, planes, 1 - loss).mean())
                    bd = math.sqrt(bound_tan2(R))
                    print(f"(b) m={m} {name}, {planes} planes, R = {R}: tan {t:.4f}; built at the same rate (mixed depths) {b:.4f}; "
                          f"ratio {t / b:.3f}; / bound {t / bd:.3f}; built / bound {b / bd:.3f}", flush=True)
    if "d" in what:
        lams = [1.6, 1.8, 2.0, 2.3, 2.6, 3.0, 3.5, 4.2]
        for m in (96, 256):
            Y = sphere(250, m, 2000 + m)
            for planes in (2, 3):
                one = [math.sqrt(lattice_plane(Y, pl, HAM, lams).mean()) for pl in (planes, planes + 1)]
                bd = [math.sqrt(bound_tan2(pl - 0.5)) for pl in (planes, planes + 1)]
                for emph in ((1, 1), (3, 1), (1, 3), (10, 1), (1, 10)):
                    t = np.sqrt(nested_e8(Y, planes, lams, emph).mean(0))
                    print(f"(d) m={m} nested E8 plane code, rises {planes - 0.5} and {planes + 0.5}, emphasis {emph}: tan {t[0]:.4f} / {t[1]:.4f}; "
                          f"each rise alone {one[0]:.4f} / {one[1]:.4f}; nesting loss {t[0] / one[0]:.3f} / {t[1] / one[1]:.3f}; "
                          f"/ bound {t[0] / bd[0]:.3f} / {t[1] / bd[1]:.3f}", flush=True)
