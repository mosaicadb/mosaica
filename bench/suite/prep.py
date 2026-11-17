# The prep stage: one dataset's corpus, queries, learn sample, visible sets and f64 ground truth, under DATA/prep/<name>.
# usage (normally through run.py): python3 -I prep.py SUITE DATASET [--data DIR] [--hdf5-dirs D1,D2] [--no-f16]
#        [--drop-hdf5] [--bioclip-raw DIR --bioclip-text DIR]
# HDF5 files are untrusted downloads: run this with python3 -I. They are looked for in DATA/hdf5 and in --hdf5-dirs (a
# VIBE or ANN-Benchmarks data directory, say), else downloaded to DATA/hdf5.
#
# Writes (see common.Prep): corpus32.npy (unit vectors, f32; row i is entity i), corpus16.npy (the same in f16, unless
# --no-f16), queries.npy (f64 unit vectors), query_ids.npy (their rows in the source's test set), learn.npy and
# learn_nb.npy (out-of-distribution sets: up to LEARN_MAX rows of the query sample, evenly spaced, and the first 10
# neighbours of the first 1,000), and the truth: order.npy, thresholds.npy, knn_thr.npy (the midpoint between the k-th
# and (k+1)-th similarity for each k-NN k), and for each visible set vis_<f>.npy, order_<f>.npy, thresholds_<f>.npy and
# knn_thr_<f>.npy. Similarities are f64 products of the f64 queries with the stored f32 vectors.
import argparse
import glob
import json
import os
import shutil
import subprocess
import sys
import time
import zlib

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import data_dir, prep_dir  # noqa: E402
from datasets import DATASETS, url  # noqa: E402
import suites as S  # noqa: E402

SEED = 20261116
LEARN_MAX = 100_000
BLOCK = 32768


def log(*a):
    print("[prep]", *a, flush=True)


def unit(M):
    M = np.asarray(M, dtype=np.float64)
    n = np.linalg.norm(M, axis=1, keepdims=True)
    n[n == 0] = 1.0  # a zero vector stays zero (glove has a few)
    return M / n


def find_hdf5(name, data, dirs):
    for d in [os.path.join(data, "hdf5")] + [x for x in dirs if x]:
        p = os.path.join(d, f"{name}.hdf5")
        if os.path.exists(p):
            return p
    os.makedirs(os.path.join(data, "hdf5"), exist_ok=True)
    p = os.path.join(data, "hdf5", f"{name}.hdf5")
    u = url(name)
    log(f"downloading {u}")
    for attempt in range(4):
        r = subprocess.run(["curl", "-sSL", "--fail", "-o", p + ".part", u])
        if r.returncode == 0:
            os.replace(p + ".part", p)
            return p
        time.sleep(2 ** (attempt + 2))
    sys.exit(f"could not download {u}")


def write_unit_rows(path32, path16, rows_iter, N, d):
    """Normalises blocks of rows in f64 and writes them as f32 (and f16) .npy files."""
    o32 = np.lib.format.open_memmap(path32 + ".part", mode="w+", dtype=np.float32, shape=(N, d))
    o16 = np.lib.format.open_memmap(path16 + ".part", mode="w+", dtype=np.float16, shape=(N, d)) if path16 else None
    i = 0
    for B in rows_iter:
        U = unit(B)
        o32[i:i + len(U)] = U
        if o16 is not None:
            o16[i:i + len(U)] = U
        i += len(U)
    assert i == N, (i, N)
    o32.flush()
    del o32
    os.replace(path32 + ".part", path32)
    if o16 is not None:
        o16.flush()
        del o16
        os.replace(path16 + ".part", path16)


def corpus_from_hdf5(name, out, a, nq):
    import h5py
    ds = DATASETS[name]
    p = find_hdf5(name, a.data, a.hdf5_dirs.split(","))
    with h5py.File(p, "r") as f:
        tr = f["train"]
        N, d = tr.shape
        log(f"{name}: train {N} x {d}, test {f['test'].shape[0]}; metric {f.attrs.get('distance')}")
        write_unit_rows(os.path.join(out, "corpus32.npy"), None if a.no_f16 else os.path.join(out, "corpus16.npy"),
                        (tr[i:i + BLOCK] for i in range(0, N, BLOCK)), N, d)
        test = f["test"][:]
        rng = np.random.default_rng(SEED)
        pick = np.sort(rng.choice(len(test), min(nq, len(test)), replace=False))
        np.save(os.path.join(out, "query_ids.npy"), pick)
        np.save(os.path.join(out, "queries.npy"), unit(test[pick]))
        kinds = ["query"] * len(pick)
        if ds["ood"] and "learn" in f:
            L = f["learn"]
            take = np.unique(np.linspace(0, L.shape[0] - 1, min(LEARN_MAX, L.shape[0])).astype(np.int64))
            np.save(os.path.join(out, "learn.npy"), np.asarray(L[take], dtype=np.float32))
            if "learn_neighbors" in f:
                np.save(os.path.join(out, "learn_nb.npy"),
                        np.asarray(f["learn_neighbors"][take[:1000], :10], dtype=np.int64))
            log(f"{name}: learn sample {len(take)} of {L.shape[0]}")
    if a.drop_hdf5 and p.startswith(os.path.join(a.data, "hdf5")):
        os.remove(p)
        log(f"removed {p}")
    return N, d, kinds


def corpus_from_bioclip(out, a):
    """As ../threshold-db/prep.py: 50 random rows of the fetched BioCLIP-2 embeddings are held out as image queries,
    with 50 of text_q and 50 of text_insample (the same seed and draws); the rest is the corpus."""
    if not a.bioclip_raw or not a.bioclip_text:
        sys.exit("bioclip2-768 needs --bioclip-raw (../threshold-db/fetch.py's output) and --bioclip-text "
                 "(text_q.npy, text_insample.npy, optionally text_train.npy: BioCLIP-2 text-prompt embeddings)")
    files = sorted(glob.glob(f"{a.bioclip_raw}/e*.npy"))
    sizes = [np.load(f, mmap_mode="r").shape[0] for f in files]
    total = sum(sizes)
    d = np.load(files[0], mmap_mode="r").shape[1]
    rng = np.random.default_rng(20261114)
    held = rng.choice(total, 50, replace=False)
    keep = np.ones(total, bool)
    keep[held] = False
    tq = np.load(f"{a.bioclip_text}/text_q.npy")
    ti = np.load(f"{a.bioclip_text}/text_insample.npy")
    starts = np.cumsum([0] + sizes)

    def rows(ix):
        out = []
        for i in ix:
            fi = np.searchsorted(starts, i, side="right") - 1
            out.append(np.load(files[fi], mmap_mode="r")[i - starts[fi]])
        return np.stack(out)

    Q = np.concatenate([unit(rows(held)), unit(tq[rng.choice(len(tq), 50, replace=False)]),
                        unit(ti[rng.choice(len(ti), 50, replace=False)])])

    def blocks():
        for fi, f in enumerate(files):
            A = np.load(f, mmap_mode="r")
            yield A[keep[starts[fi]:starts[fi + 1]]]

    N = int(keep.sum())
    write_unit_rows(os.path.join(out, "corpus32.npy"), None if a.no_f16 else os.path.join(out, "corpus16.npy"),
                    blocks(), N, d)
    np.save(os.path.join(out, "queries.npy"), Q)
    np.save(os.path.join(out, "query_ids.npy"), np.arange(len(Q)))
    if os.path.exists(f"{a.bioclip_text}/text_train.npy"):
        np.save(os.path.join(out, "learn.npy"), np.load(f"{a.bioclip_text}/text_train.npy").astype(np.float32))
    return N, d, ["image"] * 50 + ["text_q"] * 50 + ["text_insample"] * 50


def similarities(X, Q):
    """nq x N f64 similarities of the f64 queries with the stored f32 rows."""
    N = X.shape[0]
    S = np.empty((len(Q), N))
    for i in range(0, N, BLOCK):
        S[:, i:i + BLOCK] = Q @ np.asarray(X[i:i + BLOCK], dtype=np.float64).T
    return S


def truth_rows(s, ids, ks, counts):
    """For one query's similarities s over the ids (None: all rows): ids best first to the depth needed, the k-NN
    midpoints for ks and the threshold midpoints for the answer counts."""
    n = len(s)
    depth = min(n - 1, max([k + 1 for k in ks] + [c + 1 for c in counts]))
    top = np.argpartition(-s, depth)[:depth + 1]
    top = top[np.argsort(-s[top], kind="stable")]
    mid = lambda k: (s[top[k - 1]] + s[top[k]]) / 2 if 0 < k < n else (np.inf if k == 0 else -np.inf)
    order = top[:depth] if ids is None else ids[top[:depth]]
    return order.astype(np.int32), [mid(k) for k in ks], [mid(c) for c in counts]


def spherical_kmeans(X, k, rng, iters=15, sample=100_000):
    N = X.shape[0]
    S = np.asarray(X[np.sort(rng.choice(N, min(sample, N), replace=False))], dtype=np.float32)
    C = S[rng.choice(len(S), k, replace=False)].copy()
    for _ in range(iters):
        a = np.argmax(S @ C.T, axis=1)
        for j in range(k):
            m = S[a == j]
            if len(m):
                v = m.sum(0)
                C[j] = v / max(np.linalg.norm(v), 1e-12)
    return C


def visible_set(name, spec, X, N):
    rng = np.random.default_rng(SEED + zlib.crc32(name.encode()))
    target = round(spec["share"] * N)
    if spec["kind"] == "random":
        return np.sort(rng.choice(N, target, replace=False)).astype(np.int32), {}
    C = spherical_kmeans(X, spec["centres"], rng)
    lab = np.empty(N, np.int32)
    for i in range(0, N, BLOCK):
        lab[i:i + BLOCK] = np.argmax(np.asarray(X[i:i + BLOCK], dtype=np.float32) @ C.T, axis=1)
    sizes = np.bincount(lab, minlength=len(C))
    chosen, total = [], 0
    for c in rng.permutation(len(C)):
        if total + sizes[c] > target:
            rows = np.flatnonzero(lab == c)
            fill = rng.choice(rows, target - total, replace=False)
            ids = np.concatenate([np.flatnonzero(np.isin(lab, chosen)), fill])
            return np.sort(ids).astype(np.int32), dict(centres_whole=len(chosen), filled_from=int(c))
        chosen.append(c)
        total += sizes[c]
    return np.arange(N, dtype=np.int32), {}


def prep(suite, name, a):
    out = prep_dir(a.data, name)
    os.makedirs(out, exist_ok=True)
    mpath = os.path.join(out, "meta.json")
    meta = json.load(open(mpath)) if os.path.exists(mpath) else None
    needs = S.truth_needs(suite, name)
    t0 = time.time()
    if meta is None or not os.path.exists(os.path.join(out, "corpus32.npy")):
        if DATASETS[name]["source"] == "bioclip":
            N, d, kinds = corpus_from_bioclip(out, a)
        else:
            N, d, kinds = corpus_from_hdf5(name, out, a, suite["nq"])
        meta = dict(name=name, N=N, d=d, nq=len(kinds), query_kinds=kinds, k=[], shares=[], filters={},
                    source=DATASETS[name]["source"], ood=DATASETS[name]["ood"])
        log(f"{name}: corpus and queries in {time.time() - t0:.0f} s")
    # what is already there and covers the needs is kept
    have_k, have_s = set(meta["k"]), set(meta["shares"])
    todo_main = not (set(needs["k"]) <= have_k and set(needs["shares"]) <= have_s)
    todo_f = {f: sp for f, sp in needs["filters"].items()
              if f not in meta["filters"] or not (set(sp["k"]) <= set(meta["filters"][f]["k"])
                                                  and set(sp["shares"]) <= set(meta["filters"][f]["shares"]))}
    if not todo_main and not todo_f:
        log(f"{name}: prepared already")
        return
    X = np.load(os.path.join(out, "corpus32.npy"), mmap_mode="r")
    Q = np.load(os.path.join(out, "queries.npy"))
    N = X.shape[0]
    t1 = time.time()
    Sm = similarities(X, Q)
    log(f"{name}: f64 similarities {Sm.shape} in {time.time() - t1:.0f} s")
    if todo_main:
        ks = sorted(have_k | set(needs["k"]))
        shares = sorted(have_s | set(needs["shares"]))
        counts = [round(s * N) for s in shares]
        res = [truth_rows(Sm[j], None, ks, counts) for j in range(len(Q))]
        np.save(os.path.join(out, "order.npy"), np.stack([r[0] for r in res]))
        np.save(os.path.join(out, "knn_thr.npy"), np.array([r[1] for r in res]))
        np.save(os.path.join(out, "thresholds.npy"), np.array([r[2] for r in res]))
        meta["k"], meta["shares"] = ks, shares
        log(f"{name}: truth to depth {res[0][0].shape[0]} (k {ks}, shares {shares})")
    for f, sp in todo_f.items():
        old = meta["filters"].get(f)
        vpath = os.path.join(out, f"vis_{f}.npy")
        if old and os.path.exists(vpath) and old["kind"] == sp["kind"] and old["share"] == sp["share"]:
            vis = np.load(vpath)
            info = old.get("info", {})
        else:
            t1 = time.time()
            vis, info = visible_set(f, sp, X, N)
            np.save(vpath, vis)
            log(f"{name}: visible set {f}: {len(vis)} ids in {time.time() - t1:.0f} s {info}")
        ks = sorted(set(sp["k"]) | set(old["k"] if old else []))
        shares = sorted(set(sp["shares"]) | set(old["shares"] if old else []))
        counts = [round(s * len(vis)) for s in shares]
        vi = vis.astype(np.int64)
        res = [truth_rows(Sm[j, vi], vi, ks, counts) for j in range(len(Q))]
        np.save(os.path.join(out, f"order_{f}.npy"), np.stack([r[0] for r in res]))
        np.save(os.path.join(out, f"knn_thr_{f}.npy"), np.array([r[1] for r in res]))
        np.save(os.path.join(out, f"thresholds_{f}.npy"), np.array([r[2] for r in res]))
        meta["filters"][f] = dict(kind=sp["kind"], share=sp["share"], n=int(len(vis)), k=ks, shares=shares,
                                  info=info, **({"centres": sp["centres"]} if "centres" in sp else {}))
    with open(mpath + ".part", "w") as fo:
        json.dump(meta, fo, indent=1)
    os.replace(mpath + ".part", mpath)
    log(f"{name}: prepared in {time.time() - t0:.0f} s")


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("suite", choices=list(S.SUITES))
    ap.add_argument("dataset")
    add_args(ap)
    a = ap.parse_args(argv)
    a.data = data_dir(a.data)
    prep(S.SUITES[a.suite], a.dataset, a)


def add_args(ap):
    ap.add_argument("--data", default=None)
    ap.add_argument("--hdf5-dirs", default="", help="more directories to look for <name>.hdf5 in (comma-separated)")
    ap.add_argument("--no-f16", action="store_true", help="do not write corpus16.npy")
    ap.add_argument("--drop-hdf5", action="store_true", help="remove a downloaded HDF5 file once prepared")
    ap.add_argument("--bioclip-raw", default="")
    ap.add_argument("--bioclip-text", default="")


if __name__ == "__main__":
    main()
