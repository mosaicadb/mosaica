# Comparative benchmark

Mosaica against other systems that draw a map over many points, each asked for the same frames and
aggregations as the same viewer, with every answer checked against an exact oracle.
[METHOD.md](METHOD.md) defines the frame, the viewer, the workloads, the metrics and the fairness
rules. This file is operational, and lists where stage 1 departs from the method.

Stage 1 covers the shared harness and Mosaica, ClickHouse, PostGIS and OpenSearch, on the
PaperSeek data at 10⁶ points for development only. Each system runs two frames: `points`, the
default, in which every dense tile sends a sample of K of its matching items, and `counts`, the
earlier frame, in which it sends none (see "The frame"). Runs write to `results/`, which
is not tracked. **No figure from this directory is fit to publish yet**: every record carries
`"development_only": true`, and the machine it ran on was shared with other work.

## Running it from scratch

Everything below runs from the repository root, with `DATA` the directory that holds the
`ladder/` corpora and `bench-compare/` (`data/` in the main checkout). Build steps run under
`nice -n 19`.

```bash
# 1. Python 3.10 (tested on 3.10.12), every package pinned with its hashes, in a venv inside this
#    directory that git ignores
python3 -m venv bench/compare/.venv
bench/compare/.venv/bin/pip install --require-hashes -r bench/compare/requirements.lock

# 2. The tiers, from the PaperSeek rung's points file (52 GB, of which only seven columns are
#    read). About 15 seconds for 1e6 and 1e7; `full` writes all 102,117,343 rows.
cd bench/compare && nice -n 19 .venv/bin/python prepare.py \
    --source $DATA/ladder/paperseek/points.parquet --data $DATA/bench-compare --tiers 1e6,1e7 && cd -

# 3. The cameras for a tier (checked in for paperseek-1e6)
cd bench/compare && .venv/bin/python cameras.py \
    --tier $DATA/bench-compare/tiers/paperseek-1e6.parquet --out cameras/paperseek-1e6.json && cd -

# 4. The Mosaica image, from the repository's Dockerfile at this checkout's commit; the record
#    keeps its image id and the version it prints
docker build --build-arg MOSAICA_BUILD_COMMIT=$(git rev-parse --short=9 HEAD) \
    --build-arg MOSAICA_VERSION=$(python3 scripts/version.py) -t bench-compare/mosaica .

# 5. The ClickHouse image (26.8.34.13), by the digest adapters/clickhouse.py runs
docker pull clickhouse/clickhouse-server@sha256:8b28eb09479c36c4533eec7a6268917d04ba3a6fd90e3f4205f70e761b6d7779
#    and the PostGIS image (PostgreSQL 18.6, PostGIS 3.6.4), by the digest adapters/postgis.py runs
docker pull postgis/postgis@sha256:4a43e3c3e5d55b9208240fc71db543b79c638766280c625a425bd0a2390989c4

# 5a. The OpenSearch image (3.9.0), by the digest adapters/opensearch.py runs
docker pull opensearchproject/opensearch@sha256:3b55f6b6249cfdb92b52ee59adc8a266203ba9ef9f5d82544fad6f1af2f3609e

# 6. One run per system; each appends a record to results/dev.jsonl
cd bench/compare
for s in mosaica clickhouse postgis postgis+leakproof postgis+integer opensearch; do
  for c in compute defaults; do
    for d in points counts; do
      .venv/bin/python run.py --system $s --caches $c --dense $d \
          --tier $DATA/bench-compare/tiers/paperseek-1e6.parquet \
          --cameras cameras/paperseek-1e6.json --work $DATA/bench-compare/work
    done
  done
done

# 7. The table
.venv/bin/python tables.py results/dev.jsonl
```

`run.py` refuses to start above a one-minute load of 3 (`--max-load`). On a shared machine, for a
development run whose timings do not matter, raise it, and pass `--nice 19`, which runs every
system's build at that niceness (Mosaica's `mosaica build`, and ClickHouse's server for the
duration of its `INSERT` and `OPTIMIZE`) and every server at 0. The load before and after is in
the record. A run builds the system from nothing, checks every frame and aggregation against the
oracle, runs each workload five times warm, measures server start and session start five times warm
and five times cold, then times one cold frame per battery camera, and stops and removes everything it started.
Each run works in a directory of its own, `$DATA/bench-compare/work/<system>-<caches>-<tier>-<pid>`,
and removes it when it ends, and each server listens on ports the kernel reports free when the run
starts (`adapters/base.py`, `free_ports`), so runs side by side do not collide.

Every system's build and server run in Docker containers, each capped at 16 GiB with no swap and
run as the invoking user, so that each cgroup's `memory.peak` is that system's own
(`adapters/base.py`, `Container`). A container's cgroup is read at Docker's systemd driver path on
cgroup v2, and a run stops with an error where it is missing. Each container runs the host's
`/usr/bin/nice` and `/usr/bin/sleep`, mounted read-only, because the Mosaica image has neither;
both link only to the C library, and a run checks that they start in each image before using it.

Build time excludes each server's start in the same way, and ends with the data on disc.
ClickHouse's is from `CREATE TABLE` to the end of `OPTIMIZE` and an `fsync` of every file in its
data directory, on a running server, leaving out those inactive parts; Mosaica's build syncs its own files
before it exits. Mosaica's build runs by `docker exec` in a container kept
alive for it, so its cgroup's peak can be read once the build has ended, and its time is the
exec's wall time less the median of three empty execs in that container; both figures are in the
record. Size on disk is ClickHouse's data directory, less the inactive parts that `OPTIMIZE` replaced
and that ClickHouse keeps for `old_parts_lifetime` (their count and size are in the record), and
Mosaica's bundle directory, each by `du`.

Each system runs twice. With `--caches compute`, every cache of a request's result or of a
resolved filter is off, and the state a system keeps for a session or a grant stays as it is by
default, since every deployment keeps it; with `--caches defaults`, every cache is at the system's
default. The table gives each its own column, and each record lists every cache setting.

| System | Setting | What it holds | Compute | Defaults |
|---|---|---|---|---|
| ClickHouse | `use_query_cache` | query results | off | off |
| ClickHouse | `use_query_condition_cache`, `use_query_condition_cache_for_top_k` | which granules a filter matched | off | on |
| ClickHouse | mark cache, primary index, compiled expressions | the table's index and code: state | default | default |
| Mosaica | `region_cache_bytes` | resolved `region` filters | 0 | 256 MiB |
| Mosaica | `masked_count_cache_bytes`, `figures_disk_bytes` | annotation layers' counts; this corpus has no layer, so they hold nothing | 0 | 1 GiB, 8 GiB |
| Mosaica | `row_projection_cache_bytes` | each grant's projection of the rows it may see: state | default | 2 GiB |
| Mosaica | `fragment_cache_bytes` | each grant's visibility masks in memory: state | default | 1 GiB |
| Mosaica | `occupancy_cache_bytes` | each viewer's occupied-tile counts, from which sampling is set: state | default | 32 MiB |
| PostGIS | none | PostgreSQL keeps no cache of results, so its two modes run the same server | – | – |
| PostGIS | `shared_buffers`, and the machine's page cache | the table's and index's pages: state | 4 GB | 4 GB |
| PostGIS | each connection's catalogue and plan caches | the schema, and statements a client prepares; the adapter prepares none (`prepare_threshold=None`): state | default | default |
| OpenSearch | `index.requests.cache.enable` | the shard request cache: whole results of `size: 0` searches. The security plugin bypasses it for a search under document-level security, so it never serves the viewer in either mode | off | on |
| OpenSearch | `index.queries.cache.enabled` | the node query cache: which documents a filter clause matched, per segment | off | on |
| OpenSearch | field-data cache, the security plugin's cache of authenticated users and their roles (`plugins.security.cache.ttl_minutes`), compiled scripts | state; this index has no `text` field, so field data holds nothing | default | default |

Each Mosaica record holds the admission limits and cache bounds the server reports in
`/control/status`; it does not report `compute_threads`, which defaults to the CPUs the container
may use. Mosaica also writes each grant's visibility masks to its cache directory, which no setting
bounds; they are grant state, kept in both modes, and deleted by a cold start.

ClickHouse runs at its defaults apart from the sort key, `ORDER BY m`, the cache settings above
and its system log tables, which are off. Its build alone sets `old_parts_lifetime = 0`, so that
the parts inserts and `OPTIMIZE` replace are deleted once no query holds them rather than kept
for eight minutes, and resets it before serving: at GBIF's 10⁹ those parts took the build's disc
use to 57 GB for a 10 GiB table. It is a disc setting and touches no query. A dense tile's sample is its K matching rows of lowest
`rank`, by `groupArraySorted(K)((rank, qx, qy))` grouped by tile and unnested with `arrayJoin`,
or at zoom 0, where the screen is one tile, by `ORDER BY rank LIMIT K`; one query answers sparse
and dense tiles alike. Three shapes,
each sending the same points, were measured once over every frame of the three workloads at 10⁷,
warm, on a loaded machine (median ms by zoom):

| Query | Zoom 0 | 3 | 4 | 6 | 8 | 12 | 16 | All frames |
|---|---|---|---|---|---|---|---|---|
| **`ORDER BY tile, rank LIMIT K BY tile` (used)** | 29 | 269 | 35 | 24 | 10 | 7.2 | 7.2 | **2.6 s** |
| `arrayJoin(groupArraySorted(K)((rank, qx, qy)))` `GROUP BY` tile | 117 | 158 | 40 | 38 | 13 | 5.5 | 6.2 | 2.6 s |
| `row_number() OVER (PARTITION BY` tile `ORDER BY rank) <= K` | 844 | 451 | 63 | 31 | 7.9 | 6.2 | 5.9 | 5.3 s |

The aggregation was faster at zoom 3 and slower at zoom 0, and level overall, at 10⁷. At GBIF's
5 × 10⁸ it was twice as fast on the slow zoom-6 frames, as the viewer with compute caches, each
query run alone three times, and sent the same 7,500 points:

| Zoom-6 frame (rows read) | Grid query | `LIMIT BY` | `groupArraySorted` |
|---|---|---|---|
| `r0-z6` (100.1 M) | 8.4 s | 13.2–16.4 s, 1.72 GiB | 8.7–9.1 s, 88 MiB |
| `r0-z6-pan0` (76.8 M) | 2.6–2.7 s | 5.7–6.5 s, 1.66 GiB | 2.8–2.9 s, 88 MiB |
| `r1-z6` (88.7 M) | 2.9–3.1 s | 5.7–6.0 s, 1.85 GiB | 3.4 s, 87 MiB |

`LIMIT BY`'s pipeline sorts every matching row in twelve streams and merges them into one, where
the aggregation keeps K ranks a tile in each of twelve streams, as the grid query's does; neither
spilled. So the frame uses the aggregation, and at zoom 0, where the aggregation was slower and
the screen is one tile, a top K by `ORDER BY rank LIMIT K`. `rank` is a plain column,
outside the sort key, since no query filters by it; ClickHouse has no secondary index that
returns rows in an order (its skipping indexes only skip granules), and reading in order applies
to the sort key alone. `SAMPLE` was not used: it needs a sampling
key in the table's sort key, samples a fraction of the whole table rather than K a tile, and so
cannot give exactly K from each dense tile. It leaves `index_granularity` (8192),
`max_insert_threads` and `max_threads` (both `auto`, 12 on this machine) at their defaults, and
each record lists them. Its filter predicates (`adapters/clickhouse.py`, `filter_sql`) have the
same SQL shapes as the oracle's; the oracle is DuckDB over the Parquet file, so the two share no
code and no engine, but a mistake in a predicate's shape would be made in both.

PostGIS is PostgreSQL 18.6 with PostGIS 3.6.4 (`adapters/postgis.py`), run as three arms that
share the adapter, the table's columns, the viewer's policy and every query but the condition that
selects the camera's tiles:

- **`postgis`** indexes `geom`, each position as a point in quantised space, with GiST, the index
  PostGIS's documentation recommends, and stores the table in `geom`'s order, which PostGIS sorts
  along a Hilbert curve. Under row-level security the viewer's queries cannot use that index, so
  each scans the whole table. This is what a deployment that follows PostGIS's documentation and
  restricts its viewers with row-level security gets.
- **`postgis+leakproof`** is the same table and index, and the superuser marks `&&`'s function
  leakproof, so the viewer's queries use the index.
- **`postgis+integer`** has no geometry: a B-tree on `m`, the Morton code of `(qx, qy)` as
  ClickHouse stores it, with the table in that order, so a tile is one range of `m`. A B-tree
  compares `bigint`, which PostgreSQL marks leakproof, so the viewer's queries use it under the
  same policy with no claim made.

The build streams the tier from pyarrow's CSV writer into `COPY staging FROM STDIN`, since `COPY`
is the bulk load PostgreSQL documents and the Parquet file is on the client's side. Binary `COPY`
would mean psycopg encoding each row in Python, which took three times as long as CSV. Then
`CREATE TABLE points AS SELECT ... ORDER BY` the arm's key quantises each position by the frame's
rule, with every operand a `float8`, into `qx` and `qy`, `bigint` since PostgreSQL has no unsigned
32-bit integer, and in the integer arm `m`, the two interleaved by shifts and masks and offset by
2⁶³ so that `bigint`'s order is the code's; one index build, statistics on the cells, `VACUUM ANALYZE`, a `CHECKPOINT` and an
`fsync` of the data directory follow. The build time runs from creating the staging table to the
end of the `fsync`; `initdb`, which the image runs at the container's first start, is the
server's setup and falls outside it, as ClickHouse's start does. The loader runs in the harness's
process, in a thread at the build's niceness, so the build's cgroup peak is the server's alone.
Size on disk is the data directory by `du`, less `pg_wal` and the template databases, as
ClickHouse's is less the parts it replaced; each record also gives the WAL's, the templates', the
table's and the index's sizes. The GiST arms select the camera's tiles with `geom &&
ST_MakeEnvelope(...)` and the same bounds on `qx` and `qy`, because PostGIS compares boxes in
single precision, rounded outwards, and `&&` alone admits points just outside a tile; the integer
arm selects them by their ranges of `m`, adjacent ranges joined, `OR`ed in one condition. Server start ends when the superuser's `SELECT 1 FROM points LIMIT 1`
answers; session start holds the viewer's two connections, each authenticated by SCRAM, and its
first frame.

PostGIS is tuned as PostgreSQL's documentation recommends for reads, under the 16 GiB cap; every
setting is in `SETTINGS` and each record lists them with the server's values:

| Setting | Value | Default | Why |
|---|---|---|---|
| `shared_buffers` | 4 GB | 128 MB | A quarter of the memory, the documentation's starting point |
| `effective_cache_size` | 12 GB | 4 GB | `shared_buffers` and the page cache the cap leaves |
| `work_mem` | 64 MB | 4 MB | Each sort and hash in a query may use this much; the largest grid's hash table needs more than the default |
| `max_worker_processes`, `max_parallel_workers` | the machine's CPUs, which each record gives | 8 | Parallel query may use every CPU |
| `max_parallel_workers_per_gather` | half the CPUs | 2 | A frame sends two queries at once |
| `jit` | off | on | JIT pays for long CPU-bound queries and costs more than it saves on short ones, which a frame's are |
| `random_page_cost` | 1.1 | 4 | The value the documentation gives for solid-state storage |
| `parallel_setup_cost`, `parallel_tuple_cost` | 100, 0.01 | 1000, 0.1 | A grid's partial aggregates send many groups to the leader; at the defaults the planner aggregates millions of rows in one process rather than pay for them |
| `CREATE STATISTICS` on `(qx >> s), (qy >> s)` for each depth 0 to 16 | built by `VACUUM ANALYZE` | none | From the columns' statistics alone the planner overestimates the cells of a grid or tile grouping many times over, and does not then aggregate in parallel |

A dense tile's sample is its K matching rows of lowest `rank`. Three ways of asking for it were
measured, each exact, over every frame of the three workloads, warm, as the viewer (median ms by
zoom). Each record's `config` keeps the viewer's plan of the arm's query at zoom 0 and at
zoom 8:

| Query | Arm | Zoom 0 | 3 | 4 | 6 | 8 | 12 | 16 | All frames |
|---|---|---|---|---|---|---|---|---|---|
| `row_number() OVER (PARTITION BY` tile `ORDER BY rank)` over the camera's rows | `postgis+integer` | 1,316 | 661 | 168 | 42 | 4.9 | 0.7 | 0.6 | 7.6 s |
| | `postgis+leakproof` | 1,169 | 759 | 80 | 31 | 2.2 | 0.7 | 0.7 | 6.9 s |
| | **`postgis` (used)** | 996 | 636 | 77 | 44 | 26 | 25 | 26 | **7.9 s** |
| One `ORDER BY rank LIMIT K` subquery a tile, `UNION ALL`, with a B-tree on `rank` | **`postgis+integer` (used)** | 1.6 | 146 | 56 | 39 | 7.2 | 2.3 | 1.8 | **2.7 s** |
| | **`postgis+leakproof` (used)** | 1.2 | 108 | 77 | 21 | 3.2 | 2.0 | 1.9 | **1.7 s** |
| | `postgis` | 1.1 | 256 | 332 | 355 | 374 | 332 | 322 | 32.3 s |
| `CROSS JOIN LATERAL` the same subquery over a `VALUES` list of the tiles | `postgis+integer` | 1.3 | 267 | 80 | 36 | 6.1 | 1.1 | 0.6 | 2.6 s |
| | `postgis+leakproof` | 1.6 | 1,452 | 1,316 | 1,096 | 1,452 | 1,243 | 1,221 | 111 s |
| | `postgis` | 1.4 | 1,333 | 1,287 | 1,118 | 1,233 | 1,226 | 1,277 | 107 s |

The window sorts every matching row in the camera, 860,000 of them at zoom 0, spilling to disc.
With a subquery a tile, each with the tile's bounds as constants, the planner chooses for each
tile: it walks the `rank` index and stops after K matching rows where the tile holds a large
share of the table, and reads the tile from the spatial index and keeps the K lowest ranks with a
top-N sort where it holds few. A comparison of `rank` is leakproof, so the index serves the
viewer under the policy. The lateral form gets one plan for every tile, since its bounds are
parameters; in the GiST arms it cannot use `&&`, whose box must be a constant here, and walks
the `rank` index across the whole table. So `postgis+integer` and `postgis+leakproof` use a
subquery a tile and build the `rank` index, and `postgis`, whose viewer cannot use the spatial
index and scans the table for each subquery, uses the window over one scan and builds no `rank`
index.

On GBIF the walk is the slow plan. The planner estimates the rows a tile and the policy keep as if
every place held the same share of every label, while GBIF's records crowd onto land and a label
is a country: a tile over Canada or Britain, whose labels the viewer lacks, holds hundreds of
thousands of rows and almost none the viewer may see, and the walk reads most of the table before
it finds K of them, or the whole of it for a tile with none (7.4 s for one tile at 10⁷ rows, and
the 5 × 10⁷ tier's zoom-4 frames a median of 17.7 s). On `(qx, qy)` the estimate was worse still,
the product of the two columns' ranges. So in `postgis+integer` each tile is its range of `m`,
read from the B-tree and kept to the K lowest ranks by a top-N sort, which costs at most the
tile's rows whatever the policy keeps; it sorts by `rank + 0`, which the `rank` index does not
serve, so that the planner cannot choose the walk. The whole map under no filter is the one tile
that sorts by `rank` and walks the index, since there the policy keeps exactly the share the
planner assumes. The rows above for `postgis+integer` are PaperSeek's, with the earlier `(qx, qy)`
key. `TABLESAMPLE` was not used: it samples pages or rows of the whole table by a fraction,
before the policy and the filter, so it cannot give exactly K matching rows a tile.
The build's server also runs with what "Populating a Database" recommends for a bulk load:
`wal_level = minimal` and `max_wal_senders = 0`, so that the `COPY` and the table and index made in
the same transaction may skip the WAL; `maintenance_work_mem = 2GB`; and `max_wal_size = 16GB`
(`BULK`). The serving server runs without them. `/dev/shm` is 2 GB, since the image's
documentation warns that Docker's 64 MB may be too small for parallel query.

The viewer's restriction is row-level security: the superuser builds and owns the table, enables
and forces row-level security on it, and creates a policy `FOR SELECT TO viewer USING (licence IN
(...))`; the login role `viewer` owns nothing and is granted `SELECT` on the table alone. Forcing
applies the policy to the table's owner too; a superuser bypasses it regardless. PostgreSQL
evaluates a policy's condition before any condition of the query that is not marked leakproof,
and will not use such a condition to scan an index, so that a function cannot reveal, by its
result or an error, anything about rows the policy hides. PostGIS 3.6.4 marks none of its 776
functions leakproof, `&&` included, while PostgreSQL's own comparisons of integers and text are.
So GiST, PostGIS's recommended index, is unusable under row-level security, and an index on
integers is usable. `EXPLAIN` shows it: the viewer's query in the `postgis` arm scans the table, in
parallel where the planner chooses to, and the superuser's identical query uses the index. Each
record keeps the viewer's plan of one grid and whether `&&` is leakproof.

In `postgis+leakproof` the superuser runs `ALTER FUNCTION geometry_overlaps(geometry, geometry)
LEAKPROOF`. That is the superuser's claim, which PostGIS's maintainers have not made, that the
function reveals nothing about its arguments beyond its result. An operator who has read it, a
comparison of two bounding boxes, might make the claim so that row-level security and the spatial
index can work together. One way it could fail is an error that depends on a hidden row, such as
one for mixed SRIDs; in PostGIS 3.6.4 `&&` raises none, which this harness checked for mixed SRIDs,
empty points and mixed dimensions. The column is declared `geometry(Point, 0)` and every row is
written by the build with SRID 0, though a typmod of SRID 0 means an unspecified SRID and does not
by itself refuse others.

## OpenSearch

OpenSearch 3.9.0 runs as one node from the official image, pinned by digest, with its security
plugin on, since document-level security is what restricts the viewer. Elasticsearch is not used
because its document-level security is a paid feature. The plugin runs with the image's demo
configuration: its demo TLS certificates, which the adapter does not verify, and the `admin` user
with a password made for the run. That is fit for a local run and for nothing else.

The build (`adapters/opensearch.py`) creates an ingest pipeline that quantises `x` and `y` in
Painless by `frame.fixed32`'s rule, in the same order of operations as ClickHouse's SQL, and
stores `m16`, the Morton code of the top 16 bits of `qx` and `qy` with x on the even bits; and an
index of one shard and no replicas, sorted by `m16`, with `qx`, `qy` and `m16` as `long`,
`licence` and `type` as `keyword`, `publication_year` as `integer` and `is_oa` as `boolean`, and
`rank` as `long`, and `_source` disabled, so positions are read from doc values and nothing else
is stored. Six client
processes read the tier's Parquet on the host and send it through the pipeline in bulk requests of
50,000 documents (about 7 MB), with the refresh interval at `-1`; the index is then refreshed,
force-merged to at most six segments, returned to the default refresh interval (one second) and
flushed, and every file in its data directory is synced. Build time runs from creating the
pipeline to the end of that sync, on a running server; size on disk is the whole data directory,
which also holds the security plugin's own index. The bulk clients' CPU time is in each record
(`bulk_clients_cpu_s`): about 10 CPU seconds at 10⁶, over a load of about 10 seconds in a build of
about 13, so roughly one more CPU busy throughout the load than the build's cgroup shows.

The container runs as the host's uid, as every container here does, and the image's `config/`,
which the security plugin's demo installer writes to at each start, belongs to uid 1000, which is
this machine's user.

Access: after the build the admin creates the internal user `viewer`, a role `viewer_read` with
`read` on the index and the document-level security query `{"terms": {"licence": [the viewer's
terms]}}`, and a mapping of the role to the user. Every frame and table is a search sent with the
viewer's credentials over basic authentication, and no search names a licence. Each record's
`config.access_checks` holds what the running server did: an aggregation over the whole index
counts every row as the admin and, as the viewer, the admin's count of the viewer's terms and the
oracle's count of the viewer's rows, and a run stops if either differs. The same aggregation sent
twice as the admin hits the shard request cache on the second (with default caches); sent twice
as the viewer it neither hits nor misses, because the security plugin turns the request cache off
for a search under document-level security, so the admin's cached result cannot reach the viewer.

A frame is two searches sent at once, each filtered to the camera's tiles by two ranges on `qx`
and `qy` and to the filter by its terms and ranges, all in filter context. The grid's is a
`histogram` on `m16` at interval 4^(16 − depth) with `min_doc_count` 1, whose bucket keys are the
cells' Morton codes; the tiles' is the same histogram at the tile depth, each tile with a
`top_hits` of K positions from doc values. Under the `points` frame the `top_hits` is sorted by
`rank`, so each tile sends its K matching documents of lowest rank, every document of a sparse
tile; under `counts` it is sorted by `_doc`, beside a `bucket_selector` that keeps only the tiles of
at most K, so no dense tile's positions are sent. A `sampler` or `diversified_sampler` aggregation
was not used, since each takes the best-scoring documents a shard, not K a tile, and a
`random_score` query would score every document in the camera for each search. Tile counts are the
grid's sums, as for ClickHouse. `m16` holds cells to depth 16, as deep as the frame's grid and the cameras' zoom go.

Every aggregation here is exact on any number of shards and segments. A histogram's buckets are
merged whole: a value falls in bucket `floor(value / interval)`, exact in double precision for a
power of two and a value below 2^53, and no shard drops a bucket. Each shard's `top_hits` sends its
K lowest ranks per tile and the merge keeps the K lowest of those, which are the tile's; under
`counts` the `bucket_selector` runs on the merged count, so a sparse tile gets every position. The `type` aggregation asks for 21 buckets, more than the 20 types and
`none`, so no shard cuts one, and it reports no error bound.

The aggregation workload is one search with a `terms` aggregation on `type` (`none` for a missing
one), a year `histogram` at interval 1 inside a `filter` on the binned range, a `filter` for the
years outside it and a `missing` aggregation for no year. Sent as two searches at once, `type`
in one and the years in the other, the tables were slower (p50 4.9 ms against 4.5 ms with one
shard and one segment; 4.9 against 4.3 in the chosen layout), so they stay one search.

Ways of asking for the frame, each exact, over every frame of the three workloads three times,
warm, with compute caches, one shard and one segment, each timed with its JSON decoding (so the
figures run above the harness's):

| Frame | Per pass | p50 / p99 |
|---|---|---|
| Nested histograms on `qx` and `qy`, grid and tiles in one search | 22.6 s | 27 / 1,980 ms |
| The same, as two searches at once | 19.1 s | 23 / 1,782 ms |
| A `composite` grid of two histogram sources, paged, beside the nested tiles | 14.3 s | 26 / 1,480 ms |
| `m16` histograms, grid and tiles in one search | 7.6 s | 21 / 566 ms |
| **`m16` histograms, as two searches at once (used)** | **6.3 s** | **22 / 422 ms** |

`m16` costs about a second of build time and 21 MiB on disk at 10⁶. With `http.compression:
true`, which 3.9 accepts as a node setting and which then gzips each response the client asks to
have compressed, the chosen frame took 7.2 s a pass (28 / 459 ms), so compression is left at its
default, off.

Layouts of the index, each built from nothing and exact, with the chosen frame, over the same
frames timed as the harness times them (decoding outside):

| Layout | Build | Size | Frames per pass | Frame p50 / p99 |
|---|---|---|---|---|
| 1 shard, 1 segment | 14.0 s | 91 MiB | 5.21 s | 22.3 / 269 ms |
| 4 shards, 1 segment each | 10.9 s | 91 MiB | 3.90 s | 11.2 / 187 ms |
| 12 shards (the container's CPUs), 1 segment each | 12.9 s | 100 MiB | 5.84 s | 11.1 / 286 ms |
| 4 shards, 3 segments each, concurrent search `all` | 12.2 s | 95 MiB | 4.13 s | 9.8 / 315 ms |
| 1 shard, 6 segments, concurrent search `auto` (the default) | 10.9 s | 91 MiB | 3.22 s | 9.3 / 219 ms |
| 1 shard, 6 segments, concurrent search `all` | 12.9 s | 92 MiB | 3.29 s | 9.7 / 224 ms |
| 1 shard, 1 segment, sorted by `m16` | 17.9 s | 85 MiB | 3.41 s | 14.5 / 159 ms |
| **1 shard, 6 segments, sorted by `m16` (used)** | **14.9 s** | **90 MiB** | **2.76 s** | **8.9 / 174 ms** |

A force-merge to twelve segments left six, the segments the load wrote, so six is the most there
are to search at once at 10⁶. Concurrent segment search's default mode, `auto`, already searches
a request with aggregations across segments on several threads, and `all` was no faster, so the
mode stays at its default. The sorted layout with six segments was measured with `all`.

Tuning, every setting the adapter writes against its default, from OpenSearch's documentation on
indexing speed and search performance:

| Setting | Value | Default | Why |
|---|---|---|---|
| Heap (`-Xms`, `-Xmx`) | 8 GiB | 1 GiB (the image's `jvm.options`) | Half the container's memory, up to 31 GB |
| `discovery.type` | `single-node` | multi-node discovery | One node |
| `index.number_of_shards` | 1 | 1 | Stated: the fastest of the layouts measured |
| `index.number_of_replicas` | 0 | 1 | One node, and no replicas during a bulk load |
| `index.refresh_interval` | `-1` during the load, then the default | 1 s | No refreshes during a bulk load |
| `index.sort.field` | `m16`, ascending | none | Each tile's documents adjacent, as ClickHouse and PostGIS store theirs |
| `_source` | disabled | enabled | Positions come from doc values; nothing reads the source |
| Force-merge | to at most six segments | none | A read-only index merged, keeping segments for concurrent segment search |
| `index.max_inner_result_window` | 500 | 100 | A `top_hits` of K |
| `rank` | `long` with doc values | – | A random value per document for the dense tiles' samples, as ClickHouse and PostGIS store it |
| `search.max_buckets` | 2,097,152 | 65,535 | The largest grid: 20 tiles of 256 × 256 cells |
| Bulk requests | 50,000 documents from 6 processes | – | The documentation's 5 to 15 MB a request, from several clients |
| Performance analyzer agent | off | on | A monitoring process beside the server, kept out of the timings as ClickHouse's log tables are |
| `index.requests.cache.enable`, `index.queries.cache.enabled` | off with compute caches | on | See the cache table |

Every other setting is the image's default, and each record lists the index's settings, its
layout, the node's cache settings, the concurrent segment search mode, `http.compression`, the
role and the access checks. The security plugin caches an authenticated user for an hour, so the
viewer's first search after a start, which session start times, pays for its password check and
the resolution of its role, and later searches do not.

## Files

| File | What it does |
|---|---|
| `requirements.txt`, `requirements.lock` | The direct pins, and every package with its hashes |
| `tiers-manifest.json` | The extent, and each tier's row count, rows per licence and SHA-256, which `prepare.py` must reproduce and `run.py` checks |
| `viewer.json` | The one viewer: its principal name and the licence terms it is granted |
| `prepare.py` | Derives the tiers deterministically and writes `$DATA/bench-compare/tiers/manifest.json` with every checksum, seed and count |
| `frame.py` | The frame's definition: extent, screen, tiles, grid depth, K, the quantisation rule, the filter grammar and the aggregation's bins |
| `cameras.py` | The three workloads (battery, camera script, filter changes) from a tier's density as the viewer sees it |
| `oracle.py` | The exact frame and aggregation tables by DuckDB over the tier file for the viewer, and their comparison with a system's answer |
| `adapters/` | One adapter per system behind `adapters/base.py` |
| `run.py` | One run of one system, appending one record to a JSONL file |
| `tables.py` | Records to one Markdown table per tier, one column per system |
| `test_spread.py` | The dense-tile spread check on synthetic tiles, run with `.venv/bin/python test_spread.py` |

## The data

The source is the PaperSeek rung of the Mosaica test ladder
(`data/ladder/paperseek/points.parquet`, 52,225,761,595 bytes, 102,117,343 works), whose
positions are a UMAP layout of Stella V5 embeddings of each work's title and abstract and whose
attributes come from OpenAlex. The benchmark reads `x`, `y`, `licence`, `publication_year`,
`type` and `is_oa`. A work OpenAlex did not match has no year, type or `is_oa`, and its licence is
`unlicensed`, as is that of every work without one, so every row has a licence.

A tier of nominal size N keeps the row whose `entity_id` gives `splitmix64(entity_id ^
0x6D6F736169636131) < floor(N · 2⁶⁴ / 102,117,343)`, and `full` keeps every row, so the tiers
nest. The extent is the square x ∈ [−18, 21], y ∈ [−14, 25], fixed from the full file's bounds by
the rule in `prepare.py`'s docstring and used by every system. Before anything drawn from this data
is published, the licences of PaperSeek and OpenAlex (OpenAlex is CC0) must be stated with it.

## The frame

A screen of 1920 × 1080 pixels at an integer zoom, with 512-pixel tiles: the depth-`zoom` tiles it
touches, each tile's matching count, a count grid at depth `min(zoom + 8, 16)` (256 × 256 cells a
tile to zoom 8), every matching position in each tile with at most K = 500 of them, and from each
tile with more exactly K of its matching items, a random sample. Positions are compared on the 2³²
grid that Mosaica's build quantises to (`frame.fixed32`), so an exact frame has zero error. Every
count is over the viewer's visible items.

Mosaica serves the K matching items of each tile with the lowest `mosaica_id`
(`crates/mosaica-engine/src/select.rs`, whose module comment defines it), since `k_min = K`
makes each tile's floor `K`. A `mosaica_id` is a keyed Feistel
permutation of the item's internal number, under a key each build draws at random, so the sample
is uniform and differs from build to build. Every other system takes the K with the lowest `rank`,
a column `prepare.py` writes as `splitmix64(entity_id ^ RANK_SEED) >> 1`, which is the same
method over a stored random value. `rank` is computed by
`prepare.py`, outside every build, where Mosaica assigns its `mosaica_id`s inside its timed
build; the column costs 7.5 ms at 10⁶ rows (7.5 ns a row), so leaving it out of the other builds'
times changes them by less than their noise. The samples are therefore different items, and the
oracle cannot check one item by item. It checks what holds for any correct sample: exactly K positions
from each dense tile, each one of the viewer's matching items there, none sent twice (all three
point errors), and the sample's spread over the sub-cells four levels below the tile
(`SUBCELL_OFFSET` in `frame.py`). Two figures measure the spread against a uniform random sample
of as many of the tile's matching items, drawn without replacement: the sub-cells the sample
covers against the number such a sample covers on average, and Pearson's chi-square of the
sample's count in runs of sub-cells, taken in Morton order and each closed once a uniform sample
expects five points in it, corrected for drawing without replacement. Each is given as a normal
deviate, for the workload and for each zoom of it, and the table marks a column where any is past
5 either way. A workload judges a tile once under each filter, however often its pans and steps
back out return to it. The chi-square counts runs rather than single sub-cells because a GBIF tile
holding millions of records has sub-cells at sea holding a few, which a sample of 500 rarely
reaches and which add hundreds to the statistic when it does: counted cell by cell, most uniform
samples scored far below the statistic's mean and a few far above it. `test_spread.py` checks the
figures on synthetic tiles of that kind: uniform samples give deviates of unit spread, a fifth of
each sample drawn from one corner gives a chi-square deviate of +92, the first K in Morton order
+743, and three skewed tiles among 300 are marked at their zoom while the workload's sum passes.
A timed frame must also send the same sample as the untimed one, since the oracle cannot tell two
valid samples apart.

The `counts` frame (`--dense counts`) is the earlier definition, in which a dense tile need send
nothing and may send up to K, each counted and checked as above; ClickHouse, PostGIS and
OpenSearch send none. Mosaica has no request that sends a sparse tile's points and none of a dense
one's, so under `counts` it sends K from each dense tile, as it did.

## Departures from the design

- **No text search, no abstracts, no topic layer.** PaperSeek carries titles, abstracts and a
  topic tree, and the design's filters include text. Stage 1 reads none of them, so neither the
  build nor the filters exercise text indexing or annotation layers.
- **The grid stops at depth 16.** Mosaica's viewport grid is 2¹⁶ cells a side and its sub-cell
  counts cannot go deeper, so the design's 256 × 256 cells a tile holds only to zoom 8. Every
  system is asked for the same coarser grid beyond that (16 × 16 a tile at zoom 12, one cell at
  zoom 16). Mosaica's `/v1/aggregate` can count to depth 32, but as a second request.
- **Mosaica's viewport grid ignores the filter.** The sub-cell counts (`underlay_offset`) are
  taken over the viewer's visible set, not the filtered set, while the tiles' `matched` counts do
  follow the filter. So for a filtered frame whose grid is deeper than its tiles, the adapter
  sends the viewport request without the underlay and a `/v1/aggregate` request for the filtered
  cells at the same time, and the latency covers both. That request's filter is the frame's with
  the screen's tiles as a `region` beside it, as a map client sends it, so the server can narrow
  the filter to the screen. Every other frame is the viewport request
  alone: an unfiltered one, whose underlay is the grid, and a filtered one at zoom 16, whose grid
  is its tiles.
- **Filters where the data permits.** A type filter names one type or an `in` set of up to three,
  whichever comes nearest its target. `article` is 79% of the viewer's items and every other type
  together about 18%, so no set comes near 50%, and that filter is `article`. `is_oa` has two
  values. The zoom-9 filter screen holds about a thousand of the viewer's items at 10⁶, so its
  0.1% filters match one item or none. Each step's actual share
  is in the cameras file. A box stands in for a polygon.
- **One viewer.** The design's concurrent viewers, with distinct grants, are not built.
- **Mosaica is configured to produce the frame.** Every `[serve]` setting the adapter writes
  (`adapters/mosaica.py`, `SERVE`), against its default:

  | Setting | Value | Default | Why |
  |---|---|---|---|
  | `k_min` | 500 | 2 | Every tile serves `min(cap, max(k_min, C_θ))`, which is `min(matched, K)` at `k_min = K`, so a sparse tile serves every point; by default it may serve fewer, by design. `theta_target_marks` stays at its default, 16: raised to 2⁴⁰ it gave the same points but saturated the density threshold, and an unfiltered dense tile was then read by a scan rather than from the identity bands, 214 ms against 1.4 ms at zoom 0 over the full tier. A filtered request scans either way |
  | `max_underlay_offset` | 8 | 4 | The grid is 8 levels below the tiles |
  | `max_underlay_cells` | 1,310,720 | 8,192 | 20 tiles at an offset of 8 |
  | `max_aggregate_cells` | 1,310,720 | 1,048,576 | The same cells through `/v1/aggregate` |
  | `aggregate_response_bytes` | 256 MiB | 16 MiB | One aggregate response holds a whole grid |
  | `k_max_marks` | 500 | 500 | Stated so K and the cap cannot drift apart |
  | `max_k` | 1,000 | 1,000 | Stated for the same reason |

  The build runs with `--no-oracle-pairs`, which leaves out `pairs.parquet`, a file only the
  conformance suite and `mosaica verify --deep` read; the server does not.
- **Dense tiles are sampled by different items.** Each system draws its own random sample of a
  dense tile, so the oracle checks the sample's size, its validity and its spread rather than its
  items (see "The frame"). Under `counts` Mosaica alone sends dense tiles' positions.
- **Every build loads `rank`.** ClickHouse, PostGIS and OpenSearch load the `rank` column under
  both frames, so their builds and sizes include it; Mosaica's build does not read it.
- **Cold means this system's files only.** The machine's page cache is shared with other work, so
  a cold start drops the system's own files' pages with `posix_fadvise` and deletes its caches,
  rather than dropping every cache on the machine.
- **Memory** is reported twice: the server process's peak RSS, and its cgroup's `memory.peak`,
  which includes the page cache it filled. Mosaica maps its bundle, so its RSS counts pages
  ClickHouse's would not; the cgroup figure is the comparable one. Each serving cgroup's figure
  leaves out the pages of the data that the build wrote and that are still cached, because the
  kernel charged those to the build's container, which has since been removed. The table
  therefore also gives the serving cgroup's figure after the last cold restart, which dropped
  those pages, and that restart's one frame.
- **Server start and session start are two figures**, divided at the same point for both systems.
  Server start runs from starting the container until the data is loaded and the server answers a
  request that is not the viewer's: Mosaica's `/readyz`, and for ClickHouse a query against the
  table as its default user, which reads one granule. Session start runs from opening the viewer's session to the end of
  its first whole-map frame, so it holds every cost of the viewer's first use: for Mosaica the
  login and the first read, which builds the viewer's visible set; for ClickHouse, whose HTTP
  interface sends the viewer's user and password with every query, the first frame, which
  authenticates the viewer and resolves its rights and row policy.
- **OpenSearch loads through a client.** OpenSearch has no reader for Parquet, so the bulk clients
  run on the host, outside the container: their CPU and memory are in neither the build's cgroup
  peak nor its RSS, while ClickHouse reads the file inside its server. They run at the build's
  niceness. The quantisation is still the server's, in its ingest pipeline.
- **OpenSearch's host settings.** Its documentation asks for `vm.max_map_count` of at least
  262,144, and this machine has 65,530; single-node discovery turns that bootstrap check into a
  warning, and one merged segment maps few files. `bootstrap.memory_lock` is not set, since the
  container has no swap.
- **OpenSearch speaks TLS.** Its responses are encrypted with the demo certificates and are not
  compressed (`http.compression` is off by default in this release, and turning it on was
  slower; see "OpenSearch"), where ClickHouse's and Mosaica's are plain HTTP. A `bbox` filter's
  corners are quantised by the adapter with `frame.fixed32`, since a range query has no
  arithmetic.
- **Timed responses are checked too.** Every timed frame is checked as the untimed pass checks
  it, tile counts, grid and every point check including the dense tiles', and every timed table
  likewise. Where the untimed answer to a step was wrong in a part, which its errors already
  count, the timed answer is compared with the untimed one in that part instead, so the figure
  there measures repeatability.
- **PostGIS without Martin.** The method names PostGIS with Martin, which serves vector tiles from
  it. The frame is a count grid and positions, not a vector tile, so the PostGIS adapter queries
  PostgreSQL directly and Martin is not run.

## GBIF

The second data set is GBIF's occurrence records, from 5 × 10⁷ to 10⁹ points, for Mosaica and
ClickHouse, and under single-label access also for PostGIS's `postgis+integer` arm and OpenSearch.
`BENCH_DATASET=gbif` selects it in every script (`dataset.py`), so the tier, the viewer
(`viewer-gbif.json`), the committed manifest (`tiers-manifest-gbif.json`) and each adapter's schema
are GBIF's; run without it, every script works on PaperSeek as above.

```bash
# The tiers, from the gbif-terms rung's points file (74 GB, 3,495,729,729 placed occurrences),
# about 15 minutes for one pass. The tiers nest, so 1e6 and 5e8 come from one pass and 1e9 from
# another, written when its runs are about to start
cd bench/compare && nice -n 19 .venv/bin/python prepare_gbif.py --tiers 1e6,5e8 && cd -

# The cameras; the first use of a tier also writes the oracle's copy of it (below)
cd bench/compare && BENCH_DATASET=gbif nice -n 19 .venv/bin/python cameras.py \
    --tier $DATA/bench-compare/tiers/gbif-5e8.parquet --out cameras/gbif-5e8.json && cd -

# One run per system
cd bench/compare && for s in mosaica clickhouse postgis+integer opensearch; do
  BENCH_DATASET=gbif BENCH_MEMORY=24g .venv/bin/python run.py --system $s --nice 19 \
      --tier $DATA/bench-compare/tiers/gbif-5e8.parquet --cameras cameras/gbif-5e8.json
done
```

`prepare_gbif.py`'s docstring defines the tiers. Each keeps a row by the same splitmix64 threshold
as PaperSeek's, on GBIF's `gbifid`, which is also its `entity_id`, and stores `rank` by the same
rule with its own seed, `0x6D6F736169636132`. That is the seed the GBIF tiers were derived with,
and it is pinned in `prepare_gbif.py` so the committed checksums hold without writing the tiers
again. It differs from `SAMPLE_SEED` in two low bits, and splitmix64 mixes those inputs into
unrelated outputs, so the rank is independent of the sampling hash; the dense-tile coverage
figures measure the sample's spread either way. Positions are projected to Web Mercator in metres once, by `prepare_gbif.py`, with the
latitude clamped at ±85.0511° as the GBIF rung's view clamps it, so the 66,037 polar records of
the full file lie on the extent's edges. Every system and the oracle read the projected `x` and
`y`: Mosaica's view declares `projection = "none"` over the world square, `±π · 6,378,137` on
both axes, as for PaperSeek, and ClickHouse quantises the same doubles by the same rule.

A tier keeps `kingdom`, the category, which has nine values; `year`, an unsigned 16-bit year
with no value for about 2.5% of the rows; and `access`, the rung's list of up to three access
terms a row: its country (`UNRECORDED` where GBIF has none), `y:` and its year, and `s:` and its
species key. GBIF's species, names and country columns are not kept, and no system builds a
taxonomy layer. The aggregation tables are the counts by kingdom and the year histogram over
1900 to 2029.

A row is visible to a viewer who holds any one of its access terms, which is how Mosaica reads a
list-valued `point_visibility`. ClickHouse's row policy is `hasAny(access, [the viewer's
terms])` and the oracle's predicate `list_has_any(access, [the viewer's terms])`, which is the
same rule. No row's list is empty, which `prepare_gbif.py` checks, so the view's default term
never applies. The viewer holds every country term but `GB` and `CA`, and also `y:2024` and
`y:2025`, so it sees those two countries' records of 2024 and 2025 through the year term:
890,985 of the 1,000,313 rows of the 10⁶ tier (89.1%). ClickHouse stores `access` as
`Array(String)`, since its documentation recommends `LowCardinality` under 10,000 distinct
values and the column holds about 1.4 million.

Each tier also holds `label`, each row's one country term from `access`, so that the main GBIF curve
measures a single access label an item, the common case, as PaperSeek's licence is.
`BENCH_GBIF_ACCESS` chooses which column decides visibility: `label`, the default, or `list` for the
list of country, year and species terms above, the heavier case. Under `label` the viewer is
`viewer-gbif-label.json`, every country but `GB` and `CA` with no year term; Mosaica's view declares
`point_visibility` on `label`, ClickHouse stores it as `LowCardinality(String)` (253 terms) with the
row policy `label IN (the viewer's terms)`, and the oracle restricts by `label IN (...)`. PostGIS
stores `label` as `text`, with the row-level security policy `label IN (the viewer's terms)`, and
OpenSearch as a `keyword` field, with the document-level security query `terms` on `label`; both
store `kingdom` as they store PaperSeek's `type` and `year` as they store `publication_year`, and
neither has a schema for the list, which `run.py` refuses for them. Each system loads only the
access column the run uses. `prepare_gbif.py --add-label` added the column to the 1e6 and 5e8 tiers
written before it, so the list-access records at 5e8 name the tier's earlier checksum. The full
tier, all 3,495,729,729 rows, is written with `--no-access`, without the list, so that it fits this
machine's disc; it serves label access alone, the oracle refuses it under `list`, and its checksum
differs from that of a tier with the same rows and `access`.

The full tier is a performance run, with no oracle (`run.py --no-oracle`): the oracle's copy of
its 3.1 × 10⁹ visible rows spilled 169 GB of sort to disc before it was stopped, short of this
machine's room. Such a record says it was not checked, and `tables.py` shows its error rows as
"not checked"; it keeps every timing, and the points and bytes each workload received, the
points from dense tiles counted by the system's own tile counts. Every smaller tier is checked
against the oracle. The full tier reuses the 10⁹ tier's cameras (`cameras/gbif-1e9-label.json`):
both are Web Mercator's world square, so the screens apply, and each filter's share is the one
measured at 10⁹.

The oracle answers from its own copy of each tier (`Oracle(copy=True)`): a DuckDB file beside the
tier holding the viewer's visible rows' positions, kingdom and year, ordered by the Morton code
of each position's top 16 bits, so that a query over a small screen reads few of DuckDB's row
groups. The copy is made with the same visibility predicate, once for each tier and viewer, because
otherwise every one of a run's thousand or so oracle queries would scan the whole Parquet tier,
which at 10⁹ rows takes many seconds each. A
dense tile's validity check is also kept per step and per set of positions, since the timed
passes send the same sample again.

The filters follow PaperSeek's rules where the data permits. `Animalia` is about 79% of the
viewer's records, and no set of kingdoms comes nearer 50%, so the 50% kingdom filter is
`Animalia`; GBIF has no boolean column, so there are no flag filters, and the combined filter is
`Animalia` from the 50% year onwards.
