# Reduced-dimension STAND-IN: project images and the real text embeddings on the top 192 (uncentred) principal directions of the
# images and renormalise. Not a real 192-dimension model.
import numpy as np, shutil
def unit(a):
    a = a.astype(np.float32); a /= np.linalg.norm(a, axis=1, keepdims=True); return a
X = np.concatenate([unit(np.load(f"../data/e{i:02d}.npy")) for i in (0, 5, 10, 15)])
l, V = np.linalg.eigh((X.T @ X).astype(np.float64) / len(X)); P = V[:, ::-1][:, :192].astype(np.float32)
print("image energy kept", l[::-1][:192].sum() / l.sum())
np.save("n192/X.npy", unit(X @ P))
for f in ("text_q", "text_train"):
    T = np.load(f + ".npy"); Tp = T @ P; print(f, "text energy kept", float((Tp ** 2).sum(1).mean())); np.save(f"n192/{f}.npy", unit(Tp)); shutil.copy(f + ".json", "n192/")
shutil.copy("item_q_idx.npy", "n192/")
