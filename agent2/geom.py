import numpy as np, json, pyarrow.parquet as pq, collections
D = "../data/"
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = np.concatenate([unit(np.load(D + f"e{i:02d}.npy")) for i in (0, 5, 10, 15)])
T = np.load("text_q.npy"); P = json.load(open("text_q.json"))
lab = []
for i in (0, 5, 10, 15):
    t = pq.read_table(f"meta/m{i:02d}.parquet", columns=["genus", "species", "class", "order", "family"]).to_pydict()
    lab += [dict(species=f"{g} {s}" if g and s else None, genus=g, **{"class": c, "order": o, "family": f}) for g, s, c, o, f in zip(t["genus"], t["species"], t["class"], t["order"], t["family"])]
S = X @ T.T   # 200k x 369
mu = X.mean(0); print("image mean norm", np.linalg.norm(mu), "text mean norm", np.linalg.norm(T.mean(0)), "cos(image mean, text mean)", mu @ T.mean(0) / np.linalg.norm(mu) / np.linalg.norm(T.mean(0)))
kinds = [p[0] for p in P]
for k in sorted(set(kinds)):
    m = np.array([x == k for x in kinds]); s = S[:, m]
    srt = np.sort(s, axis=0)[::-1]
    print(f"{k:7s} n={m.sum():3d} cos: mean {s.mean():.3f} max {srt[0].mean():.3f} | thr at top 0.01% {srt[19].mean():.3f} 0.1% {srt[199].mean():.3f} 1% {srt[1999].mean():.3f} 5% {srt[9999].mean():.3f} 50% {srt[99999].mean():.3f}")
# zero-shot sanity: precision of top-20 for labelled prompts
for k in ("sci", "tax", "bare", "com", "higher"):
    pr = []
    for j, p in enumerate(P):
        if p[0] != k: continue
        top = np.argpartition(-S[:, j], 20)[:20]
        pr.append(np.mean([lab[i][p[2]] == p[3] for i in top]))
    print(k, "precision@20 of the label", round(float(np.mean(pr)), 3))
# argmax classification among the 120 sci prompts for items of these species
sci = [j for j, p in enumerate(P) if p[0] == "sci"]; names = [P[j][3] for j in sci]
idx = [i for i in range(len(lab)) if lab[i]["species"] in set(names)]
pred = S[idx][:, sci].argmax(1); acc = np.mean([names[p] == lab[i]["species"] for p, i in zip(pred, idx)])
print("zero-shot top-1 over", len(idx), "items,", len(sci), "classes:", round(float(acc), 3))
# item-item baseline
rng = np.random.default_rng(0); qi = rng.choice(len(X), 100, replace=False); s = X @ X[qi].T; srt = np.sort(s, axis=0)[::-1]
print(f"item    cos: mean {s.mean():.3f} | thr at top 0.01% {srt[19].mean():.3f} 0.1% {srt[199].mean():.3f} 1% {srt[1999].mean():.3f} 5% {srt[9999].mean():.3f} 50% {srt[99999].mean():.3f}")
# where does text live relative to image PCA?
Xc = X - mu; lam, V = np.linalg.eigh((Xc.T @ Xc).astype(np.float64) / len(X)); lam = lam[::-1]; V = V[:, ::-1]
Tc = (T - mu) @ V; e = (Tc ** 2).mean(0); Ie = lam
cum = lambda a: [round(float(a[:k].sum() / a.sum()), 3) for k in (8, 32, 64, 128, 256, 512)]
print("global PCA of images: image energy cum at 8/32/64/128/256/512:", cum(Ie)); print("text-minus-image-mean energy cum:", cum(e), "total |T-mu|^2", e.sum(), "image total", Ie.sum())
gap = T.mean(0) - mu; print("gap norm", np.linalg.norm(gap), "share of text offset energy along gap dir", ((T - mu) @ gap / np.linalg.norm(gap)) ** 2 @ np.ones(len(T)) / len(T) / e.sum())
Tcc = T - T.mean(0); sv = np.linalg.svd(Tcc, compute_uv=False) ** 2; print("text centred energy", sv.sum() / len(T), "cum by own PCs 1/4/16/64/128:", [round(float(sv[:k].sum() / sv.sum()), 3) for k in (1, 4, 16, 64, 128)])
