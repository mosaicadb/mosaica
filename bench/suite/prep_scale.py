# The prep stage of the LAION scale suites (speed only): every size of one population in one streaming pass over
# LAION-400M's released CLIP ViT-B/32 embeddings (datasets.py: LAION_URL; part 0's images are VIBE's laion-clip-512).
# usage (through run.py): python3 -I run.py scale|scale-test --stages prep [--datasets ...] [--download DIR]
#
# A population is the first P parts. Each part is downloaded (into --download, else DATA/download/laion, one part at a
# time, removed once read), and for every size with fraction f the rows perm_i[:round(f n_i)] of a permutation seeded
# by the part are kept (sorted), so the sizes are nested and stratified by part. Rows are normalised in f32 and stored
# as f16 only (corpus16.npy; nothing stores f32 at scale). Queries: nq text embeddings of part 0 (f64, as everywhere),
# and a learn sample of 100,000 other text rows for the index's dual fit (out of distribution, as VIBE's laion).
#
# The truth is kept to what speed needs, without an N x nq matrix: the same pass computes each part's similarities
# with every query (f32 products of the stored f16 vectors) and keeps, per query, the running top K = max(k + 1,
# largest share x N + 1). From it: order.npy (the top 100 ids, for exact-k-NN agreement and approximate recall),
# knn_thr.npy, and thresholds.npy at the shares, the midpoint between the k-th and (k+1)-th similarity (so they are
# exact up to f32 rounding, not sample estimates; systems' answer sizes are recorded all the same). Each visible set
# gets the same within its ids. No answer dumps and no check stage follow.
import json
import os
import subprocess
import sys
import time
import zlib

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import prep_dir  # noqa: E402
from datasets import DATASETS, LAION_URL  # noqa: E402
import suites as S  # noqa: E402

SEED = 20261117
LEARN = 100_000
BLOCK = 65536


def log(*a):
    print(f"[prep-scale {time.strftime('%H:%M:%S')}]", *a, flush=True)


def npy_header(url):
    """Shape of a remote .npy file, from its first 256 bytes (a range request)."""
    r = subprocess.run(["curl", "-sSfL", "-r", "0-255", url], capture_output=True, check=True)
    b = r.stdout
    hl = int.from_bytes(b[8:10], "little")
    h = b[10:10 + hl].decode()
    shape = tuple(int(x) for x in h.split("'shape': (")[1].split(")")[0].split(",") if x.strip())
    return shape, 10 + hl


def download(url, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    for attempt in range(5):
        r = subprocess.run(["curl", "-sSfL", "--retry", "3", "-o", path + ".part", url])
        if r.returncode == 0:
            os.replace(path + ".part", path)
            return path
        time.sleep(2 ** (attempt + 2))
    sys.exit(f"could not download {url}")


class TopK:
    """Per query, the running top K of (similarity, id)."""

    def __init__(self, nq, K):
        self.K = K
        self.s = np.full((nq, 0), -np.inf, np.float32)
        self.i = np.zeros((nq, 0), np.int64)

    def add(self, sims, ids):
        """sims: nq x m similarities of the rows with global ids `ids` (m)."""
        if sims.shape[1] == 0:
            return
        s = np.concatenate([self.s, sims], axis=1)
        i = np.concatenate([self.i, np.broadcast_to(ids, sims.shape)], axis=1)
        if s.shape[1] > self.K:
            top = np.argpartition(-s, self.K - 1, axis=1)[:, :self.K]
            s = np.take_along_axis(s, top, 1)
            i = np.take_along_axis(i, top, 1)
        self.s, self.i = s, i

    def sorted(self):
        o = np.argsort(-self.s, axis=1, kind="stable")
        return np.take_along_axis(self.s, o, 1), np.take_along_axis(self.i, o, 1)


def population(suite, name):
    """The sizes of `name`'s population in this suite (same parts), largest first."""
    parts = DATASETS[name]["parts"]
    sizes = [d for d in suite["datasets"] if DATASETS[d].get("source") == "laion-scale" and DATASETS[d]["parts"] == parts]
    return parts, sorted(sizes, key=lambda d: -DATASETS[d]["fraction"])


def prep(suite, name, a):
    parts, sizes = population(suite, name)
    if all(os.path.exists(os.path.join(prep_dir(a.data, d), "meta.json")) for d in sizes):
        log(f"{name}: prepared already ({', '.join(sizes)})")
        return
    t0 = time.time()
    dl = a.download or os.path.join(a.data, "download", "laion")
    nq = suite["nq"]
    t = S.types_for(suite, name)
    ks = sorted(set(t["exact"]["k"]) | set(t["approx"]["k"]) | {100})
    shares = sorted(t["threshold"]["shares"])
    fk = sorted(set(suite["filtered"]["exact"]["k"]) | set(suite["filtered"]["approx"]["k"]) | {100})
    fshares = sorted(suite["filtered"]["threshold"]["shares"])
    filters = [f for f in suite["filters"] if f != "none"]
    for f in filters:
        if S.FILTERS[f]["kind"] != "random":
            sys.exit(f"the scale prep makes random visible sets only, not {f}")

    # part sizes, from the headers
    counts, d = [], None
    for i in range(parts):
        shape, _ = npy_header(LAION_URL.format(kind="img", part=i))
        counts.append(shape[0])
        d = shape[1]
    take = {s: [int(round(DATASETS[s]["fraction"] * c)) for c in counts] for s in sizes}
    N = {s: sum(take[s]) for s in sizes}
    log(f"population of {parts} parts ({sum(counts)} rows, {d} dimensions); sizes {N}")

    # queries and learn sample: text embeddings of part 0
    tf = download(LAION_URL.format(kind="text", part=0), os.path.join(dl, "text_emb_0.npy"))
    T = np.load(tf, mmap_mode="r")
    rng = np.random.default_rng(SEED)
    pick = rng.permutation(T.shape[0])[:nq + LEARN]
    Q = np.asarray(T[np.sort(pick[:nq])], dtype=np.float64)
    Q /= np.linalg.norm(Q, axis=1, keepdims=True)
    L = np.asarray(T[np.sort(pick[nq:])], dtype=np.float32)
    L /= np.linalg.norm(L, axis=1, keepdims=True)
    del T
    os.remove(tf)
    Q32 = Q.astype(np.float32)

    outs, tops, vis, ftops, offs = {}, {}, {}, {}, {}
    for s in sizes:
        out = prep_dir(a.data, s)
        os.makedirs(out, exist_ok=True)
        np.save(os.path.join(out, "queries.npy"), Q)
        np.save(os.path.join(out, "query_ids.npy"), np.sort(pick[:nq]))
        np.save(os.path.join(out, "learn.npy"), L)
        outs[s] = np.lib.format.open_memmap(os.path.join(out, "corpus16.npy.part"), mode="w+", dtype=np.float16,
                                            shape=(N[s], d))
        tops[s] = TopK(nq, max(max(ks) + 1, max(round(x * N[s]) for x in shares) + 1))
        vis[s] = {}
        ftops[s] = {}
        for f in filters:
            r = np.random.default_rng(SEED + zlib.crc32(f"{f}/{s}".encode()))
            ids = np.sort(r.choice(N[s], round(S.FILTERS[f]["share"] * N[s]), replace=False)).astype(np.int32)
            np.save(os.path.join(out, f"vis_{f}.npy"), ids)
            m = np.zeros(N[s], bool)
            m[ids] = True
            vis[s][f] = m
            ftops[s][f] = TopK(nq, max(max(fk) + 1, max(round(x * len(ids)) for x in fshares) + 1))
        offs[s] = 0
    del L

    big = sizes[0]
    for i in range(parts):
        tp = time.time()
        pf = download(LAION_URL.format(kind="img", part=i), os.path.join(dl, f"img_emb_{i}.npy"))
        A = np.load(pf, mmap_mode="r")
        assert A.shape == (counts[i], d), (A.shape, counts[i])
        perm = np.random.default_rng(SEED + i).permutation(counts[i])
        rows = np.sort(perm[:take[big][i]])  # the largest size's rows of this part; the others are subsets
        rank = np.empty(counts[i], np.int64)  # a row's place in the permutation: kept by a size iff below its take
        rank[perm] = np.arange(counts[i])
        for b0 in range(0, len(rows), BLOCK):
            r = rows[b0:b0 + BLOCK]
            X = np.asarray(A[r], dtype=np.float32)
            X /= np.maximum(np.linalg.norm(X, axis=1, keepdims=True), 1e-12)
            X16 = X.astype(np.float16)
            sims = Q32 @ X16.astype(np.float32).T  # nq x m, on the stored f16 values
            for s in sizes:
                keep = rank[r] < take[s][i]
                n = int(keep.sum())
                if n == 0:
                    continue
                g = offs[s] + np.arange(n)
                outs[s][offs[s]:offs[s] + n] = X16[keep]
                sk = sims[:, keep]
                tops[s].add(sk, g)
                for f in filters:
                    m = vis[s][f][g]
                    ftops[s][f].add(sk[:, m], g[m])
                offs[s] += n
        del A
        os.remove(pf)
        log(f"part {i}: {counts[i]} rows, {len(rows)} kept for {big}, in {time.time() - tp:.0f} s")

    for s in sizes:
        assert offs[s] == N[s], (s, offs[s], N[s])
        out = prep_dir(a.data, s)
        outs[s].flush()
        del outs[s]
        os.replace(os.path.join(out, "corpus16.npy.part"), os.path.join(out, "corpus16.npy"))

        def finish(top, n, kk, sh, suffix):
            sv, iv = top.sorted()
            mid = lambda c: (sv[:, c - 1] + sv[:, c]) / 2 if 0 < c < sv.shape[1] else np.full(len(sv), -np.inf)
            np.save(os.path.join(out, f"order{suffix}.npy"), iv[:, :100].astype(np.int32))
            np.save(os.path.join(out, f"knn_thr{suffix}.npy"), np.stack([mid(k) for k in kk], 1).astype(np.float64))
            np.save(os.path.join(out, f"thresholds{suffix}.npy"),
                    np.stack([mid(round(x * n)) for x in sh], 1).astype(np.float64))

        finish(tops[s], N[s], ks, shares, "")
        fm = {}
        for f in filters:
            nv = int(vis[s][f].sum())
            finish(ftops[s][f], nv, fk, fshares, f"_{f}")
            fm[f] = dict(kind="random", share=S.FILTERS[f]["share"], n=nv, k=fk, shares=fshares)
        meta = dict(name=s, N=N[s], d=d, nq=nq, query_kinds=["text"] * nq, k=ks, shares=shares, filters=fm,
                    source="laion-scale", ood=True, scale=True, truth_queries=list(range(nq)), order_depth=100,
                    parts=parts, fraction=DATASETS[s]["fraction"],
                    truth="streaming top-K per query in f32 over the stored f16 vectors; thresholds are midpoints")
        with open(os.path.join(out, "meta.json"), "w") as fo:
            json.dump(meta, fo, indent=1)
        log(f"{s}: N {N[s]}, prepared")
    log(f"population prepared in {time.time() - t0:.0f} s")
