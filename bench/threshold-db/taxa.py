# Check that the recovered taxonomy rows are the corpus rows, then build visible sets of whole genera.
# usage: python3 -I taxa.py RAW_DIR SPECIES_DIR PREP_DIR OUT_DIR
# RAW_DIR holds the corpus's e00..e19.npy, SPECIES_DIR the output of fetch_species.py, PREP_DIR the output of prep.py.
# Stops unless every embedding equals the corpus's and the rows left after prep.py's hold-out equal corpus16.npy.
# Writes to OUT_DIR: taxa.parquet (one row per corpus id: kingdom .. species) and gen_1.npy, gen_10.npy, gen_50.npy,
# sorted int32 ids of every item in genera drawn at random (seed 1114) until they cover about 1%, 10% and 50%.
import glob, sys
import numpy as np
import pyarrow as pa, pyarrow.parquet as pq, pyarrow.compute as pc

raw, sp, prep, out = sys.argv[1:5]
A, T = [], []
for i in range(20):
    a, b = np.load(f"{raw}/e{i:02d}.npy"), np.load(f"{sp}/e{i:02d}.npy")
    if a.shape != b.shape or not np.array_equal(a.view(np.uint16), b.view(np.uint16)):
        sys.exit(f"file {i}: the embeddings differ from the corpus's")
    A.append(a)
    T.append(pq.read_table(f"{sp}/t{i:02d}.parquet"))
A = np.concatenate(A)
T = pa.concat_tables(T)
print("embeddings equal in all 20 files,", len(A), "rows", flush=True)
rng = np.random.default_rng(20261114)  # prep.py's hold-out, its first draw
held = rng.choice(len(A), 50, replace=False)
keep = np.ones(len(A), bool)
keep[held] = False
X = A[keep].astype(np.float64)
X /= np.linalg.norm(X, axis=1, keepdims=True)
C = np.load(f"{prep}/corpus16.npy", mmap_mode="r")
if X.shape != C.shape or not np.array_equal(X.astype(np.float16).view(np.uint16), np.asarray(C).view(np.uint16)):
    sys.exit("the rows left after the hold-out differ from corpus16.npy")
print("corpus16.npy equals the recovered rows after the hold-out:", X.shape, flush=True)
T = T.filter(pa.array(keep))
pq.write_table(T, f"{out}/taxa.parquet")
N = T.num_rows
for col in ("family", "genus", "species"):
    v = T.column(col)
    print(f"{col}: {pc.count_distinct(v).as_py()} distinct, {v.null_count} null, {pc.sum(pc.equal(v, '')).as_py() or 0} empty", flush=True)
g = np.array(T.column("genus").fill_null("").to_pylist(), dtype=object)
names, inv, counts = np.unique(g, return_inverse=True, return_counts=True)
ok = names != ""
print(f"genera {ok.sum()}, largest {counts[ok].max()} items ({100 * counts[ok].max() / N:.2f}%), median {np.median(counts[ok]):.0f}", flush=True)
rng = np.random.default_rng(1114)
for pct in (1, 10, 50):
    target, total, chosen = pct / 100 * N, 0, []
    for j in rng.permutation(np.flatnonzero(ok)):
        if total + counts[j] > 1.05 * target:
            continue
        chosen.append(j)
        total += counts[j]
        if total >= 0.95 * target:
            break
    ids = np.flatnonzero(np.isin(inv, chosen)).astype(np.int32)
    np.save(f"{out}/gen_{pct}.npy", ids)
    print(f"gen_{pct}: {len(chosen)} genera, {len(ids)} items ({100 * len(ids) / N:.2f}%)", flush=True)
