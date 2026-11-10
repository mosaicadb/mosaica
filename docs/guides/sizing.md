# Size and tune a deployment

How much memory and disc Mosaica needs depends on how many items you have and on what your schema does with their fields. The most reliable estimate comes from building a sample of your own data, so this page explains what uses each resource, how to build that sample, and which limits to change once the server runs.

## Memory

A running server needs memory for two things: its caches, which have fixed limits, and the bundle's files, which use whatever is left.

With the default limits, the caches and the paged reads add up to about 7.9 GiB. You can lower them for a small database, or raise them when many users each hold their own access attributes. [The caches](#the-caches) lists each one.

The bundle isn't loaded into memory. Mosaica maps its files, so the kernel keeps the parts people are looking at in memory and drops the rest when memory runs short. Set the server's memory cap (`MemoryMax` under systemd, or the container's memory limit) above the cache limits, and leave enough beyond them for the parts of the bundle your users read most, its hot set. While the memory beyond the caches holds the hot set, the server runs at full speed; once it can't, nearly every request waits on the disc and throughput falls steeply. [Memory beyond the caches](#memory-beyond-the-caches) gives measured figures and how to find your own hot set.

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

Each cache has a limit you can set under `[serve]` in `mosaica.toml`:

| Key | Default | What it holds |
|---|---|---|
| `row_projection_cache_bytes` | 2 GiB | For each set of access attributes, view and shard, which rows that set may see. Shared by every session holding the same attributes |
| `fragment_cache_bytes` | 1 GiB | For each distinct set of access attributes, the items it admits. Copies on disc, in the cache directory, outlive the memory |
| `masked_count_cache_bytes` | 1 GiB | The counts, centroids and boxes of annotation levels served from a column, as each set of attributes sees them, and the tallies of number and timestamp fields. Empty without such a level |
| `figures_disk_bytes` | 8 GiB | The same counts, kept on disc in the cache directory so that a restart reads them back |
| `region_cache_bytes` | 256 MiB | Drawn filter regions broken into map tiles, shared between viewers |
| `filter_cache_bytes` | 1 GiB | The items each filter condition matches (a `match`, a `phrase`, or a range, `eq` or `in` on a column), for each set of access attributes, until the next flush, coalesce or compaction. An entry is about 12 MB at 100 million items for a condition half of them match, scattered. Each request in flight can hold one more beside the limit. A filter answered over the whole map also keeps the map rows of the items it matches, and a condition on a rendered column read from the map's rows keeps the rows read so far and their matches, so a pan reads only what it brings into view |
| `occupancy_cache_bytes` | 32 MiB | How many tiles at each zoom hold something a set of access attributes can see |

Beside these, the server keeps what it subtracts and adds on each request for deleted, suppressed and newly ingested items, and the labels of denied rows, under fixed limits that come to 1.5 GiB. The paged reads, `POST /v1/items` and `POST /v1/artifacts`, share one more: two may run at once (`bulk_admission`), each holding up to seven pages of at most 64 MiB (`max_page_bytes`). The server logs what that comes to when it starts, as `bulk_read_memory_bytes=939524096`.

??? info "Sizing the annotation count caches"
    `masked_count_cache_bytes` bounds two caches, each to that size: the annotation counts, and the tallies of number and timestamp fields, which are under a kilobyte each.

    The annotation counts hold one entry for each set of access attributes and each annotation level served from a column, whatever the number of shards. Per artifact, an entry takes 4 B for counts alone and 40 B with a centroid and a box. A layer that serves a box also keeps the eight most extreme rows on each side, which adds 4 B, plus 128 B for each artifact with more than sixteen placed rows, or 256 B in a bundle of several shards.

    So a level of 1.4 million artifacts takes 56 MB for each set of attributes counted with geometry, and a layer serving a box keeps at most 185 MB more in the same entry, or 364 MB over several shards. The default then holds about nineteen such entries, and at least four with their extra rows, or two over several shards. A deployment where every user holds their own attributes needs one entry per user per level.

    An entry larger than the limit is served and not kept, and one that fits only without its extra rows is kept without them. `masked_count_cache.not_admitted` in `/control/status` counts both.

    On disc, `figures_disk_bytes` keeps the same entries and the labels of denied rows they're corrected with. In a bundle of several shards it also keeps each shard's pass over each set's items, which a compaction of one shard reads back so as to redo that shard alone: up to 172 B for each artifact a shard holds a member of, for each version of the level. Size it as the memory limit times the number of entries a restart should find, plus those passes, plus about 24 B per denied row per level. Past it, the files least recently written or read are removed.

### Memory beyond the caches

The hot set depends on where your users look, so the reliable figure is your own: run the server without a cap under real traffic and read `heap.file_bytes` from `mosaica status` once it has settled. Give the server at least that much beyond its cache limits.

As an example of the shape, the table below was measured on one 4-core machine with a corpus of 100 million items of about 60 bytes each (a 5.6 GiB bundle) and default cache limits. Ten users panned close-ups across the whole map at random, which is close to the worst case for the page cache, since real users gather on the busy parts of a map. Its hot set was about 2.0 GiB, a third of the bundle. Each figure is one run of 20 seconds, and repeated runs of a cell differed by up to 30%.

| Memory cap | Bundle ÷ cap | Requests per second | p99 latency | Reads from disc in 20 s |
|---|---|---|---|---|
| None | | 671 | 83 ms | 783 |
| 2 GiB | 2.8 | 811 | 71 ms | 3,032 |
| 1 GiB | 5.6 | 468 | 130 ms | 111,985 |
| 512 MiB | 11 | 116 | 755 ms | 258,300 |

With these users, a bundle up to about three times the memory beyond the caches served at full speed. A narrower map, users who stay in one region, or a lower [point cap](#points-drawn-per-tile) lets the ratio go higher; more columns, indexed text and filters make the hot set larger.

A larger corpus should tolerate a higher ratio under the same users, because an unfiltered view reads a bounded amount of the bundle. A tile reads the positions and fields of at most its point cap of items, so once a tile holds many times more items than a page, those reads touch about one page per item drawn, whatever the corpus size. Below zoom 10, a tile finds those items from copies kept for the purpose without reading the rest of the tile; above it, a tile holds few enough items that reading them all is cheap. The two corpora measured agree: the hot set was over half of the bundle at 10 million items and a third at 100 million. This is inferred from how a view reads and from those two sizes, and it doesn't hold for a filtered view, which reads the identifier of every item that matches in view and so grows with the corpus.

### Users who see different items

Each distinct set of access attributes that is in use holds two bitmaps of the items it may see: its visible set, in item order, and its row projection, in map order, one for each view and shard. Sessions holding the same attributes share both. A bitmap is stored in blocks of 65,536 rows, and each block takes the least of three forms: 2 bytes for each visible row, a flat 8 KiB (one bit for every row), or a few bytes for each run of consecutive visible rows. So, for each bitmap:

- **Visible items scattered through the corpus**, a sixteenth of the items or more: about one bit for every item in the corpus, whatever the share. At 100 million items that is 12 MiB.
- **A smaller scattered share**: about 2 bytes for each visible item.
- **Visible items that sit together**, because the attributes follow regions of the map or the build's ordering: much less, a few bytes per run.

This is worked out from the bitmap format and agrees with what was measured. In the example above, each user with attributes of their own, seeing a sixteenth of the items scattered, added 1.4 MiB at 10 million items and 13.5 MiB at 100 million, against 1.2 MiB and 11.9 MiB predicted for one bitmap.

A set's bitmaps are built on its first request, which costs CPU in proportion to the corpus. At 100 million items, a thousand users arriving at once with attributes of their own were limited by that work: the server built a few hundred sets a minute on three cores, and raising `row_projection_cache_bytes` to 8 GiB and `fragment_cache_bytes` to 4 GiB did not help.

### Under a small memory cap

The cache limits are fixed, so a server capped below their total keeps filling its caches past the cap and is stopped by the kernel. In the example above, a server with default limits was stopped at caps of 512 MiB and 1 GiB once many users held attributes of their own. Under a cap below the default total, lower the limits yourself. Setting `row_projection_cache_bytes` to 12% of the cap, `fragment_cache_bytes` and `filter_cache_bytes` to 6% each, `masked_count_cache_bytes` and `region_cache_bytes` to 3% each and `occupancy_cache_bytes` to 1% kept every server in that run alive, though one serving many users with attributes of their own was slow at caps well below its hot set.

### Points drawn per tile

`k_max_marks` (500 by default) is the most points a tile draws, and `max_k` (1,000) the most a request may ask for. A tile reads its points' positions and rendered fields from the bundle, so a lower cap reads fewer pages for each view and shrinks the hot set. It doesn't change the memory each set of access attributes holds, and a filtered view still reads the identifier of every item that matches in view, whatever the cap.

Measured with the example above at a 2 GiB cap, with 100 users panning without pause:

| Points per tile | Requests per second | p99 latency of pans | Reads from disc in 20 s |
|---|---|---|---|
| 50 | 1,050 | 79 ms | 882 |
| 200 | 879 | 84 ms | 12,817 |
| 500 | 536 | 149 ms | 112,824 |

At 500 points a tile, that server had moved past the point where its hot set fit. A lower cap moves that point, though it couldn't rescue the same server at 1 GiB, five times smaller than the bundle, where filtered views dominated.

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
