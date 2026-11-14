# Loads the corpus of prep.py into DuckDB, and writes the Parquet file ClickHouse loads (load_clickhouse.sh).
# usage: python3 -I load.py parquet PREP_DIR OUT.parquet      (id UInt32, vec fixed_size_list<float32, 768>)
#        python3 -I load.py duckdb PREP_DIR CORPUS.parquet DB_FILE
import sys, time
import numpy as np

cmd, prep = sys.argv[1], sys.argv[2]
if cmd == "parquet":
    import pyarrow as pa, pyarrow.parquet as pq
    X = np.load(f"{prep}/corpus32.npy", mmap_mode="r")
    N, n = X.shape
    schema = pa.schema([("id", pa.uint32()), ("vec", pa.list_(pa.float32(), n))])
    with pq.ParquetWriter(sys.argv[3], schema, compression="none") as w:
        for i in range(0, N, 65536):
            x = np.ascontiguousarray(X[i:i + 65536])
            vec = pa.FixedSizeListArray.from_arrays(pa.array(x.reshape(-1)), n)
            w.write_table(pa.table({"id": pa.array(np.arange(i, i + len(x), dtype=np.uint32)), "vec": vec}, schema=schema))
    print("wrote", sys.argv[3], N, "rows")
elif cmd == "duckdb":
    import duckdb
    pqf, dbf = sys.argv[3], sys.argv[4]
    con = duckdb.connect(dbf, config={"memory_limit": "12GB", "threads": 4})
    t = time.time()
    con.execute(f"CREATE TABLE t32 AS SELECT id::INTEGER AS id, vec::FLOAT[768] AS vec FROM read_parquet('{pqf}') ORDER BY id")
    for v in ("vis_50", "vis_10", "vis_1"):
        ids = np.load(f"{prep}/{v}.npy")
        con.execute(f"CREATE TABLE v_{v} AS SELECT * FROM (SELECT unnest($1::INTEGER[]) AS id) ORDER BY id", [ids.tolist()])
    con.execute("CHECKPOINT")
    print("loaded in", round(time.time() - t, 1), "s:", con.execute("SELECT count(*) FROM t32").fetchone(),
          [con.execute(f"SELECT count(*) FROM v_{v}").fetchone()[0] for v in ("vis_50", "vis_10", "vis_1")])
