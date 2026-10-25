# Size a deployment

How much memory and disc Mosaica needs depends on how many items you have and on what your schema does with their fields. The most reliable estimate comes from building a sample of your own data, so this page explains what uses each resource and then how to build that sample.

## Memory

A running server needs memory for two things: its caches, which have fixed limits, and the bundle's files, which use whatever is left.

With the default limits, the caches and the paged reads add up to about 7.7 GiB. [Memory](operating.md#memory) lists each cache, what it holds and the setting that bounds it. You can lower them for a small database or raise them when many users each hold their own access attributes.

The bundle isn't loaded into memory. Mosaica maps its files, so the kernel keeps the parts people are looking at in memory and drops the rest when memory runs short. Set the server's memory cap above the cache limits, and every gigabyte beyond that keeps more of the bundle in memory. A cap that's too low makes the server slower before it makes it fail, so if the whole bundle fits in memory alongside the caches, that's the fastest a server gets.

A build sizes itself separately. `mosaica build --memory-budget 24g` keeps the build's own structures within 24 GiB. Without it, the build takes 80% of the memory available when it starts, so set one when a server runs on the same machine.

## Disc

Plan disc space for four things:

- **The bundle.** Deleted and edited rows stay in it until a compaction, which by default starts once they're a fifth of all rows. Merging segments also leaves files behind, and a compaction starts once those take as much space as the files the bundle uses. So a bundle can reach about twice its live size before a compaction reclaims it, and a little more in the day after one, since compactions are at least a day apart.
- **Room for a compaction**, which needs free space of one and a half times the size of the files the bundle uses. Without it the compaction is refused. In a bundle of several shards, each shard is compacted on its own, so the space needed is that of the largest.
- **The cache directory**, which holds work the server can redo. The saved counts are bounded by `figures_disk_bytes`, 8 GiB by default. The copies of each distinct set of granted labels' visible items aren't bounded: they grow with the number of such sets, and with each flush, until the next compaction removes them.
- **The write-ahead log**, which holds every change since the last flush, so it usually stays small. An annotation level that has grown or been filled holds the log until the next compaction, and `wal_pin` in `/control/status` says what is holding it.

Taken together, a database that's written to and compacted on the default schedule needs about three and a half times its live bundle size, plus the cache. A database you only build and read needs the bundle and the cache.

Keep the bundle and the log on a local disc. A fast SSD matters most when the bundle doesn't fit in memory, since every read the page cache misses goes to disc.

## CPU

Requests run on a pool of `compute_threads`, which defaults to the number of CPUs the process may use, so more cores let more requests run at once. Lower it to leave some for other work on the same machine.

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

While a server runs, `/control/status` shows what it holds: `heap.anon_bytes` is the caches and the allocator's own memory, and `heap.file_bytes` is how much of the bundle is in memory.
