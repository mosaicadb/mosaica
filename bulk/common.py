# Shared setup for the bulk experiments: the 200k random sample of the 1M set used by vc/scripts/margin.py (seed 11),
# cluster means over the full 1M, per-group bases (PCA of the residuals, or the dual pair for a query-offset covariance),
# residual coordinates Z (float32, in place of the sample), exact scores for the test queries.
# No memory maps are kept: rows are read with plain file reads so resident memory stays near one copy of the sample.
import numpy as np, os, time, json
V = os.environ.get("V", "/tmp/claude-0/-home-user-mosaica/8c244050-1af9-5b77-a4e8-97638e77c857/scratchpad/vc")
T0 = time.time()
def log(*a): print(f"[{time.time()-T0:6.0f}s]", *a, flush=True)

def wait_lock():
    w = False
    while os.path.exists(f"{V}/TIMING.lock"):
        if not w: log("TIMING.lock present, waiting"); w = True
        time.sleep(5)

EDGES = [0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 768]

def read_rows(lo, hi):
    X = np.load(f"{V}/X.npy", mmap_mode="r"); off = X.offset; n = X.shape[1]; del X
    with open(f"{V}/X.npy", "rb") as f:
        f.seek(off + lo * n * 4); return np.fromfile(f, dtype="<f4", count=(hi - lo) * n).reshape(hi - lo, n)

def setup(basis="pca", eps=0.1, nq_text_step=3, nq_item=100, ns=200_000, seed=11):
    """Returns a dict with Z (sample residual coordinates, clusters contiguous), per-item and per-cluster arrays, queries and exact scores."""
    wait_lock()
    rng = np.random.default_rng(seed)
    ix = np.load(f"{V}/index.npz"); a0 = ix["levels"][0]; grp = ix["grp"]; K = int(a0.max()) + 1; G = int(grp.max()) + 1
    N = len(a0); n = 768
    si = np.sort(rng.choice(N, ns, replace=False))
    Ms = np.zeros((K, n)); cnt = np.bincount(a0, minlength=K); Xs = np.empty((ns, n), np.float32); p = 0
    cs = a0[si]; gs = grp[cs]
    o = np.lexsort((cs, gs)); inv = np.argsort(o)                                      # groups, then clusters, contiguous
    for i in range(0, N, 50_000):
        C = read_rows(i, i + 50_000); np.add.at(Ms, a0[i:i+50_000], C)
        m = si[(si >= i) & (si < i + 50_000)] - i; Xs[inv[p:p+len(m)]] = C[m]; p += len(m); del C
    Mean = (Ms / cnt[:, None]).astype(np.float32); del Ms
    cs = cs[o]; gs = gs[o]; si = si[o]
    # queries and exact scores (before the sample is overwritten by its coordinates)
    Qt = np.load(f"{V}/text_q.npy").astype(np.float32); Qt /= np.linalg.norm(Qt, axis=1, keepdims=True); Qt = Qt[::nq_text_step]
    qi = np.load(f"{V}/item_q_idx.npy")[:nq_item]
    Qi = np.stack([read_rows(int(i), int(i) + 1)[0] for i in qi]) if nq_item else np.zeros((0, n), np.float32)
    Q = np.concatenate([Qt, Qi]); kind = ["text"] * len(Qt) + ["item"] * len(Qi)
    S = np.empty((ns, len(Q)), np.float32)
    for i in range(0, ns, 20_000): S[i:i+20_000] = Xs[i:i+20_000] @ Q.T
    b = np.einsum("ij,ij->i", Xs, Mean[cs]) - (Mean[cs] ** 2).sum(1)                   # m.r, exact scalar per item
    if basis != "pca":
        SQ = np.load(basis).astype(np.float64); SQ = SQ + eps * np.trace(SQ) / n * np.eye(n)
        lq, vq = np.linalg.eigh(SQ); Aq = (vq * np.sqrt(lq)) @ vq.T; Aqi = (vq / np.sqrt(lq)) @ vq.T
    VQ = np.empty((G, n, n), np.float32); LAM = np.empty((G, n), np.float32)
    gb = np.searchsorted(gs, np.arange(G + 1))
    for g in range(G):
        s, e = gb[g], gb[g+1]; R = Xs[s:e] - Mean[cs[s:e]]
        cov = (R.T @ R).astype(np.float64) / max(e - s, 1)
        if basis == "pca":
            l, v = np.linalg.eigh(cov); VD = v[:, ::-1]; VQ[g] = VD
        else:
            l, v = np.linalg.eigh(Aq @ cov @ Aq); l = np.maximum(l[::-1], 1e-12 * l.max()); v = v[:, ::-1]
            VD = (Aq @ v) * l ** -0.25; VQ[g] = (Aqi @ v) * l ** 0.25
        Zg = (R @ VD.astype(np.float32)).astype(np.float32); Xs[s:e] = Zg; LAM[g] = (Zg.astype(np.float64) ** 2).mean(0)
        del R, Zg
    Z = Xs
    Mu = np.stack([Mean[k] @ VQ[grp[k]] for k in range(K)])                           # centres in query-side coordinates
    # cluster boundaries in the sorted sample (clusters are contiguous but not sorted by id)
    st = np.concatenate([[0], np.flatnonzero(np.diff(cs)) + 1]); en = np.concatenate([st[1:], [ns]]); cl = cs[st]
    Rn2 = np.stack([(Z[:, EDGES[i]:EDGES[i+1]] ** 2).sum(1) for i in range(len(EDGES) - 1)], 1)
    log(f"setup done: basis {basis}, sample {ns}, clusters present {len(cl)}, queries text {len(Qt)} item {len(Qi)}")
    return dict(Z=Z, cs=cs, gs=gs, grp=grp, K=K, G=G, n=n, Mean=Mean, Mu=Mu, VQ=VQ, LAM=LAM, b=b, Q=Q, kind=kind, S=S,
                st=st, en=en, cl=cl, Rn2=Rn2, ns=ns)
