"""The stored level ratios against the replay that chose the rule ("A data cluster ratio" of
../sample-when-needed/replay.py): the replay's per-cluster ratio from its training dump of 400 corpus rows as queries,
against the library's `adaptive::level_ratios` from the same queries (`study_level_ratios`, .study.f32) and from the
build's own 400 rows (.build.f32). Prints, per deterministic level, the ratios' p10 / median / p90 under each and the
per-cluster |log| differences.
usage: python3 -I check_ratios.py NAME DUMP_PREFIX DATA_ROWS.npy STUDY_PREFIX
"""
import ast
import sys

import numpy as np

name, prefix, rows_path, study = sys.argv[1:5]
MIN_PAIRS = 3
meta = dict(line.rstrip("\n").split(" ", 1) for line in open(prefix + ".meta"))
nq, K, M, L, ndl = (int(meta[x]) for x in ("nq", "k", "m", "levels", "ndl"))
rho = np.array(ast.literal_eval(meta["rho"]), np.float64)
idx = np.fromfile(prefix + ".idx", np.uint32).reshape(nq, M, 2)
w = 5 + 2 * L
raw = np.memmap(prefix + ".rows", np.float32, mode="r").reshape(-1, w)
ent = np.memmap(prefix + ".rows", np.uint32, mode="r").reshape(-1, w)[:, 0]
off = np.concatenate([[0], np.cumsum(idx[:, :, 1].astype(np.int64).ravel())])
grp = np.load(prefix + ".grp.npy")
self_rows = np.load(rows_path)

# replay.py's train() and units(): rms z per cluster, a cluster of fewer than MIN_PAIRS pairs taking its group's
acc = np.zeros((K, ndl, 2))
pairs = np.zeros(K, int)
for qi in range(nq):
    for j in range(M):
        c, nr = int(idx[qi, j, 0]), int(idx[qi, j, 1])
        if c == 0xFFFFFFFF or nr == 0:
            continue
        lo, hi = off[qi * M + j], off[qi * M + j + 1]
        r = np.asarray(raw[lo:hi], np.float64)
        keep = np.asarray(ent[lo:hi]) != self_rows[qi]
        if keep.sum() < 2:
            continue
        lv = r[:, 5:].reshape(-1, L, 2)
        mid, h = (lv[:, :, 0] + lv[:, :, 1]) / 2, (lv[:, :, 1] - lv[:, :, 0]) / 2
        pairs[c] += 1
        for l in range(ndl):
            z = (r[keep, 1] - mid[keep, l]) / (rho[grp[c], l] * h[keep, l])
            acc[c, l] += (len(z), (z * z).sum())
ng = rho.shape[0]
gacc = np.zeros((ng, ndl, 2))
for c in range(K):
    gacc[grp[c]] += acc[c]
rms = lambda x: np.sqrt(x[..., 1] / np.maximum(x[..., 0], 1))
replay = np.where((pairs >= MIN_PAIRS)[:, None], rms(acc), rms(gacc)[grp])

lib = np.fromfile(study + ".study.f32", np.float32).reshape(K, ndl).astype(np.float64)
build = np.fromfile(study + ".build.f32", np.float32).reshape(K, ndl).astype(np.float64)
q3 = lambda v: " / ".join(f"{x:.3f}" for x in np.quantile(v, [0.1, 0.5, 0.9]))
print(f"## {name}: {K} clusters, {ng} groups; {nq} training queries; clusters with >= {MIN_PAIRS} pairs: {int((pairs >= MIN_PAIRS).sum())}; "
      f"groups without a training row: {int((gacc[:, 0, 0] == 0).sum())}")
for l in range(ndl):
    d_lib = np.abs(np.log(lib[:, l] / replay[:, l]))
    d_build = np.abs(np.log(build[:, l] / replay[:, l]))
    print(f"  level {l}: p10 / median / p90: replay {q3(replay[:, l])}, library (same queries) {q3(lib[:, l])}, build (its own rows) {q3(build[:, l])}")
    print(f"    |log(library / replay)| per cluster: median {np.median(d_lib):.2e}, max {d_lib.max():.2e}; "
          f"|log(build / replay)|: median {np.median(d_build):.3f}, p90 {np.quantile(d_build, 0.9):.3f}")
