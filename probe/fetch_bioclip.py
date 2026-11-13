# Fetch 4 random row groups (at most 200k vectors) of BioCLIP-2 image embeddings into the directory given.
import sys, random, time, numpy as np
from huggingface_hub import HfFileSystem
import pyarrow.parquet as pq
out = sys.argv[1]
fs = HfFileSystem()
d = [x for x in fs.ls("datasets/imageomics/TreeOfLife-200M-Embeddings", detail=False) if x.endswith("bioclip-2_float16")][0]
files = sorted(fs.glob(d + "/*.parquet"))
print(len(files), "files", flush=True)
random.seed(11)
picks = random.sample(range(len(files)), 4)
total = 0
for j, i in enumerate(picks):
    for attempt in range(5):
        try:
            pf = pq.ParquetFile(fs.open(files[i], block_size=8 << 20))
            rg = random.Random(i).randrange(pf.metadata.num_row_groups)
            emb = pf.read_row_group(rg, columns=["emb"]).column("emb").combine_chunks()
            a = emb.values.to_numpy(zero_copy_only=False).reshape(len(emb), -1).astype(np.float16)[: 50_000]
            np.save(f"{out}/g{j}.npy", a); total += len(a)
            print(j, files[i].split("/")[-1], "row group", rg, a.shape, flush=True)
            break
        except Exception as e:
            print("retry", j, repr(e)[:200], flush=True); time.sleep(2 ** attempt)
print("total", total)
