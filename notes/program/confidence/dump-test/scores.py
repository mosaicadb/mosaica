# Scores of the 20 dump queries against all 49,500 rows (normalised, float64), and a check against the run's answer sizes.
import numpy as np, csv
X = np.load("/home/joe/code/vexact-combined-data/c50k/corpus.npy").astype(np.float64); X /= np.linalg.norm(X, axis=1, keepdims=True)
Q = np.concatenate([np.load("/home/joe/code/vexact-combined-data/c50k/item_q.npy")[:10], np.load("/home/joe/code/vexact-data/bioclip2-text/text_q.npy")[:10]]).astype(np.float64)
Q /= np.linalg.norm(Q, axis=1, keepdims=True)
S = Q @ X.T; np.save("work/scores.npy", S)
rows = list(csv.DictReader(open("work/study_res/verify2.csv")))[1::2]          # index-plan rows
T = np.zeros((20, 7)); worst = 0
for i, r in enumerate(rows):
    q, s = int(r["q"]), i % 7; T[q, s] = float(r["t"]); n = int((S[q] >= T[q, s]).sum()); worst = max(worst, abs(n - int(r["truth_size"])))
np.save("work/thresholds.npy", T)
print("largest |count of scores >= t - answer size| over 140 (query, size):", worst)
print("score sd per query:", np.round(S.std(1), 4).tolist())
print("mean score sd: item", round(float(S[:10].std(1).mean()), 4), "text", round(float(S[10:].std(1).mean()), 4))
