# Recover the taxonomy of the 1M corpus: the same 20 files and row groups as fetch.py (seed 7), read again with the
# taxonomic columns. The embeddings are saved too, so check_species.py can show the rows match e00..e19.npy.
# usage: python3 -I fetch_species.py OUT_DIR [REVISION]
# Writes OUT_DIR/e{i}.npy (f16 embeddings) and OUT_DIR/t{i}.parquet (uuid, kingdom .. species) for i = 0..19.
import random, sys, time
import numpy as np
import pyarrow.parquet as pq
from concurrent.futures import ThreadPoolExecutor
from huggingface_hub import HfFileSystem

out = sys.argv[1]
rev = sys.argv[2] if len(sys.argv) > 2 else "e8ad9d4b4c88ff425860897f80c036563e0ce96d"
COLS = ["uuid", "kingdom", "phylum", "class", "order", "family", "genus", "species"]
fs = HfFileSystem()
root = f"datasets/imageomics/TreeOfLife-200M-Embeddings@{rev}"
d = [x for x in fs.ls(root, detail=False) if x.endswith("bioclip-2_float16")][0]
files = sorted(fs.glob(d + "/*.parquet"))
print(len(files), "files at revision", rev, flush=True)
random.seed(7)
picks = [files[i] for i in sorted(random.sample(range(len(files)), 20))]


def get(i_p):
    i, p = i_p
    for attempt in range(5):
        try:
            pf = pq.ParquetFile(fs.open(p, block_size=8 << 20))
            rg = random.Random(i).randrange(pf.metadata.num_row_groups)
            t = pf.read_row_group(rg, columns=["emb"] + COLS)
            emb = t.column("emb").combine_chunks()
            np.save(f"{out}/e{i:02d}.npy", emb.values.to_numpy(zero_copy_only=False).reshape(len(emb), -1).astype(np.float16))
            pq.write_table(t.select(COLS), f"{out}/t{i:02d}.parquet")
            print(i, p.split("/")[-1], "row group", rg, t.num_rows, flush=True)
            return
        except Exception as e:
            print("retry", i, repr(e)[:200], flush=True)
            time.sleep(2 ** attempt)
    raise SystemExit(f"file {i} failed")


with ThreadPoolExecutor(4) as ex:
    list(ex.map(get, enumerate(picks)))
