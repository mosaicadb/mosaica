# Share of the answer within a band just above the threshold, in units of the query's score sd (200k sample).
import numpy as np, sys
W = sys.argv[1]; rng = np.random.default_rng(11)
X = np.load(f"{W}/X.npy", mmap_mode="r"); N = X.shape[0]
si = np.sort(rng.choice(N, 200_000, replace=False)); Xs = np.asarray(X[si])
Qt = np.load(f"{W}/text_q.npy").astype(np.float32); Qt /= np.linalg.norm(Qt, axis=1, keepdims=True)
Q = {"text": Qt, "item": np.asarray(X[np.load(f"{W}/item_q_idx.npy")])}
for kd, Qk in Q.items():
    S = Xs @ Qk.T; sig = S.std(0)
    print(f"== {kd} ({Qk.shape[0]} queries): share of the answer within b sd above the threshold, b = 0.25 / 0.5 / 1")
    for f in (1e-4, 1e-3, 1e-2, 5e-2):
        k = round(f * len(Xs)); t = -np.sort(-S, axis=0)[k - 1]; z = (S - t) / sig
        sh = [np.mean(((z >= 0) & (z < b)).sum(0) / k) for b in (0.25, 0.5, 1.0)]
        top = np.mean((S.max(0) - t) / sig)
        print(f"  answer {100*f:5.2f}% ({k} items): {100*sh[0]:5.1f}% / {100*sh[1]:5.1f}% / {100*sh[2]:5.1f}%   best match {top:.2f} sd above t")
