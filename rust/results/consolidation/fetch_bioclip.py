# Fetch BioCLIP-2 float16 image embeddings from imageomics/TreeOfLife-200M-Embeddings: whole row groups from parquet
# files chosen with a fixed seed, until N rows; writes OUT/emb.npy (N x 768 float16) and OUT/src.txt (file, row group).
# usage: python3 -I fetch_bioclip.py N OUT [SEED]
import os, random, sys, time
import numpy as np
import pyarrow.parquet as pq
from huggingface_hub import HfFileSystem

N, OUT = int(sys.argv[1]), sys.argv[2]
seed = int(sys.argv[3]) if len(sys.argv) > 3 else 7
os.makedirs(OUT, exist_ok=True)
fs = HfFileSystem()
root = [d for d in fs.ls("datasets/imageomics/TreeOfLife-200M-Embeddings", detail=False) if d.endswith("bioclip-2_float16")][0]
files = sorted(fs.glob(root + "/*.parquet"))
rng = random.Random(seed)
order = rng.sample(range(len(files)), len(files))
parts, got, log = [], 0, []
for fi in order:
    if got >= N:
        break
    for attempt in range(4):
        try:
            pf = pq.ParquetFile(fs.open(files[fi], block_size=8 << 20))
            rg = rng.randrange(pf.metadata.num_row_groups)
            emb = pf.read_row_group(rg, columns=["emb"]).column("emb").combine_chunks()
            a = emb.values.to_numpy(zero_copy_only=False).reshape(len(emb), -1).astype(np.float16)
            break
        except Exception as e:
            print("retry", fi, repr(e)[:160], flush=True)
            time.sleep(2 ** attempt)
    else:
        continue
    parts.append(a)
    got += len(a)
    log.append(f"{files[fi].split('/')[-1]} {rg} {len(a)}")
    print(f"{got} rows ({files[fi].split('/')[-1]} row group {rg})", flush=True)
X = np.concatenate(parts)[:N]
np.save(os.path.join(OUT, "emb.npy"), X)
open(os.path.join(OUT, "src.txt"), "w").write("\n".join(log) + "\n")
print("saved", X.shape)
