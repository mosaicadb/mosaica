# Times the threshold filter in ClickHouse or DuckDB, on the queries, thresholds and visible sets of prep.py.
# usage: python3 -I dbrun.py clickhouse|duckdb PREP_DIR OUT_CSV [--table T] [--form F] [--vis all,vis_50,vis_10,vis_1]
#        [--threads 1,4] [--step 5] [--reps 5] [--dump DIR] [--db FILE] [--url URL]
# Each (query, share, visible set) runs --reps times, timed at the client from sending the SQL to holding the ids as a
# numpy array. Writes the per-query median and minimum, as the Rust runner's dbcmp does; with --dump, the 1-thread
# answers go to DIR/<what>_<vis>.bin as (query, share, count, ids...) u32 records.
import argparse, http.client, os, sys, time, urllib.parse
import numpy as np

ap = argparse.ArgumentParser()
ap.add_argument("system", choices=["clickhouse", "duckdb"])
ap.add_argument("prep")
ap.add_argument("out")
ap.add_argument("--table", default="t32")
ap.add_argument("--form", default="dot")  # dot | cos (the similarity), and after a '+' the visible-set form: in | bitmap | join
ap.add_argument("--vis", default="all,vis_50,vis_10,vis_1")
ap.add_argument("--threads", default="1,4")
ap.add_argument("--step", type=int, default=5)
ap.add_argument("--reps", type=int, default=5)
ap.add_argument("--shares", default="0,1,2,3,4")
ap.add_argument("--dump", default="")
ap.add_argument("--db", default="")
ap.add_argument("--url", default="127.0.0.1:18123")
ap.add_argument("--memory-limit", default="12GB", help="DuckDB's memory_limit")
ap.add_argument("--cold", action="store_true", help="first, each share's first query once after the page cache is "
                "dropped: --evict-cmd for ClickHouse; for DuckDB a new connection and this cgroup's memory.reclaim")
ap.add_argument("--evict-cmd", default="")
a = ap.parse_args()

Q = np.load(f"{a.prep}/queries.npy")
T = np.load(f"{a.prep}/thresholds.npy")
sim, _, visform = a.form.partition("+")
visform = visform or "in"


def lit(q):
    return "[" + ",".join(repr(float(x)) for x in q.astype(np.float32)) + "]"


def sql(qi, si, vis):
    q, t = Q[qi], float(T[qi, si])
    if a.system == "clickhouse":
        cond = f"dotProduct(vec, {lit(q)}::Array(Float32)) >= {t!r}" if sim == "dot" else \
            f"cosineDistance(vec, {lit(q)}::Array(Float32)) <= {1 - t!r}"
        if vis == "all":
            return f"SELECT id FROM {a.table} WHERE {cond} SETTINGS optimize_move_to_prewhere = 0"
        if visform == "in":  # a Set-engine table: the set is built once, at load
            return f"SELECT id FROM {a.table} PREWHERE id IN s_{vis} WHERE {cond}"
        if visform == "bitmap":  # a stored groupBitmap state
            return f"SELECT id FROM {a.table} PREWHERE bitmapContains((SELECT bm FROM b_{vis}), id) WHERE {cond}"
        return f"SELECT t.id FROM {a.table} AS t INNER JOIN v_{vis} AS v ON t.id = v.id WHERE {cond}"
    cond = f"array_inner_product(vec, {lit(q)}::FLOAT[768]) >= {t!r}" if sim == "dot" else \
        f"array_cosine_similarity(vec, {lit(q)}::FLOAT[768]) >= {t!r}"
    if vis == "all":
        return f"SELECT id FROM {a.table} WHERE {cond}"
    if visform == "in":
        return f"SELECT id FROM {a.table} WHERE id IN (SELECT id FROM v_{vis}) AND {cond}"
    if visform == "mat":  # the visible rows first, materialised, then the similarity on them alone
        return (f"WITH v AS MATERIALIZED (SELECT t.id, t.vec FROM {a.table} AS t SEMI JOIN v_{vis} USING (id)) "
                f"SELECT id FROM v WHERE {cond}")
    return f"SELECT t.id FROM {a.table} AS t SEMI JOIN v_{vis} AS v ON t.id = v.id WHERE {cond}"


if a.system == "clickhouse":
    conn = http.client.HTTPConnection(a.url)

    def run(q, th):
        params = urllib.parse.urlencode({"max_threads": th, "use_query_cache": 0, "use_query_condition_cache": 0,
                                         "default_format": "RowBinary"})
        conn.request("POST", "/?" + params, body=q.encode())
        r = conn.getresponse()
        body = r.read()
        if r.status != 200:
            sys.exit(f"ClickHouse {r.status}: {body[:500].decode(errors='replace')}")
        return np.frombuffer(body, dtype="<u4")

    def set_threads(th):
        pass
else:
    import duckdb
    con = duckdb.connect(a.db, read_only=True, config={"memory_limit": a.memory_limit})

    def run(q, th):
        return con.execute(q).fetchnumpy()["id"].astype(np.uint32)

    def set_threads(th):
        con.execute(f"SET threads = {th}")

HEADER = "what,threads,vis,share_index,query,median_ms,min_ms,count" + (",kind" if a.cold else "")


def evict(th):
    """Drops the data from the page cache before a cold query (and DuckDB's buffer pool, by reconnecting)."""
    global con
    if a.evict_cmd:
        import subprocess
        subprocess.run(a.evict_cmd, shell=True, check=True)
    if a.system == "duckdb":
        con.close()
        cg = "/sys/fs/cgroup" + open("/proc/self/cgroup").read().strip().split("::", 1)[1]
        try:
            with open(f"{cg}/memory.reclaim", "w") as f:
                f.write(str(16 << 30))
        except OSError:
            pass
        con = duckdb.connect(a.db, read_only=True, config={"memory_limit": a.memory_limit})
        set_threads(th)
what = f"{a.system}_{a.table}_{a.form}"
shares = [int(s) for s in a.shares.split(",")]
t_start = time.time()
for vis in a.vis.split(","):
    for th in [int(x) for x in a.threads.split(",")]:
        set_threads(th)
        dump = open(f"{a.dump}/{what}_{vis}.bin", "wb") if a.dump and th == 1 else None
        rows, meds = [], []
        for si in shares if a.cold else []:
            evict(th)
            t0 = time.perf_counter()
            ids = run(sql(0, si, vis), th)
            ms = (time.perf_counter() - t0) * 1e3
            rows.append(f"{what},{th},{vis},{si},0,{ms:.4f},{ms:.4f},{len(ids)},cold")
        for qi in range(0, len(Q), a.step):
            for si in shares:
                s = sql(qi, si, vis)
                ts = []
                for _ in range(a.reps):
                    t0 = time.perf_counter()
                    ids = run(s, th)
                    ts.append((time.perf_counter() - t0) * 1e3)
                ids = np.sort(ids)
                if dump:
                    dump.write(np.array([qi, si, len(ids)], "<u4").tobytes() + ids.astype("<u4").tobytes())
                meds.append(float(np.median(ts)))
                rows.append(f"{what},{th},{vis},{si},{qi},{np.median(ts):.4f},{min(ts):.4f},{len(ids)}" + (",warm" if a.cold else ""))
        if dump:
            dump.close()
        new = not os.path.exists(a.out) or os.path.getsize(a.out) == 0
        with open(a.out, "a") as f:
            f.write((HEADER + "\n" if new else "") + "\n".join(rows) + "\n")
        print(f"{what} threads {th} vis {vis}: {len(meds)} timings, median {np.median(meds):.2f} ms "
              f"[{time.time() - t_start:.0f}s]", flush=True)
