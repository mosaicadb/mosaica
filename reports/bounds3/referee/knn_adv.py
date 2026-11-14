# Adversarial test of the k-NN selection loop under harmful-side-only validity.
# Differences from the report's knn_onesided.py: the sweep at tau_1 is modelled (in on lower >= tau_1 at one level's
# interval, out on upper < tau_1, the rest exact); scores and errors sit on coarse grids so that ties between scores,
# between bounds, and between a bound and a score are frequent; the harmless side is wrong by any amount, the harmful
# side is often tight (bound == score); the truth is the first k in (score descending, id ascending).
import numpy as np, sys
rng = np.random.default_rng(int(sys.argv[1]) if len(sys.argv) > 1 else 1)

def select(scores, cands, iv, k, rule, clamp):
    """cands: list of [id, lb, ub, next_level, exact]. rule: 'coded' or 'others'."""
    L = len(iv[0])
    c = [list(x) for x in cands]
    def certified(x, c):
        cnt = sum(1 for y in c if y[2] >= x[1])
        if rule == "others":
            return cnt - (1 if x[2] >= x[1] else 0) <= k - 1
        return cnt <= k
    while True:
        if len(c) <= k: break
        if rule == "others":
            keep = []
            for x in c:
                cnt = sum(1 for y in c if y is not x and y[1] > x[2])
                if cnt < k: keep.append(x)
            c = keep
        else:
            tau = sorted((x[1] for x in c), reverse=True)[k - 1]
            c = [x for x in c if x[2] >= tau]
        if len(c) <= k: break
        snap = [list(x) for x in c]; refined = 0
        for x in c:
            if x[4] or certified(x, snap): continue      # the code tests against the bounds sorted before the pass
            refined += 1
            if x[3] >= L:
                s = scores[x[0]]
                if clamp: x[1], x[2] = max(s, x[1]), min(s, x[2])
                else: x[1], x[2] = s, s
                x[4] = True
            else:
                l, h = iv[x[0]][x[3]]
                x[1], x[2] = max(l, x[1]), min(h, x[2])
            x[3] += 1
        if refined == 0: break
    fin = []
    for x in c:
        cert = (not x[4]) and certified(x, c)
        v = x[1] if x[4] else (np.inf if cert else scores[x[0]])
        fin.append((-v, x[0]))
    fin.sort()
    return [i for _, i in fin[:k]], {i: -v for v, i in fin[:k]}

def trial(mode):
    # integer arithmetic throughout, so that "bound == score" is exact
    n = int(rng.integers(3, 30)); k = int(rng.integers(1, min(n, 7) + 1)); L = int(rng.integers(1, 5))
    scale = int(rng.choice([2, 10, 200]))
    s = np.round(rng.standard_normal(n) * scale).astype(np.int64)
    order = sorted(range(n), key=lambda i: (-s[i], i))
    top = order[:k]; T = set(top)
    iv = []
    for i in range(n):
        row = []
        for l in range(L):
            h = max(1, int(round(0.6 * scale * 0.45 ** l)))
            if mode == "two":
                e = int(rng.integers(-h, h + 1))
            elif i in T:      # upper valid: e >= -h ; lower may be wrong by anything
                e = int(rng.integers(-h, int(rng.choice([1, 3, 30])) * h + 1))
                if rng.random() < 0.15: e = -h     # tight: upper == score
            else:             # lower valid: e <= h
                e = int(rng.integers(-int(rng.choice([1, 3, 30])) * h, h + 1))
                if rng.random() < 0.15: e = h      # tight: lower == score
            row.append((int(s[i]) + e - h, int(s[i]) + e + h))
        iv.append(row)
    # seed: k-th best exact score among a random subset of at least k rows (tau_1 <= s_k), or -inf
    m = int(rng.integers(k, n + 1)); sub = rng.choice(n, m, replace=False)
    tau1 = int(sorted(s[sub], reverse=True)[k - 1]) if rng.random() < 0.9 else -10**9
    cands = []
    for i in range(n):
        done = False
        for l in range(L):
            lb, ub = iv[i][l]
            if lb >= tau1: cands.append([i, lb, ub, l + 1, False]); done = True; break
            if ub < tau1: done = True; break
        if not done: cands.append([i, int(s[i]), int(s[i]), L, True])
    return s, iv, k, top, cands

for mode in ("two", "harmful"):
    for rule, clamp in (("coded", True), ("others", False), ("others", True)):
        bad = badscore = 0; tot = 20000
        for _ in range(tot):
            s, iv, k, top, cands = trial(mode)
            out, vals = select(s, cands, iv, k, rule, clamp)
            if set(out) != set(top): bad += 1
            elif any(np.isfinite(v) and v != s[i] for i, v in vals.items()): badscore += 1
        print(f"valid: {mode:8s} rule: {rule:6s} clamp exact: {clamp!s:5s}  wrong sets {bad} of {tot}; right set but a wrong finite score {badscore}")
