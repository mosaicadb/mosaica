# Does the k-NN selection loop (knn2d at c25333fa9, transcribed) stay exact when only each item's harmful side is
# valid?  Harmful side: upper bounds of the true top k, lower bounds of everything else. The other side is made wrong
# on purpose. Two versions: "as coded" (tau = k-th largest lower bound over all candidates, certify when at most k
# upper bounds reach the candidate's lower bound, intervals intersected across levels) and "others only" (a candidate
# is dropped when k OTHER candidates have a lower bound above its upper bound, certified when at most k - 1 OTHER
# candidates have an upper bound at or above its lower bound).
import numpy as np, sys
rng = np.random.default_rng(int(sys.argv[1]) if len(sys.argv) > 1 else 0)

def run(scores, iv, k, others_only):
    """iv[i][l] = (lb, ub) at level l; after the last level the exact score. Returns the set returned."""
    n, L = len(scores), len(iv[0])
    c = [[i, iv[i][0][0], iv[i][0][1], 1, False] for i in range(n)]
    def drop_and_tau(c):
        if others_only:
            lbs = np.array([x[1] for x in c]); keep = []
            for x in c:
                cnt = int((lbs > x[2]).sum()) - (1 if x[1] > x[2] else 0)
                if cnt < k: keep.append(x)
            return keep
        lbs = sorted((x[1] for x in c), reverse=True); tau = lbs[k - 1]
        return [x for x in c if x[2] >= tau]
    def certified(x, ubs):
        cnt = int((ubs >= x[1]).sum())
        if others_only:
            return cnt - (1 if x[2] >= x[1] else 0) <= k - 1
        return cnt <= k
    while True:
        if len(c) <= k: break
        c = drop_and_tau(c)
        if len(c) <= k: break
        ubs = np.array([x[2] for x in c]); refined = 0
        for x in c:
            if x[4] or certified(x, ubs): continue
            refined += 1
            if x[3] >= L:
                s = scores[x[0]]
                if others_only: x[1], x[2] = s, s
                else: x[1], x[2] = max(s, x[1]), min(s, x[2])
                x[4] = True
            else:
                l, h = iv[x[0]][x[3]]
                x[1], x[2] = max(l, x[1]), min(h, x[2])
            x[3] += 1
        if refined == 0: break
    ubs = np.array([x[2] for x in c])
    fin = []
    for x in c:
        cert = (not x[4]) and certified(x, ubs)
        v = x[1] if x[4] else (np.inf if cert else scores[x[0]])
        fin.append((-v, x[0]))
    fin.sort()
    return set(i for _, i in fin[:k])

def trial(n, k, L, mode):
    s = rng.standard_normal(n)
    top = set(np.argsort(-s, kind="stable")[:k].tolist())
    iv = []
    for i in range(n):
        row = []
        for l in range(L):
            h = 0.6 * 0.45 ** l
            if mode == "two-sided":
                e = rng.uniform(-h, h)
            elif i in top:            # upper bound valid (e >= -h), lower bound often wrong
                e = rng.uniform(-h, 3 * h)
            else:                     # lower bound valid (e <= h), upper bound often wrong
                e = rng.uniform(-3 * h, h)
            row.append((s[i] + e - h, s[i] + e + h))
        iv.append(row)
    return top, s, iv

for mode in ("two-sided", "harmful side only"):
    for others_only in (False, True):
        bad = 0; tot = 0
        for _ in range(4000):
            n = int(rng.integers(8, 40)); k = int(rng.integers(1, 6)); L = int(rng.integers(1, 5))
            top, s, iv = trial(n, k, L, mode)
            out = run(s, iv, k, others_only)
            tot += 1; bad += out != top
        print(f"intervals valid on: {mode:18s}  rule: {'others only' if others_only else 'as coded  '}  wrong answers {bad} of {tot}")
