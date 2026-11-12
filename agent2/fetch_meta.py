import sys, json, numpy as np
from huggingface_hub import HfFileSystem
import pyarrow.parquet as pq
fs = HfFileSystem()
d = "datasets/imageomics/TreeOfLife-200M-Embeddings/bioclip-2_float16"
picks = {0: (38, 6), 5: (88, 4), 10: (246, 4), 15: (444, 1)}
for i, (fn, rg) in picks.items():
    f = fs.open(f"{d}/train-{fn:05d}-of-00666.parquet", block_size=4 << 20)
    pf = pq.ParquetFile(f)
    if i == 0: print(pf.schema_arrow)
    cols = [c for c in pf.schema_arrow.names if c != "emb"]
    t = pf.read_row_group(rg, columns=cols)
    pq.write_table(t, f"{sys.argv[1]}/m{i:02d}.parquet")
    # check alignment with the stored embeddings: read first 3 emb rows
    print(i, t.num_rows, {c: len(set(t.column(c).to_pylist())) for c in cols if c in ("kingdom","phylum","class","order","family","genus","species","common_name","scientific_name")})
