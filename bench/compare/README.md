# Comparative benchmark

Mosaica against other systems that draw a map over many points, each asked for the same frames and
aggregations as the same viewer, with every answer checked against an exact oracle.
[METHOD.md](METHOD.md) defines the frame, the viewer, the workloads, the metrics and the fairness
rules. This file is operational, and lists where stage 1 departs from the method.

Stage 1 covers the shared harness and Mosaica, ClickHouse and PostGIS, on the PaperSeek data
at 10⁶ points for development only. Mosaica runs as two arms: `mosaica+aggregate`, a viewport
request with the count grid from `/v1/aggregate` beside it, which is how Mosaica's own map asks
and the arm that represents it; and `mosaica`, one viewport request alone, whose sub-cell grid
counts the visible set and ignores the filter (see "Departures"). Runs write to `results/`, which
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

# 6. One run per system; each appends a record to results/dev.jsonl
cd bench/compare
for s in mosaica mosaica+aggregate clickhouse postgis; do
  for c in compute defaults; do
    .venv/bin/python run.py --system $s --caches $c \
        --tier $DATA/bench-compare/tiers/paperseek-1e6.parquet \
        --cameras cameras/paperseek-1e6.json --work $DATA/bench-compare/work
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
Work directories go under `$DATA/bench-compare/work/`; delete them when done.

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

Each Mosaica record holds the admission limits and cache bounds the server reports in
`/control/status`; it does not report `compute_threads`, which defaults to the CPUs the container
may use. Mosaica also writes each grant's visibility masks to its cache directory, which no setting
bounds; they are grant state, kept in both modes, and deleted by a cold start.

ClickHouse runs at its defaults apart from the sort key, `ORDER BY m`, the cache settings above
and its system log tables, which are off. It leaves `index_granularity` (8192),
`max_insert_threads` and `max_threads` (both `auto`, 12 on this machine) at their defaults, and
each record lists them. Its filter predicates (`adapters/clickhouse.py`, `filter_sql`) have the
same SQL shapes as the oracle's; the oracle is DuckDB over the Parquet file, so the two share no
code and no engine, but a mistake in a predicate's shape would be made in both.

PostGIS is PostgreSQL 18.6 with PostGIS 3.6.4 (`adapters/postgis.py`). Its build streams the tier
from pyarrow's CSV writer into `COPY points FROM STDIN`, in the transaction that creates the table,
since `COPY` is the bulk load PostgreSQL documents and the Parquet file is on the client's side.
PostgreSQL quantises each position as it stores the row, in generated columns, by the frame's rule
with every operand a `float8`. Each position is stored twice: as `qx` and `qy`, `bigint` since
PostgreSQL has no unsigned 32-bit integer, for the grid's `GROUP BY`; and as `geom`, a point in
quantised space, which carries the GiST index PostGIS's documentation recommends. The build then
clusters the table on that index, runs `VACUUM ANALYZE`, a `CHECKPOINT` and an `fsync` of the data
directory, and its time runs from the `CREATE TABLE` to the end of the `fsync`; `initdb`, which
the image runs at the container's first start, is the server's setup and falls outside it, as
ClickHouse's start does. Size on disk is the whole data directory by `du`: at 10⁶, 110 MiB of
table and 40 MiB of index, beside about 240 MiB of WAL segments the server keeps for reuse and the
template databases; each record gives the table's, the index's and the WAL's sizes. The loader
runs in the harness's process, in a thread at the build's niceness, so the build's cgroup peak is
the server's alone. Every query selects the camera's tiles
with `geom && ST_MakeEnvelope(...)` and the same bounds on `qx` and `qy`, because PostGIS compares
boxes in single precision, rounded outwards, and `&&` alone admits points just outside a tile.
Server start ends when the superuser's `SELECT 1 FROM points LIMIT 1` answers; session start holds
the viewer's two connections, each authenticated by SCRAM, and its first frame.

PostGIS is tuned as PostgreSQL's documentation recommends for reads, under the 16 GiB cap; every
setting is in `SETTINGS` and each record lists them with the server's values:

| Setting | Value | Default | Why |
|---|---|---|---|
| `shared_buffers` | 4 GB | 128 MB | A quarter of the memory, the documentation's starting point |
| `effective_cache_size` | 12 GB | 4 GB | `shared_buffers` and the page cache the cap leaves |
| `work_mem` | 64 MB | 4 MB | Each sort and hash in a query may use this much; at 10⁶ the largest grid's hash table took about 20 MB, which the default would spill to disc |
| `max_worker_processes`, `max_parallel_workers` | 12, the machine's CPUs | 8 | Parallel query may use every CPU |
| `max_parallel_workers_per_gather` | 6 | 2 | Half the CPUs, since a frame sends two queries at once |
| `jit` | off | on | JIT pays for long CPU-bound queries and costs more than it saves on short ones, which a frame's are |
| `random_page_cost` | 1.1 | 4 | The value the documentation gives for solid-state storage |

The build's server also runs with what "Populating a Database" recommends for a bulk load:
`wal_level = minimal` and `max_wal_senders = 0`, so that the `COPY` into a table created in its
transaction, the index build and `CLUSTER` may skip the WAL; `maintenance_work_mem = 2GB`; and
`max_wal_size = 16GB` (`BULK`). The serving server runs without them. `/dev/shm` is 2 GB, since
the image's documentation warns that Docker's 64 MB may be too small for parallel query.

The viewer's restriction is row-level security: the superuser builds and owns the table, enables
and forces row-level security on it, and creates a policy `FOR SELECT TO viewer USING (licence IN
(...))`; the login role `viewer` owns nothing and is granted `SELECT` on the table alone. Forcing
applies the policy to the table's owner too; a superuser bypasses it regardless. **Under the policy
the GiST index is never used.** PostgreSQL evaluates a policy's condition before any condition of
the query that is not marked leakproof, and will not use such a condition to scan an index.
PostGIS 3.6.4 marks none of its 776 functions leakproof, `&&` included, while PostgreSQL's own
comparisons of integers and text are. Every viewer query therefore scans the whole table, in
parallel where the planner chooses to, with the leakproof bounds on `qx` and `qy` evaluated first; `EXPLAIN` shows it, and each record keeps the
plan of the whole map's grid and whether `&&` is leakproof. The superuser's identical query uses
the index. Marking the operator's function leakproof (`ALTER FUNCTION geometry_overlaps(geometry,
geometry) LEAKPROOF`, a superuser's assertion that it reveals nothing about its arguments) lets the
viewer's queries use the index too, with the same answers: at 10⁶ the median warm frame at zooms
12 and 16 fell from about 57 ms to 0.8 ms in a trial. The runs do not do it, since PostGIS's
maintainers have not made that assertion.

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
tile to zoom 8), and every matching position in each tile with at most K = 500 of them. Positions
are compared on the 2³² grid that Mosaica's build quantises to (`frame.fixed32`), so an exact frame
has zero error. Every count is over the viewer's visible items.

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
  follow the filter. So the `mosaica` arm's filtered frames have the wrong grid, and the record
  counts the error. The `mosaica+aggregate` arm sends a `/v1/aggregate` request for the filtered
  cells at the same time as the viewport request, at the cost of a second request.
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
  | `theta_target_marks` | 2⁴⁰ | 16 | Every tile serves `min(matched, K)` points; by default a sparse tile may serve fewer, by design |
  | `max_underlay_offset` | 8 | 4 | The grid is 8 levels below the tiles |
  | `max_underlay_cells` | 1,310,720 | 8,192 | 20 tiles at an offset of 8 |
  | `max_aggregate_cells` | 1,310,720 | 1,048,576 | The same cells through `/v1/aggregate` |
  | `aggregate_response_bytes` | 256 MiB | 16 MiB | One aggregate response holds a whole grid |
  | `k_max_marks` | 500 | 500 | Stated so K and the cap cannot drift apart |
  | `max_k` | 1,000 | 1,000 | Stated for the same reason |

  The build runs with `--no-oracle-pairs`, which leaves out `pairs.parquet`, a file only the
  conformance suite and `mosaica verify --deep` read; the server does not.
- **Dense tiles.** The frame asks for no positions from a tile with more than K. ClickHouse sends
  none; Mosaica sends K anyway, so its responses are larger. Both counts are in the record, and
  every position sent from a dense tile must be one of the viewer's matching items there, counted
  as a multiset; one that is not counts as a point error.
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
- **Timed responses are checked too.** Every timed frame is checked as the untimed pass checks
  it, tile counts, grid and every point check including the dense tiles', and every timed table
  likewise. Where the untimed answer to a step was wrong in a part, which its errors already
  count, the timed answer is compared with the untimed one in that part instead, so the figure
  there measures repeatability.
- **PostGIS without Martin.** The method names PostGIS with Martin, which serves vector tiles from
  it. The frame is a count grid and positions, not a vector tile, so the PostGIS adapter queries
  PostgreSQL directly and Martin is not run.
