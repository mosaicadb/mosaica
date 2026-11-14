# Fetch the branch's 1M BioCLIP-2 sample (same 20 row groups as research/exact-vector-search fetch.py) plus metadata.
import sys, os, numpy as np, random, time
from huggingface_hub import HfFileSystem
import pyarrow.parquet as pq
from concurrent.futures import ThreadPoolExecutor
W = sys.argv[1]
fs = HfFileSystem()
d = "datasets/imageomics/TreeOfLife-200M-Embeddings/bioclip-2_float16"
files = sorted(fs.glob(d + "/*.parquet"))
print(len(files), flush=True)
random.seed(7)
picks = [files[i] for i in sorted(random.sample(range(len(files)), 20))]
def get(i_p):
    i, p = i_p
    if os.path.exists(f"{W}/data/e{i:02d}.npy") and os.path.exists(f"{W}/meta/m{i:02d}.parquet"): return
    for attempt in range(5):
        try:
            f = fs.open(p, block_size=8 << 20)
            pf = pq.ParquetFile(f)
            rg = random.Random(i).randrange(pf.metadata.num_row_groups)
            t = pf.read_row_group(rg)
            emb = t.column("emb").combine_chunks()
            a = emb.values.to_numpy(zero_copy_only=False).reshape(len(emb), -1).astype(np.float16)
            np.save(f"{W}/data/e{i:02d}.npy", a)
            pq.write_table(t.drop_columns(["emb"]), f"{W}/meta/m{i:02d}.parquet")
            print(i, p.split("/")[-1], rg, a.shape, flush=True)
            return
        except Exception as e:
            print("retry", i, repr(e)[:200], flush=True); time.sleep(2 ** attempt)
with ThreadPoolExecutor(4) as ex: list(ex.map(get, enumerate(picks)))
print("done", flush=True)
