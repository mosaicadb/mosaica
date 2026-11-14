import sys, time
import numpy as np
from scipy.sparse import csr_matrix
from pyroaring import BitMap

def read_spmat(fname):
    with open(fname, "rb") as f:
        nrow, ncol, nnz = np.fromfile(f, dtype='int64', count=3)
        indptr = np.fromfile(f, dtype='int64', count=nrow + 1)
        assert nnz == indptr[-1]
        indices = np.fromfile(f, dtype='int32', count=nnz)
        assert np.all(indices >= 0) and np.all(indices < ncol)
        data = np.fromfile(f, dtype='float32', count=nnz)
        assert f.read(1) == b""
    return csr_matrix((data, indices, indptr), shape=(nrow, ncol))

base_fn, q_fn, out_fn = sys.argv[1:4]
t0 = time.time()
B = read_spmat(base_fn)
Q = read_spmat(q_fn)
out = []
def p(*a):
    s = " ".join(str(x) for x in a)
    print(s, flush=True)
    out.append(s)

p(f"base: {B.shape[0]} rows x {B.shape[1]} tags, nnz={B.nnz}; data values: {np.unique(B.data)[:5]}")
p(f"queries: {Q.shape[0]} rows x {Q.shape[1]} tags, nnz={Q.nnz}; data values: {np.unique(Q.data)[:5]}")
B.sum_duplicates()
p(f"base nnz after dedup: {B.nnz}")

# Posting lists: CSC of base
C = B.tocsc()
C.sort_indices()
del B
freq = np.diff(C.indptr)
p(f"loaded + transposed in {time.time()-t0:.1f}s")

# Posting-list bitmaps
t1 = time.time()
bms = [None] * C.shape[1]
ser_bytes = 0
ser_bytes_opt = 0
for t in range(C.shape[1]):
    if freq[t] == 0:
        continue
    bm = BitMap(C.indices[C.indptr[t]:C.indptr[t+1]])
    ser_bytes += len(bm.serialize())
    bms[t] = bm
p(f"bitmaps built in {time.time()-t1:.1f}s")
# run-optimised size
for bm in bms:
    if bm is not None:
        bm.run_optimize()
        ser_bytes_opt += len(bm.serialize())

# Query V
qtags = np.split(Q.indices, Q.indptr[1:-1])
ntags = np.diff(Q.indptr)
p("query tag-count distribution:", {int(k): int(v) for k, v in zip(*np.unique(ntags, return_counts=True))})
V = np.zeros(Q.shape[0], dtype=np.int64)
rarer = np.zeros(Q.shape[0], dtype=np.int64)
cache = {}
t2 = time.time()
for i, tags in enumerate(qtags):
    key = tuple(sorted(int(x) for x in tags))
    if key in cache:
        V[i], rarer[i] = cache[key]; continue
    fs = [int(freq[t]) for t in key]
    if len(key) == 1:
        v = fs[0]
    else:
        if any(bms[t] is None for t in key):
            v = 0
        else:
            acc = bms[key[0]]
            for t in key[1:-1]:
                acc = acc & bms[t]
            v = acc.intersection_cardinality(bms[key[-1]])
    cache[key] = (v, min(fs))
    V[i], rarer[i] = cache[key]
p(f"V computed in {time.time()-t2:.1f}s; distinct tag-sets: {len(cache)}")

# Cross-check a sample of two-tag V with numpy
rng = np.random.default_rng(0)
two = np.where(ntags == 2)[0]
for i in rng.choice(two, size=min(200, len(two)), replace=False):
    a, b = qtags[i]
    va = C.indices[C.indptr[a]:C.indptr[a+1]]; vb = C.indices[C.indptr[b]:C.indptr[b+1]]
    assert len(np.intersect1d(va, vb, assume_unique=True)) == V[i]
p("cross-check of 200 two-tag queries against numpy intersect1d: OK")

np.savez(out_fn + ".npz", V=V, rarer=rarer, ntags=ntags, freq=freq)

def pct_table(name, v):
    ps = [50, 75, 90, 95, 99, 99.9]
    vals = [np.percentile(v, q, method="lower") for q in ps]
    return f"| {name} | {len(v)} | " + " | ".join(f"{int(x):,}" for x in vals) + f" | {int(v.max()):,} | {v.mean():,.0f} |"

groups = [("all", np.ones(len(V), bool))] + [(f"{k}-tag", ntags == k) for k in sorted(set(ntags.tolist()))]
p("\n## Distribution of V (measured)\n")
p("| queries | n | p50 | p75 | p90 | p95 | p99 | p99.9 | max | mean |")
p("|---|---|---|---|---|---|---|---|---|---|")
for name, m in groups:
    p(pct_table(name, V[m]))

edges = [0, 1, 10, 32, 100, 316, 1000, 3162, 10000, 31623, 100000, 316228, 1000000, 3162278, 10000001]
p("\n## Histogram of V on log bins (measured)\n")
p("| V range | " + " | ".join(n for n, _ in groups) + " |")
p("|---|" + "---|" * len(groups))
for lo, hi in zip(edges[:-1], edges[1:]):
    row = f"| [{lo:,}, {hi:,}) |"
    for _, m in groups:
        c = int(((V[m] >= lo) & (V[m] < hi)).sum())
        row += f" {c:,} ({100*c/m.sum():.1f}%) |"
    p(row)

p("\n## Share of queries with V below a threshold (measured)\n")
p("| threshold | " + " | ".join(n for n, _ in groups) + " |")
p("|---|" + "---|" * len(groups))
for th in [1000, 10000, 30000, 100000, 300000, 1000000]:
    p(f"| V < {th:,} | " + " | ".join(f"{100*(V[m] < th).mean():.2f}%" for _, m in groups) + " |")

p("\n## Cost model (MODELLED: 20 ns/item if V <= cut, else 3 ns/item + 0.2 ms)\n")
p("| cut-off | total single-core s | s in small regime (#q) | s in large regime (#q) | share from top 1% of V | share from top 10% of V | QPS on 8 cores | mean ms/query |")
p("|---|---|---|---|---|---|---|---|")
order = np.argsort(-V, kind="stable")
for cut in [10000, 30000, 100000]:
    small = V <= cut
    cost = np.where(small, 20e-9 * V, 3e-9 * V + 0.2e-3)
    tot = cost.sum()
    cs = cost[order]
    top1 = cs[: len(V) // 100].sum() / tot
    top10 = cs[: len(V) // 10].sum() / tot
    p(f"| {cut:,} | {tot:.1f} | {cost[small].sum():.1f} ({small.sum():,}) | {cost[~small].sum():.1f} ({(~small).sum():,}) | {100*top1:.1f}% | {100*top10:.1f}% | {len(V)/(tot/8):,.0f} | {1e3*tot/len(V):.3f} |")
# also the fixed 0.2 ms part vs per-item part for 30k
cut = 30000; big = V > cut
p(f"\nAt cut 30k, large regime: fixed 0.2 ms part = {0.2e-3*big.sum():.1f} s, per-item part = {3e-9*V[big].sum():.1f} s. Sum of V over all queries = {V.sum():,}.")

p("\n## Tag frequencies in the base (measured)\n")
p(f"(item, tag) pairs: {int(freq.sum()):,}; tags with >=1 item: {(freq>0).sum():,} of {len(freq):,}; items with no tag: computed below")
p("| tags with more than | count |")
p("|---|---|")
for th in [1000, 10000, 100000, 1000000]:
    p(f"| {th:,} items | {(freq > th).sum():,} |")
p(f"\nlargest tag frequencies: {sorted(freq.tolist(), reverse=True)[:10]}")
p(f"\n## Posting lists as Roaring bitmaps (measured with pyroaring {__import__('pyroaring').__version__})\n")
p(f"serialized size, no run optimisation: {ser_bytes:,} bytes ({ser_bytes/2**20:.1f} MiB), {8*ser_bytes/freq.sum():.2f} bits per pair")
p(f"serialized size, after run_optimize: {ser_bytes_opt:,} bytes ({ser_bytes_opt/2**20:.1f} MiB)")
p(f"for comparison, plain uint32 arrays: {4*int(freq.sum()):,} bytes ({4*freq.sum()/2**20:.1f} MiB)")

p("\n## Two-tag queries: V / rarer tag's count (measured)\n")
m2 = ntags == 2
r = V[m2] / np.maximum(rarer[m2], 1)
redges = [0, 1e-4, 1e-3, 1e-2, 0.1, 0.5, 1.0000001]
p("| V/rarer | queries |")
p("|---|---|")
zero = int((V[m2] == 0).sum())
p(f"| exactly 0 | {zero:,} ({100*zero/m2.sum():.1f}%) |")
for lo, hi in zip(redges[:-1], redges[1:]):
    c = int(((r >= lo) & (r < hi) & (V[m2] > 0)).sum()) if lo == 0 else int(((r >= lo) & (r < hi)).sum())
    p(f"| [{lo:g}, {min(hi,1):g}{']' if hi > 1 else ')'} {'(V>0)' if lo == 0 else ''} | {c:,} ({100*c/m2.sum():.1f}%) |")
p(f"\nmedian V/rarer for two-tag queries: {np.median(r):.4f}; rarer-tag count p50/p90/max: {int(np.median(rarer[m2])):,} / {int(np.percentile(rarer[m2],90)):,} / {int(rarer[m2].max()):,}")
# What a rarer-tag scan would cost vs exact intersection scan
p(f"sum over two-tag queries of rarer-tag count: {int(rarer[m2].sum()):,} vs sum of V: {int(V[m2].sum()):,}")

with open(out_fn, "w") as f:
    f.write("# YFCC-10M filtered track: filtered-set sizes\n\n" + "\n".join(out) + "\n")
