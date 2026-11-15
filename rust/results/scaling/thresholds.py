# Distribution check: for the first 20 item and 20 text queries, the score at each answer fraction (the
# ceil(frac N)-th largest float32 score of the unit query against the unit corpus) at each N; differences across N.
# usage: thresholds.py D [r]: with r, the population samples (R1M, R3M, the 5M population, and the row-group 1M) with
# the population item queries.
import sys, json, numpy as np
D = sys.argv[1]; R = len(sys.argv) > 2 and sys.argv[2] == "r"; FR = [0.0001, 0.001, 0.01, 0.1, 0.5, 0.75]
def unit(a): a = np.asarray(a, np.float32); return a / np.linalg.norm(a, axis=1, keepdims=True)
Q = np.concatenate([unit(np.load(f"{D}/r_item_q.npy" if R else f"{D}/scale_item_q.npy")[:20]), unit(np.load(f"{D}/text/text_q.npy")[:20])])
srcs = {"500k": f"{D}/c500k_x.npy", "1M": f"{D}/c1m_x.npy", "5M": f"{D}/c5m_x.npy"}
if R: srcs = {"R1M": (f"{D}/c5m_x.npy", f"{D}/r1m_rows.npy"), "R3M": (f"{D}/c5m_x.npy", f"{D}/r3m_rows.npy"), "5M": f"{D}/c5m_x.npy", "1M row groups": f"{D}/c1m_x.npy"}
out = {}
for name, p in srcs.items():
    if isinstance(p, tuple):
        Xa = np.load(p[0], mmap_mode="r"); rows = np.load(p[1])
        class _S:
            def __len__(self): return len(rows)
            def __getitem__(self, sl): return Xa[rows[sl]]
        X = _S()
    else:
        X = np.load(p, mmap_mode="r")
    S = np.empty((len(Q), len(X)), np.float32)
    for i in range(0, len(X), 200_000):
        S[:, i:i + 200_000] = Q @ unit(X[i:i + 200_000]).T
    S = -np.sort(-S, axis=1)
    out[name] = [[float(S[q, int(np.ceil(f * len(X))) - 1]) for f in FR] for q in range(len(Q))]
    print(name, len(X), flush=True)
json.dump(out, open(f"{D}/sc/thresholds{'_r' if R else ''}.json", "w"))
names = list(out)
for a in range(len(names)):
    for b in range(a + 1, len(names)):
        A, B = np.array(out[names[a]]), np.array(out[names[b]])
        for cls, sl in [("item", slice(0, 20)), ("text", slice(20, 40))]:
            d = np.abs(A[sl] - B[sl])
            print(f"{names[a]} vs {names[b]} {cls}: " + "; ".join(f"{f*100:g}% median {np.median(d[:, k]):.4f} max {d[:, k].max():.4f}" for k, f in enumerate(FR)))
