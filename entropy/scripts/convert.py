import sys, numpy as np, glob
mode, src, dst = sys.argv[1:4]
rng = np.random.default_rng(0)
if mode == "h5":
    import h5py
    f = h5py.File(src, "r"); d = f["train"]; N = d.shape[0]; print(src, d.shape, d.dtype)
    idx = np.sort(rng.choice(N, min(N, 200000), replace=False))
    out = np.empty((len(idx), d.shape[1]), np.float32)
    for s in range(0, N, 100000):
        blk = d[s:s + 100000]; sel = idx[(idx >= s) & (idx < s + 100000)]
        lo = np.searchsorted(idx, s); out[lo:lo + len(sel)] = blk[sel - s]
else:
    import pyarrow.parquet as pq
    parts = []
    for p in sorted(glob.glob(src + "/*.parquet")):
        pf = pq.ParquetFile(p); print(p, pf.schema_arrow.names, pf.metadata.num_rows)
        col = [c for c in pf.schema_arrow.names if (("3072" in c) if mode == "pq3072" else ("embedding" in c or c in ("openai", "emb")))][0]
        for rg in range(pf.metadata.num_row_groups):
            t = pf.read_row_group(rg, columns=[col]).column(col).combine_chunks()
            if hasattr(t, "values"): a = t.values.to_numpy(zero_copy_only=False).reshape(len(t), -1)
            else: a = np.array(t.to_pylist())
            parts.append(a.astype(np.float16))
    out = np.concatenate(parts)[:200000]
nrm = np.linalg.norm(out.astype(np.float32), axis=1); print("norms", nrm.min(), nrm.mean(), nrm.max(), "zero rows", (nrm == 0).sum())
np.save(dst, out.astype(np.float16)); print(dst, out.shape)
