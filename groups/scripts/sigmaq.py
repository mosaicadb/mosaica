# Query-offset covariances for the geometric-mean metric: offsets of pseudo-queries (corpus items) and of held-out text prompts
# from cluster centres, clusters drawn in proportion to their size. Offset = query minus its component along the centre.
import numpy as np, sys
W = sys.argv[1]; rng = np.random.default_rng(3)
ix = np.load(f"{W}/index.npz"); C = ix["Chat"]; p = np.bincount(ix["levels"][0], minlength=len(C)) / ix["levels"].shape[1]
X = np.load(f"{W}/X.npy", mmap_mode="r"); held = set(np.load(f"{W}/item_q_idx.npy").tolist())
ii = np.array([i for i in np.sort(rng.choice(len(X), 3100, replace=False)) if i not in held][:3000])
def cov(Q):
    S = np.zeros((C.shape[1], C.shape[1])); cnt = 0
    for q in Q:
        c = C[rng.choice(len(C), 40, p=p)]; D = q[None] - (c @ q)[:, None] * c
        S += D.T @ D; cnt += len(D)
    return S / cnt
Si = cov(np.asarray(X[ii])); np.save(f"{W}/sq_item.npy", Si)
try:
    T = np.load(f"{W}/text_train.npy").astype(np.float32); T /= np.linalg.norm(T, axis=1, keepdims=True)
    St = cov(T); np.save(f"{W}/sq_text.npy", St); np.save(f"{W}/sq_mix.npy", 0.5 * (Si / np.trace(Si) + St / np.trace(St)))
    print("text prompts", len(T))
except FileNotFoundError: print("no text_train.npy yet")
