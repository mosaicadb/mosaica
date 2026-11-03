# Comparative benchmark

Mosaica against other systems that draw a map over many points, each asked for the same frames
and each frame checked against an exact oracle. [METHOD.md](METHOD.md) defines the frame, the
workloads, the metrics and the fairness rules; this file is operational, and lists where stage 1
departs from the method.

Stage 1 covers the shared harness and two systems, Mosaica and ClickHouse, run at 10⁷ points for
development only. Mosaica runs as two arms: `mosaica+aggregate`, a viewport request with the count grid
from `/v1/aggregate` beside it, which is how Mosaica's own map asks and the arm that represents
it; and `mosaica`, one viewport request alone, whose sub-cell grid counts the visible set and
ignores the filter (see "Departures"). Runs write to `results/`, which is not tracked. **No
figure from this directory is fit to publish yet**: every record carries
`"development_only": true`, and the machine it ran on was shared with other measurement jobs.

## Running it from scratch

Everything below runs from the repository root. Build steps run under `nice -n 19`; do not start
one while the one-minute load average is above 3.

```bash
# 1. Python 3.10 (tested on 3.10.12), every package pinned with its hashes, in a venv inside this
#    directory that git ignores
python3 -m venv bench/compare/.venv
bench/compare/.venv/bin/pip install --require-hashes -r bench/compare/requirements.lock

# 2. The data: downloads 5.5 GB (HoloViz asks that it be downloaded sparingly), unpacks 7.7 GB,
#    and writes the tiers under data/bench-compare/tiers/. About 10 minutes.
cd bench/compare && nice -n 19 .venv/bin/python prepare.py --tiers 1e7,1e8 && cd -

# 3. The cameras for a tier (checked in for osm-1e7)
cd bench/compare && .venv/bin/python cameras.py \
    --tier ../../data/bench-compare/tiers/osm-1e7.parquet --out cameras/osm-1e7.json && cd -

# 4. Mosaica, in release mode, with a target directory of its own
CARGO_TARGET_DIR=$PWD/target CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_DEBUG=0 \
    nice -n 19 cargo build --release -p mosaica --bin mosaica \
    && rustc --version > target/release/mosaica.rustc   # the toolchain, for the run's record

# 5. The ClickHouse image (26.8.34.13), by the digest adapters/clickhouse.py runs
docker pull clickhouse/clickhouse-server@sha256:8b28eb09479c36c4533eec7a6268917d04ba3a6fd90e3f4205f70e761b6d7779

# 6. One run per system; each appends a record to results/dev.jsonl
cd bench/compare
.venv/bin/python run.py --system mosaica --tier ../../data/bench-compare/tiers/osm-1e7.parquet \
    --cameras cameras/osm-1e7.json --binary ../../target/release/mosaica
.venv/bin/python run.py --system mosaica+aggregate --tier ../../data/bench-compare/tiers/osm-1e7.parquet \
    --cameras cameras/osm-1e7.json --binary ../../target/release/mosaica
.venv/bin/python run.py --system clickhouse --tier ../../data/bench-compare/tiers/osm-1e7.parquet \
    --cameras cameras/osm-1e7.json

# 7. The tables
.venv/bin/python tables.py results/dev.jsonl
```

`run.py` refuses to start above a one-minute load of 3 (`--max-load`); on a shared machine, for a
development run whose timings do not matter, raise it, and pass `--nice 19`, which runs every
system's build at that niceness (Mosaica's `mosaica build`, and ClickHouse's server for the
duration of its `INSERT` and `OPTIMIZE`) and every server at 0. The load before and after is in the record.
A run builds the system from nothing, checks every frame against the oracle, runs each workload
five times warm, then once cold per battery camera, and stops and removes everything it started.
Work directories go under `data/bench-compare/work/`; delete them when done.

## Files

| File | What it does |
|---|---|
| `requirements.txt`, `requirements.lock` | The direct pins, and every package with its hashes |
| `tiers-manifest.json` | Each tier's row count and SHA-256, which `prepare.py` must reproduce and `run.py` checks |
| `prepare.py` | Downloads and checks the source, derives the tiers deterministically, and writes `data/bench-compare/tiers/manifest.json` with every checksum, seed and count |
| `frame.py` | The frame's definition: screen, tiles, grid depth, K, and the quantisation rule |
| `cameras.py` | The three workloads (battery, camera script, filter changes) from a tier's density |
| `oracle.py` | The exact frame by DuckDB over the tier file, and the comparison of a frame against it |
| `adapters/` | One adapter per system behind `adapters/base.py` |
| `run.py` | One run of one system, appending one record to a JSONL file |
| `tables.py` | Records to Markdown tables, one column per system |

## The data

The primary dataset is the HoloViz copy of the OpenStreetMap bulk GPS points
(`osm-1billion.parq.zip`, 5,498,778,647 bytes, SHA-256
`7b6e89c3b8be96813c67682d6c9e6a05519ef2e31136378123c9dfea3b93ef57`, 1,000,050,363 rows of float32
Web Mercator `x` and `y`). Neither HoloViz nor the OpenStreetMap wiki states a licence for the GPS
dump; settle that before publishing anything drawn from it.

A tier of nominal size N keeps row `rid` where `splitmix64(rid ^ 0x6D6F736169636131) <
floor(N · 2⁶⁴ / 1,000,050,363)`, `rid` being the row's position across the part files in part
order. The tiers nest. Two synthetic attributes are derived from `splitmix64(rid ^
0x6174747269627331)`, since the file has none: `band` (A 50%, B 30%, C 14.9%, D 5%, E 0.1%) and
`num` (0 to 999, uniform). `prepare.py`'s docstring gives the exact rule.

## The frame

A screen of 1920 × 1080 pixels at an integer zoom, with 512-pixel tiles: the depth-`zoom` tiles it
touches, each tile's matching count, a count grid at depth `min(zoom + 8, 16)` (256 × 256 cells a
tile to zoom 8), and every matching position in each tile with at most K = 500 of them. Positions
are compared on the 2³² grid that Mosaica's build quantises to (`frame.fixed32`), so an exact frame
has zero error.

## Departures from the design

- **The grid stops at depth 16.** Mosaica's viewport grid is 2¹⁶ cells a side and its sub-cell
  counts cannot go deeper, so the design's 256 × 256 cells a tile holds only to zoom 8. Every
  system is asked for the same coarser grid beyond that (16 × 16 a tile at zoom 12, one cell at
  zoom 16). Mosaica's `/v1/aggregate` can count to depth 32, but as a second request.
- **Mosaica's viewport grid ignores the filter.** The sub-cell counts (`underlay_offset`) are
  taken over the viewer's visible set, not the filtered set, while the tiles' `matched` counts do
  follow the filter. So the `mosaica` arm's filtered frames have the wrong grid, and the record
  counts the error. The `mosaica+aggregate` arm sends a `/v1/aggregate` request for the filtered
  cells at the same time as the viewport request, at the cost of a second request.
- **Synthetic filters.** The design's filters name GBIF columns; the OSM file has only positions.
  Stage 1 filters on the synthetic `band` and `num` at about 50%, 5% and 0.1%, and on a box. Text
  search is not in stage 1, and a box stands in for a polygon. The attributes are spatially
  uniform, which real attributes are not.
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
  none; Mosaica sends K anyway, so its responses are larger. Both counts are in the record.
- **Cold means this system's files only.** The machine's page cache is shared with other work, so
  a cold start drops the system's own files' pages with `posix_fadvise` and deletes its caches,
  rather than dropping every cache on the machine.
- **Memory** is reported twice: the server process's peak RSS, and its cgroup's `memory.peak`,
  which includes the page cache it filled. Mosaica maps its bundle, so its RSS counts pages
  ClickHouse's would not; the cgroup figure is the comparable one.
