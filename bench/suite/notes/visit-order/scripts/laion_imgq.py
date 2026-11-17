# In-distribution image queries for laion-s1m: 500 image rows of each of LAION parts 0 and 1 that the nested 1M sample
# did not take (prep_scale.py's per-part permutation, seeded SEED + part, keeps perm[:round(0.5 n)]), fetched with
# range requests from the suite's own LAION source (10 blocks of rows per part, spread over the file), normalised as
# prep_scale does (f32, then f16), and their exact top 10 over the 1M corpus (f32 products with the stored f16 rows).
# usage: python -I laion_imgq.py REPO CORPUS16.npy DLDIR OUTDIR
import sys, os, subprocess
import numpy as np

repo, corpus, dl, out = sys.argv[1:5]
sys.path.insert(0, os.path.join(repo, "bench", "suite"))
from datasets import LAION_URL, DATASETS  # noqa: E402
SEED = 20261117  # prep_scale.SEED
frac = DATASETS["laion-s1m"]["fraction"]
os.makedirs(dl, exist_ok=True)
os.makedirs(out, exist_ok=True)


def rng_get(url, a, b, path):
    subprocess.run(["curl", "-sSfL", "--retry", "3", "-r", f"{a}-{b}", "-o", path, url], check=True)
    return open(path, "rb").read()


Qs = []
for part in (0, 1):
    url = LAION_URL.format(kind="img", part=part)
    h = rng_get(url, 0, 255, os.path.join(dl, f"hdr{part}"))
    hl = int.from_bytes(h[8:10], "little")
    hdr = h[10:10 + hl].decode()
    assert "'<f2'" in hdr and "'fortran_order': False" in hdr, hdr
    cnt, d = (int(x) for x in hdr.split("'shape': (")[1].split(")")[0].split(",") if x.strip())
    off = 10 + hl
    perm = np.random.default_rng(SEED + part).permutation(cnt)
    kept = np.zeros(cnt, bool)
    kept[perm[:int(round(frac * cnt))]] = True
    got = []
    for b, start in enumerate(np.linspace(0, cnt - 200, 10).astype(int)):
        raw = rng_get(url, off + start * d * 2, off + (start + 200) * d * 2 - 1, os.path.join(dl, f"p{part}_{b}"))
        A = np.frombuffer(raw, np.float16).reshape(200, d)
        rows = np.arange(start, start + 200)
        sel = ~kept[rows]
        got.append(A[sel][:50])
    Q = np.concatenate(got)
    print(f"part {part}: {cnt} rows, header {hdr.strip()}; {len(Q)} held-out rows", flush=True)
    Qs.append(Q)
X = np.concatenate(Qs).astype(np.float32)
X /= np.maximum(np.linalg.norm(X, axis=1, keepdims=True), 1e-12)
Q = X.astype(np.float16).astype(np.float64)
Q /= np.linalg.norm(Q, axis=1, keepdims=True)
C = np.load(corpus, mmap_mode="r")
k = 10
Q32 = Q.astype(np.float32)
bs, bi = np.full((len(Q), 0), -np.inf, np.float32), np.zeros((len(Q), 0), np.int64)
for i in range(0, C.shape[0], 100_000):
    S = Q32 @ np.asarray(C[i:i + 100_000], np.float32).T
    s = np.concatenate([bs, S], 1)
    ii = np.concatenate([bi, np.broadcast_to(np.arange(i, i + S.shape[1]), S.shape)], 1)
    top = np.argpartition(-s, k - 1, axis=1)[:, :k]
    bs, bi = np.take_along_axis(s, top, 1), np.take_along_axis(ii, top, 1)
o = np.argsort(-bs, 1, kind="stable")
bs, bi = np.take_along_axis(bs, o, 1), np.take_along_axis(bi, o, 1)
np.save(os.path.join(out, "queries.npy"), Q)
np.save(os.path.join(out, "gt.npy"), bi)
print(f"{len(Q)} image queries; 1st / 10th neighbour sim {bs[:, 0].mean():.3f} / {bs[:, 9].mean():.3f}; "
      f"exact duplicates in the corpus (sim > 0.9999): {(bs[:, 0] > 0.9999).sum()}", flush=True)
