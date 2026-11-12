import sys, numpy as np, random, time
from huggingface_hub import HfFileSystem
import pyarrow.parquet as pq
from concurrent.futures import ThreadPoolExecutor
fs = HfFileSystem()
dirs = [d for d in fs.ls("datasets/imageomics/TreeOfLife-200M-Embeddings", detail=False)]
print(dirs, flush=True)
d = [x for x in dirs if x.endswith("bioclip-2_float16")][0]
files = sorted(fs.glob(d + "/*.parquet"))
print(len(files), flush=True)
random.seed(7)
picks = [files[i] for i in sorted(random.sample(range(len(files)), 20))]
def get(i_p):
    i, p = i_p
    for attempt in range(4):
        try:
            f = fs.open(p, block_size=8 << 20)
            pf = pq.ParquetFile(f)
            rg = random.Random(i).randrange(pf.metadata.num_row_groups)
            t = pf.read_row_group(rg, columns=["emb", "species"])
            emb = t.column("emb").combine_chunks()
            a = emb.values.to_numpy(zero_copy_only=False).reshape(len(emb), -1).astype(np.float16)
            np.save(f"data/e{i:02d}.npy", a)
            sp = t.column("species").to_pylist()
            print(i, p.split("/")[-1], rg, a.shape, len(set(sp)), flush=True)
            return
        except Exception as e:
            print("retry", i, repr(e)[:200], flush=True); time.sleep(2 ** attempt)
with ThreadPoolExecutor(4) as ex: list(ex.map(get, enumerate(picks)))
