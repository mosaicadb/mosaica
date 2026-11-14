# Label check for the text queries against the 200k sample (g0-g3 with meta_g0-3.parquet): for each prompt with a label,
# whether the label occurs in the sample, and the share of its 10 nearest images (and the top 1) that carry it.
# usage: check_labels.py DATADIR CORPUSDIR
import sys, json, collections, numpy as np, pyarrow.parquet as pq
D, CD = sys.argv[1], sys.argv[2]
X = np.concatenate([np.load(f"{CD}/g{j}.npy") for j in range(4)]).astype(np.float32); X /= np.linalg.norm(X, axis=1, keepdims=True)
cols = ["class", "order", "family", "genus", "species"]
M = {c: [] for c in cols}
for j in range(4):
    t = pq.read_table(f"{D}/meta_g{j}.parquet", columns=cols).to_pydict()
    for c in cols: M[c] += t[c]
M["species"] = [f"{g} {s}" if g and s else None for g, s in zip(M["genus"], M["species"])]
P = json.load(open(f"{D}/text_q.json")); E = np.load(f"{D}/text_q.npy")
S = E @ X.T; top = np.argsort(-S, axis=1)[:, :10]
cnt = {c: collections.Counter(M[c]) for c in cols}
rows = collections.defaultdict(list); insample = []
for i, (kind, text, col, val) in enumerate(P):
    if not col: insample.append(None); continue
    n = cnt[col][val]; insample.append(n)
    h = [M[col][r] == val for r in top[i]]
    rows[(kind, n > 0)].append((h[0], np.mean(h)))
print("prompts", len(P), collections.Counter(p[0] for p in P))
print("kind  label-in-sample  prompts  top-1 hit  top-10 hit share")
for (kind, ins), v in sorted(rows.items()):
    a = np.array(v, float); print(f"{kind:6s} {str(ins):5s} {len(v):4d}  {a[:, 0].mean():.3f}  {a[:, 1].mean():.3f}")
sp = [i for i, p in enumerate(P) if p[0] == "sci" and insample[i]]
print("'a photo of <species>' with the species in the sample:", len(sp), "of", sum(p[0] == "sci" for p in P),
      "; species in sample, any species-labelled prompt:", sum(1 for i, p in enumerate(P) if p[2] == "species" and insample[i]))
json.dump(insample, open(f"{D}/text_q_insample.json", "w"))
