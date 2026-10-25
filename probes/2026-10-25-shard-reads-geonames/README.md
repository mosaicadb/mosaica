# GeoNames read at 1, 2, 8, 32 and 128 shards

**Status:** measurement, 2026-10-25, with `mosaica 0.1.0 (e54c2c6a6)`, the release binary built
from main at e54c2c6a6 and kept at `data/stage7/bin/mosaica-e54c2c6a6` (sha256 `434041468a55…`),
and the TypeScript core of the same commit. Host: the 12-core Ryzen 9 5900X with 47 GiB under WSL2,
kernel 6.18, the bundles on its ext4 disc. Every server ran under a 24 GiB memory cap. Every figure
is measured. The Tree of Life builds at the end were made on 2026-10-22 with the binary of
e73d47ad6.

This is §8.2 of [the sharding plan](../../docs/sharding.md) for GeoNames, the read side's figures
at five shard counts. Tree of Life and GBIF were not read. The battery at 128 shards was stopped
part-way to free the machine, so at N = 128 only the interactive bench's figures exist.

## What was run

GeoNames' 13,463,857 places were built five times. One shard is built at the default size; each
other count N is built at a `[shards] size` of the items over N, rounded up: 6,731,929 at N = 2 and
105,187 at N = 128. The build cuts its one numbering of the items, signature batches in the
allocation view's order, into shards of that size (§5 of the plan).

Each bundle then ran two benches, one after the other, each on a server it started.

- **The interactive bench** (`test_corpora/common/interactive_bench.py`) opens a session for
  principals seeing 1%, 7%, 85% and 100% of the corpus, runs a fixed camera script for each, and
  looks up 200 items by item card and by a unique `eq` filter on `id`. Its session and map
  sections ran once at one shard, as the reference (`runs/geonames-reference.json`), and every
  count, one shard included, replayed that run's requests (`--replay`). All 498 requests were sent
  at every N, none left out. The server starts with an empty cache directory, so each principal's
  first session (`first`) is new to the server, its second (`again`) shares the grant's
  projections and figures, and `reopen` is a restarted server over the same cache directory.
- **The serve battery** (`test_corpora/common/serve_battery.py`) asks viewports at zooms 0, 3,
  6, 9 and 12 at the budget's depth 9, 20 cells a zoom drawn by density decile, for the same four
  principals. Each cell has 15 samples under each of three conditions: `cold` (pages evicted and a
  fresh session before every sample), `cold_pages_warm_engine` (pages evicted, the session kept)
  and `hot` (one location repeated). Then ten text searches each for a common token and an absent
  one.

Sequence: [`run.sh`](run.sh), with the builds run first (`BUILD_ONLY=1`):

```bash
cargo build --release -p mosaica
npm --prefix clients/ts run build -w @mosaicajs/client
BINARY=data/stage7/bin/mosaica-e54c2c6a6 CORE=data/stage7/src/clients/ts/core/dist/index.js \
  LADDER=data/ladder OUT=data/stage7/reads CORPUS=geonames ITEMS=13463857 \
  SHARDS="1 2 8 32 128" UNIQUE=id TEXT=name RANKS=country-ranks.json \
  bash probes/2026-10-25-shard-reads-geonames/run.sh
python3 probes/2026-10-25-shard-reads-geonames/collate.py data/stage7/reads geonames > ratios.md
```

`run.sh` waits for a one-minute load average under 3 before each timed run and records it in
`load.txt`. The builds' load was not recorded; their user CPU time is in Table 2.

| N | before the reference run | before the bench | before the battery |
|---:|---:|---:|---:|
| 1 | 1.99 | 2.74 | 2.86 |
| 2 | | 1.21 | 0.23 |
| 8 | | 1.50 | 0.21 |
| 32 | | 1.15 | 0.26 |
| 128 | | 1.00 | 0.48, stopped |

[`ratios.md`](ratios.md) holds every figure as its ratio to one shard, and the build's and the
server's figures as measured. The tables below are taken from it. A cold row there shows "–" where
fewer than 90% of its samples took a major fault.

## Findings

**Requests with nothing to combine cost the same up to 32 shards.** The bench rounds a lookup to 0.1
ms. An item card and a unique `eq` lookup took within 0.1 ms of one shard's 0.8 to 1.2 ms up to N =
32, and 0.1 to 0.5 ms more at N = 128. A unique lookup reads a run of the field's index in every
shard, so it does some work per shard. A hot viewport, at every zoom and for every principal, took
0.85 to 1.07 times one shard's 2.6 to 37 ms. A hot text search took 0.86 to 1.08 times one shard's.
The battery did not reach N = 128, so the hot viewport and text have no figure there. The bench's
viewport requests over the whole run (198 of them) took 0.90 to 0.93 times one shard's median up to
N = 32 and 1.32 times at N = 128.

**A grant's first request costs 4 to 5 ms a shard, whatever the grant sees.** A session's first
counts, on a server that has not seen the grant, arrived after 25 to 46 ms at one shard, 147 to 178
ms at N = 32 and 578 to 610 ms at N = 128, the same for the principal who sees 1% as for the one who
sees everything (Table 2). Each is one sample, for each of four principals. Before the first
viewport sends its head, `Engine::derive_geometry`
(`crates/mosaica-engine/src/viewport/geometry.rs`) runs for every shard, building the grant's mask
fragment (`Engine::fragment_for`) and then its row projection (§3.7 of the plan). Which of the two
costs the time was not traced. For the 85% principal at N = 32 the head came 156 ms after the
request was sent, against 22 ms at one shard. The second session for the same grant shares those
projections, and its first counts took 12.7 to 24.5 ms at N = 32 and 128 against 13.8 to 27.6 ms at
one shard, within the spread of single samples. A battery rung's first viewport, on a fresh session,
is one request: for the 1%, 7% and 85% principals it took 2.07 to 3.48 times one shard's at N = 32,
and for the 100% principal 0.63 times.

**A grant's first figures walk every shard once per level.** The first artifacts request for a grant
sums each level's figures over the shards, walking each shard's rows apart and writing each walk to
the cache directory (`Engine::part_of` in `crates/mosaica-engine/src/figures/sum.rs`). The server
counted 20 walks at one shard, four grants at five administrative levels, and 20 × N at N, 2,560 at
N = 128. The first session's `layers` time was 0.94 to 1.01 times one shard's for the 1% and 7%
principals up to N = 32, and 1.21 to 1.59 times for the 85% and 100% ones at N = 8 and 32; at N =
128 it was 1.92 to 3.55 times. The second session's was 0.99 to 1.02 times one shard's at every N.
Writing the walks takes the cache directory from 103 MB at one shard to 274 to 277 MB at every other
N.

**Opening a bundle costs time and memory per shard.** The server answered `/readyz` after 4.0 s at
one and two shards, 5.0 s at 8, 10.7 s at 32 and 22.6 s at 128. Its anonymous resident memory at
`/readyz` was 0.92 GiB at one shard and 7.47 GiB at 128, about 53 MiB more a shard. What holds that
memory was not traced here.

**Some requests that combine grow with N.** The viewer's count over a region (`/v1/aggregate` with
a region filter and a count) took 1.9 ms at one shard, 2.71 times at N = 32 and 9.34 times at
N = 128, about 0.12 ms a shard. Zoomed out at zooms 1 to 4, the last byte of the 85% and 100%
principals' points grew from 1.32 to 1.44 times at N = 8 to 1.97 to 2.29 times at N = 128; the 1%
and 7% principals' stayed at 0.81 to 1.19. Zooming in at zooms 9 to 11, the dense principals' first
counts took 60 to 73 ms at one shard and 80 to 104 ms at every other N, 1.32 to 1.42 times; the 1%
principal's took 0.98 to 1.04 times. Neither was traced to its cause here.

**A build's time grows with the shards and its memory does not.** Each count was built once. The
build took 68 s at one and two shards, 82 s at 8, 107 s at 32 and 233 s at 128, and 95, 97, 109, 136
and 273 s of user CPU time. Its peak resident memory was 2.48 to 2.61 GiB at every N. The bundle
grew from 2.001 GB at one shard to 2.050 GB at 32 and 2.132 GB at 128.

**The cold battery cells at N = 2 and 8 hold almost no cold sample.** Of 12,000 cold samples at each
count, the battery counted 1 as cold at N = 2 and 3 at N = 8, against 11,920 at one shard and 5,430
at N = 32. The battery takes a sample as cold only where the server's major-fault count rose during
its request, and reports its percentiles over those samples alone. Eviction is
`posix_fadvise(DONTNEED)` over the bundle's and cache's files and a write to `memory.reclaim` on the
server's cgroup. On these two runs neither dropped the server's pages: the warm-engine condition's
median over all samples was 3.1 to 3.3 ms, at the hot condition's 2.7 to 3.0 ms, against 5.5 to 10.0
ms at one shard. The battery ignores an error writing `memory.reclaim`, so the JSON does not say
why. The earlier runs on the binary of e73d47ad6 counted 11,998 cold samples at N = 2 and 5,428 at N
= 8, so the failure does not follow the shard count. At N = 32 the cold samples are only those that
took at least one fault: their warm-engine median, 3.5 to 4.3 ms, is near the hot one, so the cold
ratios of 0.29 to 1.32 at N = 32 do not compare with one shard's either. A cold text search counts
as cold on one fault, and cannot be told apart in the same way. No cold viewport or cold text search
has a figure at several shards. `collate.py` shows "–" for a cold row where fewer than 90% of its
samples took a fault; the zoom-0 rows at N = 32 pass that floor and are still not cold reads.

## What it decides

`shards.size` stays at 2³² − 2²⁸ on the reads and the build. No figure here is cheaper at more
shards beyond the 0.56 to 2.29 spread of single samples, leaving aside the cold cells whose eviction
failed. The costs that grow with N are fixed costs per shard: a grant's first request, a grant's
first figures, the time to `/readyz` and the memory held at open, the build, and the region count.
Each favours fewer, larger shards. The costs that could favour smaller shards, a smaller shard's
compaction time and memory and restart replay, are not measured (§8.3 of the plan).

These runs decide nothing about `REUSE_MIN_FREE` or compaction, which need the churn at several
shards and one large compaction (§8.3 and §8.4 of the plan). Still unmeasured on the read side: a
cold viewport or cold text search at several shards, the aggregate's bins, sampled bins and field
summary, a bulk read, Tree of Life and GBIF, what holds the memory each shard adds at open, and the
causes of the growth in the region count and the dense principals' camera figures.

The session and camera figures are one to four samples each. The reference run and its replay at one
shard, on the same bundle, differ by 0.56 to 2.29 times on the session's first counts and first
points, so a ratio within that band says nothing on its own. A ratio that grows steadily with N, as
the first counts do, is what the tables can show.

## Table 1: requests with nothing to combine

Ratio to one shard. One shard's figure is the median in ms; a lookup is the median of 200, a
battery figure the median over the cells of each cell's median server time.

| figure | N = 1 | N = 2 | N = 8 | N = 32 | N = 128 |
|---|---:|---:|---:|---:|---:|
| item card, 1% | 0.8 ms | 1.12 | 1.12 | 1.12 | 1.38 |
| item card, 100% | 0.9 ms | 1.00 | 1.00 | 1.11 | 1.22 |
| unique `eq` lookup, 1% | 1.2 ms | 0.92 | 0.92 | 0.92 | 1.08 |
| unique `eq` lookup, 100% | 1.1 ms | 1.09 | 1.00 | 1.09 | 1.45 |
| hot viewport, 1%, zoom 0 | 19.1 ms | 0.94 | 0.87 | 0.90 | – |
| hot viewport, 1%, zoom 12 | 2.6 ms | 1.04 | 1.02 | 1.01 | – |
| hot viewport, 100%, zoom 0 | 36.5 ms | 0.94 | 0.96 | 1.06 | – |
| hot viewport, 100%, zoom 12 | 2.8 ms | 1.03 | 1.01 | 1.07 | – |
| hot viewport, every principal and zoom | 2.6 to 37.2 ms | 0.89 to 1.05 | 0.87 to 1.02 | 0.85 to 1.07 | – |
| text, common token, hot | 42.9 ms | 0.86 | 1.00 | 1.07 | – |
| text, absent token, hot | 37.3 ms | 0.91 | 1.01 | 1.08 | – |
| the bench's viewport requests, 198 | 6.0 ms | 0.93 | 0.90 | 0.92 | 1.32 |

## Table 2: what grows with N

| figure | N = 1 | N = 2 | N = 8 | N = 32 | N = 128 |
|---|---:|---:|---:|---:|---:|
| first counts, new grant, 1% (ms) | 31.7 | 36.8 | 61.4 | 177.8 | 609.7 |
| first counts, new grant, 7% (ms) | 46.0 | 28.4 | 53.5 | 157.8 | 578.3 |
| first counts, new grant, 85% (ms) | 25.3 | 28.5 | 52.7 | 159.0 | 605.1 |
| first counts, new grant, 100% (ms) | 34.3 | 29.2 | 60.3 | 146.7 | 601.8 |
| first counts, same grant again, 1% to 100% (ms) | 13.8 to 27.6 | 13.4 to 16.8 | 11.6 to 16.9 | 12.7 to 18.6 | 14.5 to 24.5 |
| settled, new grant, 85% (ms) | 370.7 | 374.7 | 427.0 | 502.7 | 946.5 |
| layers, new grant, 100% (ms) | 235.5 | 234.5 | 294.3 | 374.1 | 835.7 |
| figures walks in the run, four grants at five levels | 20 | 40 | 160 | 640 | 2,560 |
| region count, `/v1/aggregate`, median of 92 (ms) | 1.9 | 1.8 | 2.5 | 5.2 | 17.8 |
| points' last byte, zooms 1 to 4, 100% (ms) | 247.4 | 249.1 | 327.4 | 417.0 | 487.9 |
| `/readyz` after start (s) | 4.0 | 4.0 | 5.0 | 10.7 | 22.6 |
| anonymous memory at `/readyz` (GiB) | 0.92 | 1.01 | 1.41 | 2.57 | 7.47 |
| peak resident memory after the run (GiB) | 2.46 | 2.42 | 2.89 | 4.11 | 9.29 |
| build, wall (s) | 68 | 68 | 82 | 107 | 233 |
| build, user CPU (s) | 95 | 97 | 109 | 136 | 273 |
| build, peak resident (GiB) | 2.61 | 2.60 | 2.52 | 2.48 | 2.54 |
| bundle (GB) | 2.001 | 2.007 | 2.020 | 2.050 | 2.132 |

## The fixes these runs led to

An earlier round of the same runs, on 2026-10-22 with the binary of e73d47ad6 and without the
replay, found the costs below. Its figures are in `data/stage7/reads/old-oct22/` and are not
kept here. Each fix is on main at e54c2c6a6, which the figures above measure.

| what | before | after |
|---|---|---|
| a viewport's tiles sorted once per request, and a shard's empty runs passed over in one search (9d087a32c) | a hot request's tile-range stage took 3, 5, 18, 69 and 280 ms at N = 1, 2, 8, 32 and 128 (commit); a hot viewport took 14.4 to 15.0 times one shard's at N = 32 and 52 to 54.5 times at 128, zooms 6 and 12 | about 2 ms at every N to 128 (commit); 0.85 to 1.07 times at N = 32 |
| a level's figures held as one sum per grant over every shard (dc31a60c3, d5235879a, 3c872b11a) | held per grant and shard, each dense over the level: one grant's level 4 at 128 shards came to 1.3 GB, past the cache's bound, so every repeat read every shard's entry back from disc (commit); the second session's first counts, 100%, took 6.85 times one shard's at N = 32 and 18.3 times at 128 | 12.1 MB held at N = 32 and 128 for the level against 11.1 MB at one (sharding plan §3.6); 0.67 and 0.89 times |
| the key filter read in order at open, and a batch asking for the filter pages it lacks (dd78dddd5, d6ae4b00d, 7a9234c65) | a cold open of a 2²⁶ run at 10 bits took 2.0 s; a batch of 10,000 absent keys over a cold filter 1.1 s | 35 ms and 27 ms ([key filter probe](../2026-10-22-key-filter-bits/README.md), Table 5) |
| another shard's row-major column kept through a compaction while it is behind its level, and completed per artifact (5f8beddd5 and ed2a5ca50, superseding f68d0ec40) | a compaction of one shard writes every shard's member packs again. A column written before its slice last moved was completed against the new pack, where the joins since already read as held: a growth in shard 1, a compaction of shard 0 and a restart served shard 1's level without the growth. f68d0ec40 removed such a column from the manifest and composed the level again on first use, which also composed again columns that lacked nothing | a row-major column and its member file stand behind the level's versions and never ahead (`DerivedExtent::stands` in `crates/mosaica-store/src/manifest.rs`), and the reader completes the column per artifact, counting the members with a row in its shard's base (sharding plan §4.7); `crates/mosaica-engine/tests/several_shards.rs` |

The bench's replay (f1991291f, b1ee9b915) came from the same round. Without it the client asks each
bundle for different work, since each build draws a new `mosaica_id` key and a tile's points are
chosen in `mosaica_id` order.

## Tree of Life builds

Tree of Life's 233,055,986 items were built at 1, 4 and 16 shards on 2026-10-22, with the binary of
e73d47ad6, at sizes of 233,055,986, 58,263,997 and 14,566,000. Other jobs shared the host: the
one-minute load average at the start of each build was 2.4, 5.7 and 91, and the 15-minute one at the
start of the 16-shard build was 156. The builds averaged 1.34, 0.68 and 1.08 CPUs, so their times
say what the load allowed and are not compared. Their peak memory and bundle bytes are as measured:

| N | peak resident (`/usr/bin/time`, KiB) | peak resident (GiB) | bundle (GB) | wall, not comparable |
|---:|---:|---:|---:|---:|
| 1 | 15,015,936 | 14.3 | 50.86 | 36 min 58 s |
| 4 | 20,821,356 | 19.9 | 51.38 | 85 min 3 s |
| 16 | 23,374,848 | 22.3 | 52.09 | 69 min 50 s |

No read ran on these bundles.

## Files

- `runs/geonames-<N>/bench.json`: the interactive bench at each N, with `build-time.txt`,
  `bundle-bytes.txt` and `load.txt`. `runs/geonames-reference.json` is the reference run.
- The battery's JSON, about 4.2 MB a count, is not kept here. It is at
  `data/stage7/reads/geonames-<N>/battery.json` for N = 1, 2, 8 and 32, with each count's logs.
  `collate.py` over `runs/` prints the bench's figures and "–" for the battery's.
- `runs/treeoflife-<N>/`: the Tree of Life builds' `build-time.txt` and `bundle-bytes.txt`.
