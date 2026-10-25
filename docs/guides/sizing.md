# Size and tune a deployment

How much memory and disc Mosaica needs depends on how many items you have and on what your schema does with their fields. The most reliable estimate comes from building a sample of your own data, so this page explains what uses each resource, how to build that sample, and which limits to change once the server runs.

## Memory

A running server needs memory for two things: its caches, which have fixed limits, and the bundle's files, which use whatever is left.

With the default limits, the caches and the paged reads add up to about 7.7 GiB. You can lower them for a small database, or raise them when many users each hold their own access attributes. [The caches](#the-caches) lists each one.

The bundle isn't loaded into memory. Mosaica maps its files, so the kernel keeps the parts people are looking at in memory and drops the rest when memory runs short. Set the server's memory cap (`MemoryMax` under systemd, or the container's memory limit) above the cache limits, and every gigabyte beyond that keeps more of the bundle in memory. A cap that's too low makes the server slower before it makes it fail, so if the whole bundle fits in memory alongside the caches, that's the fastest a server gets.

`mosaica status` shows what the process holds. `heap.anon_bytes` is the caches and the allocator's own memory, and `heap.file_bytes` is how much of the bundle is in memory:

```console
mosaica$ mosaica status --control unix:/run/mosaica/control.sock | jq '.heap | {anon_bytes, file_bytes}'
{
  "anon_bytes": 6320128,
  "file_bytes": 19673088
}
```

A build sizes itself separately. `mosaica build --memory-budget 24g` keeps the build's own structures within 24 GiB. Without it, the build takes 80% of the memory available when it starts, so set one when a server runs on the same machine.

### The caches

Each cache has a limit you can set under `[serve]` in `mosaica.toml`:

| Key | Default | What it holds |
|---|---|---|
| `row_projection_cache_bytes` | 2 GiB | For each set of access attributes, view and shard, which rows that set may see. Shared by every session holding the same attributes |
| `fragment_cache_bytes` | 1 GiB | For each distinct set of access attributes, the items it admits. Copies on disc, in the cache directory, outlive the memory |
| `masked_count_cache_bytes` | 1 GiB | The counts, centroids and boxes of annotation layers as each set of attributes sees them, and the tallies of number and date fields |
| `figures_disk_bytes` | 8 GiB | The same counts, kept on disc in the cache directory so that a restart reads them back |
| `region_cache_bytes` | 256 MiB | Drawn filter regions broken into map tiles, shared between viewers |
| `occupancy_cache_bytes` | 32 MiB | How many tiles at each zoom hold something a session can see |

Beside these, the server keeps what it subtracts and adds on each request for deleted, suppressed and newly ingested items, under fixed limits that come to 1.5 GiB. The paged reads, `POST /v1/items` and `POST /v1/artifacts`, share one more: two may run at once (`bulk_admission`), each holding up to seven pages of at most 64 MiB (`max_page_bytes`). The server logs what that comes to when it starts, as `bulk_read_memory_bytes=939524096`.

??? info "Sizing the annotation count caches"
    `masked_count_cache_bytes` bounds two caches, each to that size: the annotation counts, and the tallies of number and timestamp fields, which are under a kilobyte each.

    The annotation counts hold one entry for each set of access attributes and each annotation level served from a column, whatever the number of shards. Per artifact, an entry takes 4 B for counts alone and 40 B with a centroid and a box. A layer that serves a box also keeps the eight most extreme rows on each side, which adds 4 B, plus 128 B for each artifact with more than sixteen placed rows, or 256 B in a bundle of several shards.

    So a level of 1.4 million artifacts takes 56 MB for each set of attributes counted with geometry, and a layer serving a box keeps at most 185 MB more in the same entry, or 364 MB over several shards. The default then holds about nineteen such entries, and at least four with their extra rows, or two over several shards. A deployment where every user holds their own attributes needs one entry per user per level.

    An entry larger than the limit is served and not kept, and one that fits only without its extra rows is kept without them. `masked_count_cache.not_admitted` in `/control/status` counts both.

    On disc, `figures_disk_bytes` keeps the same entries and the labels of denied rows they're corrected with. In a bundle of several shards it also keeps each shard's pass over each set's items, which a compaction of one shard reads back so as to redo that shard alone: up to 172 B for each artifact a shard holds a member of, for each version of the level. Size it as the memory limit times the number of entries a restart should find, plus those passes, plus about 24 B per denied row per level. Past it, the files least recently written or read are removed.

## Annotation layers and new viewers

Annotation layers have one cost to plan for: the first time someone with a new set of access attributes opens a layer, the server counts its artifacts for them. Every viewer holding the same set shares the result, so a deployment whose viewers fall into a few groups pays it a few times for each level, while one where every user holds attributes of their own, such as a label per document, pays it for each user and each level they open.

The count takes longer the more of the database the attributes admit, and a request that shows several levels for the first time counts them one after another. Every later request reads the result, corrected for whatever has been deleted, suppressed or ingested since. The result is written to the cache directory, so a restart reads it back. After a compaction, each set of attributes counts the compacted shard again the next time it reads each level, and no other shard.

`masked_count_cache_bytes` decides how many results stay in memory and `figures_disk_bytes` how many stay on disc. `/control/status` counts the passes under `masked_count_cache.fills`, and results read back from disc under `masked_count_cache.loads`.

??? info "Which levels this applies to"
    The cost falls on annotation levels served from a column, which holds one entry per row naming its artifacts. Unless the layer declares `layout = "rows"`, that is every level of a `flat`, `stacked` or `tiered` layer whose memberships are enumerated, every attribute layer's level, and a `nested`, `dag` or shape level of at least 1,000 artifacts of which a quarter are spread too widely for the map's index.

    Drawing a layer or listing its artifacts reads its levels. Colouring points by a layer reads them when the map then reads the artifacts its points are tagged with.

??? info "Colouring by a layer"
    A viewer's first request for a layer's colours, with a `palette_size`, also costs one pass over the layer: the server works out a colour slot for every cluster the viewer is served, one level at a time. That's the level asked for and, on a `tiered` layer, the levels above it, never a deeper one. So a viewer whose first view of a `tiered` taxonomy is at genus or species also counts every coarser level. Where the layer declares a centroid, as a taxonomy does, the pass reads the centroids the map fills when it draws the level, and adds no pass of its own. Browse and the bulk reads fill the same counts when they name a `palette_size`, and nothing for colours without one.

    The slots are kept for that credential, layer and palette size, shared by every session holding the credential. After a deletion, a suppression or a lifted one, an edit, a publication, or a change to the layer's registration or edges, the next request that colours the layer pays the pass again before it is answered, and so does every change on a layer whose criterion is a fraction or whose content has a generating set. After an ingest, a flush or a growth on any other layer, the next request is answered from the slots held, and the server works them out again once that response is sent, on at most two sessions at once.

## Disc

Plan disc space for four things:

- **The bundle.** Deleted and edited rows stay in it until a compaction, which by default starts once they're a fifth of all rows. Merging segments also leaves files behind, and a compaction starts once those take as much space as the files the bundle uses. So a bundle can reach about twice its live size before a compaction reclaims it, and a little more in the day after one, since compactions are at least a day apart.
- **Room for a compaction**, which needs free space of one and a half times the size of the files the bundle uses. Without it the compaction is refused. In a bundle of several shards, each shard is compacted on its own, so the space needed is that of the largest.
- **The cache directory**, which holds work the server can redo. The saved counts are bounded by `figures_disk_bytes`, 8 GiB by default. The copies of each distinct set of access attributes' visible items aren't bounded: they grow with the number of such sets, and with each flush, until the next compaction removes them.
- **The write-ahead log**, which holds every change since the last flush, so it usually stays small. An annotation level that has grown or been filled holds the log until the next compaction, and `wal_pin` in `/control/status` says what is holding it.

Taken together, a database that's written to and compacted on the default schedule needs about three and a half times its live bundle size, plus the cache. A database you only build and read needs the bundle and the cache. [Schedule compactions](compaction.md) explains when compactions run.

Keep the bundle and the log on a local disc. A fast SSD matters most when the bundle doesn't fit in memory, since every read the page cache misses goes to disc.

## CPU

Requests run on a pool of `compute_threads`, which defaults to the number of CPUs the process may use, so more cores let more requests run at once. Lower it to leave some for other work on the same machine.

The annotations a map draws come from `POST /v1/artifacts/viewport`, which takes one request per compute thread at a time by default (`artifact_admission`). A request past that, and past as many waiting, is answered `429`, which `/control/status` counts under `artifacts.shed_total`. `max_artifacts_per_tile`, 1,000 by default, is the most artifacts one request may ask for in each tile and level.

## Measuring your own data

Bundle size per item varies widely with the schema: a rendered field travels with every point, an indexed text field adds a search index, and every view and annotation layer adds its own files. The bundles built in these docs show the range:

| Example | Items | Schema | Bundle | Per item |
|---|---|---|---|---|
| [Ireland](../start/first-map.md) | 29,935 | One geographic view; an indexed text field, a rendered category and a rendered number | 2.2 MB | 73 B |
| [GeoNames](../start/install.md#try-it) | 150,000 | One geographic view; an indexed text field and a rendered number | 11.7 MB | 78 B |
| [arXiv](backend.md) | 50,000 | Two views; six fields, including indexed text and an indexed keyword; two clusterings, one with a layer of names; 175 access attributes | 15.0 MB | 300 B |

To estimate your own, build a sample of your data with your real schema, read the size from the line that starts `built`, and scale it by the number of items you expect. A sample keeps per-item costs right, but fixed costs and the size of annotation layers don't scale the same way, so treat the result as a starting point and check it against a larger build before you buy hardware.

```bash
mosaica build 2>&1 | grep '^built'
```

```
built /home/you/places/bundle (v00000): 150000 items, 1 terms, 150000 pairs, 11684262 bytes on disk, 0 artifact(s) minted, 0 unclustered member row(s), 0 row(s) refused
```
