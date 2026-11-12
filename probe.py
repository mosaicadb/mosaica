from huggingface_hub import HfFileSystem
import pyarrow.parquet as pq
fs = HfFileSystem()
files = sorted(fs.glob("datasets/imageomics/TreeOfLife-200M-Embeddings/**/*.parquet"))
print(len(files), files[:3], files[-1])
f = fs.open(files[100], block_size=4<<20)
pf = pq.ParquetFile(f)
print(pf.schema_arrow)
print(pf.metadata.num_rows, pf.metadata.num_row_groups, [pf.metadata.row_group(i).num_rows for i in range(min(5,pf.metadata.num_row_groups))])
print(pf.metadata.row_group(0).total_byte_size)
