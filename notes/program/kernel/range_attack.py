"""Attack 1: random and adversarial clusters against the model of the cut and the sorted range (range_model.py).
For every (index, query, threshold, cluster): no row may be decided against its exact score. Also tallies the
arrangement of decided rows along the key (Editor's query 6) and checks Lemma 1.5 on the real-valued bounds.
usage: python3 range_attack.py [seed] [indexes]
"""
import math
import sys
from fractions import Fraction
import numpy as np
import range_model as rm

seed = int(sys.argv[1]) if len(sys.argv) > 1 else 1
nidx = int(sys.argv[2]) if len(sys.argv) > 2 else 40
# "built": eps = DA + 1e-9 as the code; "repair": eps = DA + 1e-9 + max(0, sqrt(x2) - 1) (finding F1)
mode = sys.argv[3] if len(sys.argv) > 3 else "built"
rng = np.random.default_rng(seed)


def unit(v):
    return v / np.linalg.norm(v, axis=-1, keepdims=True)


def make_index(n, K, nc, spread, dup, anchors_mode):
    cents = unit(rng.standard_normal((K, n)))
    if rng.random() < 0.3:      # centres close together
        cents = unit(cents[0] + 0.3 * rng.standard_normal((K, n)))
    clusters = []
    for k in range(K):
        sp = spread * rng.uniform(0.02, 1.5)
        X = unit(cents[k] + sp * rng.standard_normal((nc, n)) / math.sqrt(n))
        if dup:                   # equal rows, rows at the centre, antipodal rows, rows at equal keys
            X[: nc // 8] = X[0]
            X[nc // 8] = cents[k]
            X[nc // 8 + 1] = -cents[k]
            j = nc // 8 + 2
            # two rows with the same key and different residual directions
            a = float(X[j] @ cents[k])
            w = rng.standard_normal(n)
            w -= (w @ cents[k]) * cents[k]
            X[j + 1] = a * cents[k] + math.sqrt(max(1 - a * a, 0)) * unit(w)
        X = rm.f16(X)
        mean = X.mean(axis=0)
        sc = np.abs(mean).max() / 127.0
        m = (np.round(mean / sc) * sc).astype(np.float32)        # 8 bits a dimension
        if not np.any(m):
            m = cents[k].astype(np.float32)
        clusters.append((m, X))
    anchors = None
    if anchors_mode == 1:
        anchors = unit(rng.standard_normal((max(2, K // 2), n)))
    return rm.Index(clusters, anchors)


def queries(ix, n):
    out = []
    K = len(ix.cl)
    for _ in range(6):
        out.append(unit(rng.standard_normal(n)))
    for _ in range(6):
        c = ix.cl[rng.integers(K)]
        kind = rng.integers(9)
        if kind == 0:
            out.append(c["mhat"].copy())                         # the centre direction
        elif kind == 1:
            out.append(-c["mhat"])                               # negative centre score
        elif kind == 2:
            out.append(c["X"][rng.integers(len(c["X"]))].copy()) # a stored row
        elif kind == 3:
            out.append(unit(c["mhat"] + 10 ** rng.uniform(-7, -1) * rng.standard_normal(n)))
        elif kind == 4:
            c2 = ix.cl[rng.integers(K)]                          # in the plane of two centres
            out.append(unit(rng.uniform(-1, 1) * c["mhat"] + rng.uniform(-1, 1) * c2["mhat"] + 1e-9 * rng.standard_normal(n)))
        elif kind == 5:
            out.append(c["m"] / np.abs(c["m"]).max())            # the stored centre itself, not unit
        elif kind == 8:
            mi = np.round(c["m"] / np.abs(c["m"]).max() * 127.0)   # the centre's int8 vector, exact in f16 (finding F3)
            out.append(mi * 2.0 ** -math.ceil(math.log2(np.linalg.norm(mi))))
        elif kind == 6:
            out.append(unit(-c["X"][rng.integers(len(c["X"]))] + 0.01 * rng.standard_normal(n)))
        else:
            w = rng.standard_normal(n)
            w -= (w @ c["mhat"]) * c["mhat"]
            out.append(unit(w))                                  # orthogonal to the centre
    return out


tot = dict(checks=0, rows=0, decided=0, wrong=0, skip=0, allin=0, lemma_bad=0, ties_noncontig=0, wrong_unclamped=0, wrong_t_within_qn=0)
patterns = {}
worst = []
for it in range(nidx):
    n = int(rng.choice([3, 4, 8, 16, 32]))
    K = int(rng.choice([2, 4, 8]))
    nc = int(rng.choice([24, 64, 200]))
    ix = make_index(n, K, nc, rng.choice([0.3, 1.0, 3.0]), rng.random() < 0.5, int(rng.integers(2)))
    for q in queries(ix, n):
        for forced in (None, int(rng.integers(len(ix.anchors)))):
            pr = rm.prep(ix, q, forced)
            for ci, c in enumerate(ix.cl):
                s = rm.exact_scores(pr["q"], c["X"])
                ts = list(rng.choice(s, size=min(6, len(s)), replace=False))
                ts += [np.nextafter(x, 2.0) for x in ts[:3]] + [np.nextafter(x, -2.0) for x in ts[:3]]
                ts += [x + 10 ** rng.uniform(-12, -8) * rng.choice([-1, 1]) for x in ts[:3]]
                ts += [float(s.max()), float(s.min()), 0.0, pr["qn"], -pr["qn"], float(rng.uniform(-1, 1)), 0.9 * pr["qn"]]
                cu = rm.cut_params(ix, ci, pr)
                for t in ts:
                    t = float(t)
                    widen = None if mode == "built" else rm.DA + 1e-9 + max(0.0, math.sqrt(ix.x2) - 1.0)
                    d, how = rm.decisions(ix, ci, pr, t, widen)
                    tot["checks"] += 1
                    tot["rows"] += len(d)
                    tot["decided"] += int(np.count_nonzero(d))
                    tot["skip"] += how == "skip"
                    tot["allin"] += how == "in"
                    # wrong decisions (s is correctly rounded; exact compare when close)
                    isin = s >= t
                    close = np.abs(s - t) < 1e-12
                    for i in np.nonzero(close & (d != 0))[0]:
                        isin[i] = rm.ge_exact(pr["q"], c["X"][i], t)
                    bad = np.nonzero(((d == 1) & ~isin) | ((d == -1) & isin))[0]
                    if len(bad):
                        tot["wrong"] += len(bad)
                        tot["wrong_unclamped"] += int(np.count_nonzero(np.abs(c["a"][bad]) <= 1.0 + rm.DA))
                        tot["wrong_t_within_qn"] += int(abs(t) <= pr["qn"]) * len(bad)
                        if len(worst) < 10:
                            worst.append((it, n, K, how, t, [(int(i), float(s[i]), int(d[i]), float(c["a"][i])) for i in bad[:3]]))
                    if how == "open":
                        # arrangement along the key: runs of out(-1), in(+1), undecided(0)
                        runs = [int(d[0])]
                        for v in d[1:]:
                            if v != runs[-1]:
                                runs.append(int(v))
                        key = "".join({-1: "O", 1: "I", 0: "u"}[v] for v in runs)
                        patterns[key] = patterns.get(key, 0) + 1
                        # Lemma 1.5 on the real-valued bounds at the true keys
                        L, U = rm.real_bounds(ix, ci, pr, c["a"])
                        und = (U >= t) & (L < t)
                        o = np.argsort(c["a"], kind="stable")
                        uo = und[o]
                        if uo.any():
                            first, last = np.argmax(uo), len(uo) - 1 - np.argmax(uo[::-1])
                            if not uo[first:last + 1].all():
                                tot["lemma_bad"] += 1
                            # the same set in the stored order (ties and rounding can break contiguity there)
                            first, last = np.argmax(und), len(und) - 1 - np.argmax(und[::-1])
                            if not und[first:last + 1].all():
                                tot["ties_noncontig"] += 1
print("seed", seed, "mode", mode, tot)
print("arrangements along the key (O out, I in, u undecided):", dict(sorted(patterns.items(), key=lambda kv: -kv[1])))
for w in worst:
    print("WRONG", w)
