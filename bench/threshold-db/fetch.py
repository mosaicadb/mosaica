# Fetch 20 random row groups (about 50k items each) of BioCLIP-2 image embeddings into the directory given.
# Same picks as ../../fetch.py (seed 7), so this is the 1M set the Rust prototype was first measured on.
import sys, random, time, numpy as np
from huggingface_hub import HfFileSystem
import pyarrow.parquet as pq
from concurrent.futures import ThreadPoolExecutor
out = sys.argv[1]
fs = HfFileSystem()
d = [x for x in fs.ls("datasets/imageomics/TreeOfLife-200M-Embeddings", detail=False) if x.endswith("bioclip-2_float16")][0]
files = sorted(fs.glob(d + "/*.parquet"))
print(len(files), "files", flush=True)
random.seed(7)
picks = [files[i] for i in sorted(random.sample(range(len(files)), 20))]
def get(i_p):
    i, p = i_p
    for attempt in range(5):
        try:
            pf = pq.ParquetFile(fs.open(p, block_size=8 << 20))
            rg = random.Random(i).randrange(pf.metadata.num_row_groups)
            emb = pf.read_row_group(rg, columns=["emb"]).column("emb").combine_chunks()
            a = emb.values.to_numpy(zero_copy_only=False).reshape(len(emb), -1).astype(np.float16)
            np.save(f"{out}/e{i:02d}.npy", a)
            print(i, p.split("/")[-1], "row group", rg, a.shape, flush=True)
            return
        except Exception as e:
            print("retry", i, repr(e)[:200], flush=True); time.sleep(2 ** attempt)
with ThreadPoolExecutor(4) as ex: list(ex.map(get, enumerate(picks)))
