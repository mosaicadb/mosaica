# The 5M source: the 1M rows (c1m_x.npy, then the 1,000 held-out rows) and 200,000 more rows from each of the 20 shard
# files that fetch.py drew from, sampled at random from the rows outside the row group fetch.py took. Writes
# c5m_x.npy (5M x 768 f16) and c5m_shard.npy (shard of each row; -1 for the c1m rows, whose shards are in c1m_shard.npy).
import sys, re, time, numpy as np, pyarrow.parquet as pq
from huggingface_hub import HfFileSystem
D = sys.argv[1]
picks = {}
for line in open(f"{D}/fetch.log"):
    m = re.match(r"(\d+) (train-\S+\.parquet) (\d+) \(50000, 768\)", line)
    if m: picks[int(m[1])] = (m[2], int(m[3]))
assert len(picks) == 20
fs = HfFileSystem(); base = "datasets/imageomics/TreeOfLife-200M-Embeddings/bioclip-2_float16/"
c1 = np.load(f"{D}/c1m_x.npy", mmap_mode="r"); hold = np.load(f"{D}/c1m_item_q.npy")
N = len(c1) + len(hold) + 20 * 200_000
out = np.lib.format.open_memmap(f"{D}/c5m_x.npy", mode="w+", dtype=np.float16, shape=(N, 768))
shard = np.full(N, -1, np.int16)
for i in range(0, len(c1), 100_000): out[i:min(i + 100_000, len(c1))] = c1[i:i + 100_000]
out[len(c1):len(c1) + len(hold)] = hold.astype(np.float16)
assert np.array_equal(out[len(c1):len(c1) + len(hold)].astype(np.float32), hold)
pos = len(c1) + len(hold)
for i in range(20):
    fn, rg = picks[i]
    for attempt in range(5):
        try:
            pf = pq.ParquetFile(fs.open(base + fn, block_size=16 << 20))
            sizes = [pf.metadata.row_group(g).num_rows for g in range(pf.metadata.num_row_groups)]
            starts = np.r_[0, np.cumsum(sizes)]
            others = np.setdiff1d(np.arange(starts[-1]), np.arange(starts[rg], starts[rg + 1]))
            assert len(others) >= 200_000, f"shard {i} has only {len(others)} rows outside its row group"
            take = np.sort(np.random.default_rng(1000 + i).choice(others, 200_000, replace=False))
            got = 0
            for g in range(len(sizes)):
                if g == rg: continue
                sel = take[(take >= starts[g]) & (take < starts[g + 1])] - starts[g]
                if not len(sel): continue
                emb = pf.read_row_group(g, columns=["emb"]).column("emb").combine_chunks()
                a = emb.values.to_numpy(zero_copy_only=False).reshape(len(emb), -1).astype(np.float16)
                out[pos + got:pos + got + len(sel)] = a[sel]; got += len(sel)
            assert got == 200_000
            break
        except Exception as e:
            print("retry", i, repr(e)[:200], flush=True); time.sleep(2 ** attempt)
    else:
        sys.exit(f"shard {i} failed")
    shard[pos:pos + 200_000] = i; pos += 200_000
    print(i, fn, "rows", starts[-1], "row groups", len(sizes), "taken 200000 outside group", rg, flush=True)
out.flush(); np.save(f"{D}/c5m_shard.npy", shard)
print("done", N, flush=True)
