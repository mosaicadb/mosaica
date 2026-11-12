import sys, numpy as np, random, time
from huggingface_hub import HfFileSystem
import pyarrow.parquet as pq
fs = HfFileSystem()
d = "datasets/imageomics/TreeOfLife-200M-Embeddings/bioclip-2.5-vith14_float16"
files = sorted(fs.glob(d + "/*.parquet")); print(len(files), flush=True)
random.seed(7)
picks = [files[i] for i in sorted(random.sample(range(len(files)), 4))]
for i, p in enumerate(picks):
    for attempt in range(4):
        try:
            f = fs.open(p, block_size=8 << 20); pf = pq.ParquetFile(f)
            rg = random.Random(i).randrange(pf.metadata.num_row_groups)
            t = pf.read_row_group(rg, columns=["emb"]); emb = t.column("emb").combine_chunks()
            a = emb.values.to_numpy(zero_copy_only=False).reshape(len(emb), -1).astype(np.float16)[:50000]
            np.save(f"data1024/f{i:02d}.npy", a); print(i, p.split("/")[-1], rg, a.shape, flush=True); break
        except Exception as e:
            print("retry", i, repr(e)[:200], flush=True); time.sleep(2 ** attempt)
