import numpy as np, json
I = np.load("index.npz"); Mean, grp, V, lam, Qi = I["Mean"], I["grp"], I["V"], I["lam"], I["Qi"]
T = np.load("text_q.npy"); G, n = lam.shape
GT = np.array([1, .3634, .1188, .03744, .01154, .00349, .00104, .0003043, .0000876])
def alloc(lam, Qe, B):
    b = np.zeros(len(lam), int); f = lambda bb: np.sqrt(Qe * lam * GT[bb]); val = f(b)
    for _ in range(B):
        nb = np.minimum(b + 1, 8); gain = val - f(nb); gain[b >= 8] = -1
        i = int(gain.argmax()); b[i] += 1; val = f(b)
    return b
qm = T @ Mean.T; top = np.argsort(-qm, axis=1)[:, :8]
edges = [0, 8, 32, 64, 128, 256, 512, 768]
print("energy share by local-dimension band", edges)
tot = np.zeros((3, len(edges) - 1)); S = np.zeros((2, 2, 5)); w = 0
for g in range(G):
    rows, cols = np.where(grp[top] == g)
    if len(rows) < 30: continue
    Qt = (((T[rows] - Mean[top[rows, cols]]) @ V[g]) ** 2).mean(0)
    band = lambda a: np.array([a[e0:e1].sum() for e0, e1 in zip(edges[:-1], edges[1:])])
    print(f"g{g:2d} pairs {len(rows):4d} |D|^2 item {Qi[g].sum():.3f} text {Qt.sum():.3f} resid {lam[g].sum():.3f} | item", np.round(band(Qi[g]) / Qi[g].sum(), 2), "| text", np.round(band(Qt) / Qt.sum(), 2), "| resid", np.round(band(lam[g]) / lam[g].sum(), 2))
    # predicted l1 bound sum_i sqrt(Q lam g(b)) for: query type x allocation type
    for bi, B in enumerate((0, 256, 768, 1536, 3072)):
        for ai, Qa in enumerate((Qi[g], Qt)):
            bits = alloc(lam[g], Qa, B)
            for qi_, Qq in enumerate((Qi[g], Qt)):
                S[qi_, ai, bi] += len(rows) * np.sqrt(Qq * lam[g] * GT[bits]).sum()
    w += len(rows)
S /= w
print("predicted l1 bound  sum_i sqrt(Q_i lam_i g(b_i))  at 0/32/96/192/384 bytes")
for qi_, qn in enumerate(("item queries", "text queries")):
    for ai, an in enumerate(("item-fitted bits", "text-fitted bits")):
        print(f"  {qn}, {an}:", np.round(S[qi_, ai], 4))
