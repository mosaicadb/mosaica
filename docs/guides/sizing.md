# Size and tune a deployment

How much memory and disc Mosaica needs depends on how many items you have and on what your schema does with their fields. The most reliable estimate comes from building a sample of your own data, so this page explains what uses each resource, how to build that sample, and which limits to change once the server runs.

## Memory

A running server needs memory for two things: its caches, which have fixed limits, and the bundle's files, which use whatever is left.

With the default limits, what [the caches](#the-caches) lists adds up to about 7.7 GiB, and a few small fixed buffers, such as the request log's queue, add a little more. You can lower the limits for a small database, or raise them when many users each hold their own access attributes.

The bundle isn't loaded into memory. Mosaica maps its files, so the kernel keeps the parts people are looking at in memory and drops the rest when memory runs short. Set the server's memory cap (`MemoryMax` under systemd, or the container's memory limit) to the cache limits plus the bundle's hot set, the parts your users read most. [How much to give a server](#how-much-to-give-a-server) explains how to choose both.

`mosaica status` shows what the process holds. `heap.anon_bytes` is the caches and the allocator's own memory, and `heap.file_bytes` is how much of the bundle is in memory:

```console
mosaica$ mosaica status --control unix:/run/mosaica/control.sock | jq '.heap | {anon_bytes, file_bytes}'
{
  "anon_bytes": 6320128,
  "file_bytes": 19673088
}
```

A build sizes itself separately. `mosaica build --memory-budget 24g` keeps the build's own structures within 24 GiB. Without it, the build takes 80% of the memory available when it starts, so set one when a server runs on the same machine. A text index takes whatever its stage leaves of the budget, so a larger budget writes fewer, larger intermediate files and merges them in fewer passes; the build prints both figures before it starts. A server indexes the text of each flush within half of 80% of the memory available when the flush starts, since a compaction may run beside it and takes the other half.

### The caches

Each limit is set under `[serve]` in `mosaica.toml`, except where the table says it is fixed:

| Key | Default | What it holds |
|---|---|---|
| `row_projection_cache_bytes` | 2 GiB | For each set of access attributes, view and shard, which rows that set may see. Shared by every session holding the same attributes |
| `fragment_cache_bytes` | 1 GiB | For each distinct set of access attributes, the items it admits. Copies on disc, in the cache directory, outlive the memory |
| `masked_count_cache_bytes` | 1 GiB | The counts, centroids and boxes of annotation levels served from a column, as each set of attributes sees them, and the tallies of number and timestamp fields. Empty without such a level. A copy on disc, bounded by `figures_disk_bytes` (8 GiB), lets a restart read them back |
| `region_cache_bytes` | 256 MiB | Drawn filter regions broken into map tiles, shared between viewers |
| `filter_cache_bytes` | 1 GiB | The items each filter condition matches (a `match`, a `phrase`, or a range, `eq` or `in` on a column), for each set of access attributes, until the next flush, coalesce or compaction. An entry is about 12 MB at 100 million items for a condition half of them match, scattered. Each request in flight can hold one more beside the limit. A filter answered over the whole map also keeps the map rows of the items it matches, and a condition on a rendered column read from the map's rows keeps the rows read so far and their matches, so a pan reads only what it brings into view |
| `occupancy_cache_bytes` | 32 MiB | How many tiles at each zoom hold something a set of access attributes can see |
| Recent changes (fixed) | 1.5 GiB | What the server subtracts and adds on each request for deleted, suppressed and newly ingested items, and the labels of denied rows |
| Paged reads (`bulk_admission` × 7 × `max_page_bytes`) | 896 MiB | The pages of `POST /v1/items` and `POST /v1/artifacts` in flight: two reads at once, each holding up to seven pages of at most 64 MiB. The server logs the figure when it starts, as `bulk_read_memory_bytes` |

??? info "Sizing the annotation count caches"
    `masked_count_cache_bytes` bounds two caches, each to that size: the annotation counts, and the tallies of number and timestamp fields, which are under a kilobyte each.

    The annotation counts hold one entry for each set of access attributes and each annotation level served from a column, whatever the number of shards. Per artifact, an entry takes 4 B for counts alone and 40 B with a centroid and a box. A layer that serves a box also keeps the eight most extreme rows on each side, which adds 4 B, plus 128 B for each artifact with more than sixteen placed rows, or 256 B in a bundle of several shards.

    So a level of 1.4 million artifacts takes 56 MB for each set of attributes counted with geometry, and a layer serving a box keeps at most 185 MB more in the same entry, or 364 MB over several shards. The default then holds about nineteen such entries, and at least four with their extra rows, or two over several shards. A deployment where every user holds their own attributes needs one entry per user per level.

    An entry larger than the limit is served and not kept, and one that fits only without its extra rows is kept without them. `masked_count_cache.not_admitted` in `/control/status` counts both.

    On disc, `figures_disk_bytes` keeps the same entries and the labels of denied rows they're corrected with. In a bundle of several shards it also keeps each shard's pass over each set's items, which a compaction of one shard reads back so as to redo that shard alone: up to 172 B for each artifact a shard holds a member of, for each version of the level. Size it as the memory limit times the number of entries a restart should find, plus those passes, plus about 24 B per denied row per level. Past it, the files least recently written or read are removed.

### How much to give a server

Give the server its cache limits plus its hot set:

- **Cache limits.** The defaults suit a server that has the memory for them. Size `row_projection_cache_bytes` and `fragment_cache_bytes` to the distinct sets of access attributes in use at once. Each set costs up to about one bit per item in each of the two caches, so 12 MiB in each at 100 million items, and much less where the items it sees sit together on the map. A set that doesn't fit is rebuilt on its next request, which costs CPU in proportion to the corpus.
- **Hot set.** Run the server without a cap under real traffic and read `heap.file_bytes` from `mosaica status` once it has settled.

For example, 100 million items, 200 sets of attributes in use, the other caches at their defaults and a measured hot set of 2 GiB come to about 12 GiB: 2.4 GiB for each of the two access caches, 4.7 GiB for the rest and 2 GiB of hot set.

### What happens beyond the hot set

While the memory beyond the caches holds the hot set, the server runs at full speed. Once it can't, nearly every request waits on the disc. The table was measured on one 4-core machine with 100 million items of about 60 bytes each (a 5.6 GiB bundle). Ten users panned across the whole map at random, which is close to the worst case, since real users gather on the busy parts of a map. The hot set was about 2 GiB, and at a 2 GiB cap the server ran as fast as it did with none. Each figure is one 20-second run, and repeated runs differed by up to 30%.

| Memory cap | Bundle ÷ cap | Views per second | p99 latency |
|---|---|---|---|
| 2 GiB | 2.8 | 811 | 71 ms |
| 1 GiB | 5.6 | 468 | 130 ms |
| 512 MiB | 11 | 116 | 755 ms |

A larger corpus has a smaller hot set for its size: it was over half of the bundle at 10 million items and a third at 100 million.

### Points drawn per tile

`k_max_marks` (500 by default) is the most points a tile draws, and `max_k` (1,000) the most a request may ask for. Fewer points per tile means fewer pages read per view and a smaller hot set. With the example above at a 2 GiB cap and 100 users panning without pause:

| Points per tile | Points per view | Views per second | p99 latency | Reads from disc in 20 s |
|---|---|---|---|---|
| 50 | about 1,300 | 1,050 | 120 ms | 882 |
| 200 | about 2,900 | 879 | 141 ms | 12,817 |
| 500 | about 4,500 | 536 | 242 ms | 112,824 |

At 500 points a tile the hot set no longer fitted, and the server delivered fewer points a second in all than at 200. A view shows fewer points than the cap times its tiles, since many tiles hold fewer items than the cap.

### Under a small memory cap

The cache limits don't follow the memory cap, so a server capped below their total fills its caches past the cap and is stopped by the kernel. Under such a cap, set each cache to a share of it. These shares kept every server alive in the runs above, at caps from 512 MiB to 8 GiB; for a 2 GiB cap:

```toml
[serve]
row_projection_cache_bytes = 268435456  # 256 MiB, about 12% of the cap
fragment_cache_bytes = 134217728        # 128 MiB, about 6%
filter_cache_bytes = 134217728          # 128 MiB, about 6%
masked_count_cache_bytes = 67108864     # 64 MiB, about 3%
region_cache_bytes = 67108864           # 64 MiB, about 3%
occupancy_cache_bytes = 16777216        # 16 MiB, about 1%
```

??? info "How these figures arise"
    **The access caches.** Each set of access attributes in use holds its visible items twice: as a bitmap in item order, in `fragment_cache_bytes`, and as one in map order for each view and shard, in `row_projection_cache_bytes`. A bitmap stores each block of 65,536 rows in the least of three forms: 2 bytes per visible row, a flat 8 KiB, or a few bytes per run of consecutive visible rows. So a set seeing a sixteenth or more of the items, scattered, costs about one bit per item; a smaller scattered share costs about 2 bytes per visible item; and items that sit together cost far less. This is worked out from the format. Measured, each set seeing a scattered sixteenth added 1.4 MiB at 10 million items and 13.5 MiB at 100 million, against 1.2 MiB and 11.9 MiB predicted for one bitmap. At 100 million items, a thousand new sets arriving at once were limited by the CPU that building them takes, a few hundred a minute on three cores, and larger caches didn't help.

    **Why the hot set grows more slowly than the corpus.** A tile reads the positions and fields of at most its point cap of items. Once it holds many more items than fit in a page, those reads touch about one page per point drawn, whatever the corpus size. Below zoom 10 a tile finds its points from copies kept for the purpose, without reading the rest of the tile, and above it tiles are small. A filtered view reads more: where many of a tile's items match it passes over those that don't, and where few match it reads the identifier of every match. Its reads grow with the corpus either way, though more slowly than the corpus does.

## Annotation layers and new viewers

Annotation layers have one cost to plan for: the first time someone with a new set of access attributes opens a layer, the server counts its artifacts for them. Every viewer holding the same set shares the result, so a deployment whose viewers fall into a few groups pays it a few times for each level, while one where every user holds attributes of their own, such as a label per document, pays it for each user and each level they open.

The count takes longer the more of the database the attributes admit, and a request that shows several levels for the first time counts them one after another. Every later request reads the result, corrected for whatever has been deleted, suppressed or ingested since. The result is written to the cache directory, so a restart reads it back. After a compaction, each set of attributes counts the compacted shard again the next time it reads each level, and no other shard.

`masked_count_cache_bytes` decides how many results stay in memory and `figures_disk_bytes` how many stay on disc. `/control/status` counts the passes under `masked_count_cache.fills`, and results read back from disc under `masked_count_cache.loads`.

??? info "Which levels this applies to"
    The cost falls on annotation levels served from a column, which holds one entry per row naming its artifacts. Unless the layer declares `layout = "rows"`, that is every level of a `flat`, `stacked` or `tiered` layer whose memberships are enumerated, every attribute layer's level, and a `nested`, `dag` or shape level of at least 1,000 artifacts of which a quarter are spread too widely for the map's index.

    Drawing a layer or listing its artifacts reads its levels. Colouring points by a layer reads them when the map then reads the artifacts its points are tagged with.

??? info "Colouring by a layer"
    A viewer's first request for a layer's colours, with a `palette_size`, also costs one pass over the layer: the server works out a colour slot for every cluster the viewer is served, one level at a time. That's the level asked for and, on a `tiered` layer, the levels above it, never a deeper one. So a viewer whose first view of a `tiered` taxonomy is at genus or species also counts every coarser level. Where the layer declares a centroid, as a taxonomy does, the pass reads the centroids the map fills when it draws the level, and adds no pass of its own. The fill is kept for each set of attributes, so it is paid once for each. Any route that names a `palette_size`, browse and the bulk reads included, fills the same counts and centroids where nothing has yet, and browse and a bulk read of `/v1/artifacts` work out slots for every level whose rows they return, and on a `tiered` layer for the levels above each. Without a `palette_size`, no route fills anything for colours.

    The slots are kept for that credential, layer and palette size, shared by every session holding the credential. After a deletion, a suppression or a lifted one, an edit, a publication, or a change to the layer's registration or edges, the next request that colours the layer pays the pass again before it is answered, and so does every change on a layer whose criterion is a fraction or whose content has a generating set. After an ingest, a flush or a growth on any other layer, the next request is answered from the slots held, and the server works them out again once that response is sent, on at most two sessions at once, so the viewer pays nothing for it.

## Disc

Plan disc space for four things:

- **The bundle.** Deleted and edited rows stay in it until a compaction, which by default starts once they're a fifth of all rows, so a bundle can reach about a quarter more than its live size before a compaction reclaims it. The files a merge replaces are deleted a few publications later, once no request is still reading them, so merging adds little to that.
- **Room for a compaction**, which needs free space of one and a half times the size of the files the bundle uses. Without it the compaction is refused. In a bundle of several shards, each shard is compacted on its own, so the space needed is that of the largest.
- **The cache directory**, which holds work the server can redo. The saved counts are bounded by `figures_disk_bytes`, 8 GiB by default. The copies of each distinct set of access attributes' visible items aren't bounded: they grow with the number of such sets, and with each flush, until the next compaction removes them.
- **The write-ahead log**, which holds every change since the last flush, so it usually stays small. An annotation level that has grown or been filled holds the log until the next compaction, and `wal_pin` in `/control/status` says what is holding it.

Taken together, a database that's written to and compacted on the default schedule needs about three times its live bundle size, plus the cache. A database you only build and read needs the bundle and the cache. [Schedule compactions](compaction.md) explains when compactions run.

Keep the bundle and the log on a local disc. A fast SSD matters most when the bundle doesn't fit in memory, since every read the page cache misses goes to disc.

## CPU

Requests run on a pool of `compute_threads`, which defaults to the number of CPUs the process may use, so more cores let more requests run at once. Lower it to leave some for other work on the same machine. The flush, the fold and the coalesce each merge on a third of `compute_threads`, rounded up, beside that pool, so lowering it also slows them.

The annotations a map draws come from `POST /v1/artifacts/viewport`, which takes one request per compute thread at a time by default (`artifact_admission`). A request past that, and past as many waiting, is answered `429`, which `/control/status` counts under `artifacts.shed_total`. `max_artifacts_per_tile`, 1,000 by default, is the most artifacts one request may ask for in each tile and level.

## Measuring your own data

Bundle size per item varies widely with the schema: a rendered field travels with every point, an indexed text field adds a search index, and every view and annotation layer adds its own files. The bundles built in these docs show the range:

| Example | Items | Schema | Bundle | Per item |
|---|---|---|---|---|
| [Ireland](../start/first-map.md) | 29,935 | One geographic view; an indexed text field, a rendered category and a rendered number | 2.2 MB | 73 B |
| [GeoNames](../start/install.md#try-it) | 150,000 | One geographic view; an indexed text field and a rendered number | 11.7 MB | 78 B |
| [arXiv](backend.md) | 50,000 | Two views; six fields, including indexed text and an indexed keyword; two clusterings, one with a layer of names; 175 access attributes | 15.0 MB | 300 B |

The figures in [Memory](#memory) came from `scripts/bench_capacity.py` in the source repository, which builds synthetic corpora of a chosen size and shape, serves each under a range of memory caps and drives it with simulated users. To compare a machine against a corpus like yours, pick the synthetic profile closest to your schema; its `bench/README.md` explains the profiles.

To estimate your own bundle size, build a sample of your data with your real schema, read the size from the line that starts `built`, and scale it by the number of items you expect. A sample keeps per-item costs right, but fixed costs and the size of annotation layers don't scale the same way, so treat the result as a starting point and check it against a larger build before you buy hardware.

```bash
mosaica build 2>&1 | grep '^built'
```

```
built /home/you/places/bundle (v00000): 150000 items, 1 terms, 150000 pairs, 11684262 bytes on disk, 0 artifact(s) minted, 0 unclustered member row(s), 0 row(s) refused
```
