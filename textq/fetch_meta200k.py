# Metadata (every column but emb) for the rows of vexact-data/bioclip2-200k, from the row groups fetch_bioclip.py read
# (its log: files 463 / 573 / 476 / 462, row groups 1 / 5 / 3 / 4, first 50k rows each). Also reads emb and checks
# that it equals g0-g3.npy row for row, and fetches BioCLIP-2's open_clip config.
# usage: fetch_meta200k.py OUTDIR CORPUSDIR
import sys, json, numpy as np, pyarrow.parquet as pq
from huggingface_hub import HfFileSystem
OUT, CD = sys.argv[1], sys.argv[2]; fs = HfFileSystem()
d = "datasets/imageomics/TreeOfLife-200M-Embeddings/bioclip-2_float16"
for j, (fn, rg) in enumerate(((463, 1), (573, 5), (476, 3), (462, 4))):
    pf = pq.ParquetFile(fs.open(f"{d}/train-{fn:05d}-of-00666.parquet", block_size=8 << 20))
    t = pf.read_row_group(rg).slice(0, 50_000)
    emb = t.column("emb").combine_chunks(); a = emb.values.to_numpy(zero_copy_only=False).reshape(len(emb), -1).astype(np.float16)
    g = np.load(f"{CD}/g{j}.npy"); same = a.shape == g.shape and bool((a == g).all())
    pq.write_table(t.drop(["emb"]), f"{OUT}/meta_g{j}.parquet")
    print(j, fn, rg, t.num_rows, "emb equals g%d.npy:" % j, same, "species", len(set(t.column("species").to_pylist())), flush=True)
    if j == 0: print(t.schema)
with fs.open("imageomics/bioclip-2/open_clip_config.json") as f: cfg = json.load(f)
json.dump(cfg, open(f"{OUT}/open_clip_config.json", "w"), indent=1); print("config", cfg["model_cfg"]["text_cfg"])
