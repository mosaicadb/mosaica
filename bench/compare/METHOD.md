# Method

This document says what the comparative benchmark measures and the rules that keep it fair. It
covers Mosaica and the systems it is compared with: ClickHouse (with DuckDB as a sub-arm),
Datashader with HoloViews or Panel, Tippecanoe with PMTiles, PostGIS with Martin, and OpenSearch.
`README.md` says how to run it and lists where the current stage departs from this method.

## The frame

Each system is asked for the same frames, and nothing compares until a frame is defined.

A frame is what one screen of a map needs from a server. The screen is 1920 by 1080 pixels at an
integer zoom, and a tile is 512 pixels across. The frame covers every tile at that zoom that the
screen touches, and holds, for each tile:

- the number of items matching the filter;
- a count grid: the matching items in each cell of a fixed grid, 256 by 256 cells a tile, so two
  pixels a cell;
- every matching item's position, where the tile holds at most K of them (K = 500). A tile with
  more is dense, and a system may send up to K positions from it, which are counted and not
  required.

Positions are compared on one grid. Each coordinate is quantised onto 2³² steps across a fixed
square extent by `floor((v − min) / (max − min) · 2³²)`, clamped to the grid, computed in IEEE
754 double precision. A tile or cell at depth d is the top d bits of each axis. Each system is
configured to produce the frame by its own means: a `GROUP BY` on the cell and a query for the
sparse tiles' positions in SQL systems, a grid aggregation in OpenSearch, a raster and a point
query in Datashader, and drop or cluster settings in Tippecanoe.

## The oracle

Every frame is checked against an exact answer computed by DuckDB over the same Parquet file the
systems are built from. The oracle shares no code with any system's adapter. Each result records,
beside its latency, the error of every frame: the absolute count error per tile and per grid cell,
the positions missing from or added to each sparse tile, and the positions sent from dense tiles.
A system that drops points at build time, as precomputed tiles do, has a non-zero error, and the
error is published with its speed.

## Workloads

A uniform random sweep of viewports mostly measures the input's distribution, because most random
screens are empty or nearly so. The workloads are fixed lists of screens, the same for every
system, derived from the tier's own density.

| Workload | Screens |
|---|---|
| Battery | At zooms 0, 4, 8, 12 and 16, one screen centred on the occupied tile at each density decile |
| Camera script | From the whole map into each of the two densest regions through zooms 3, 6, 9, 12 and 14, panning east by half a screen at 6, 9 and 14 and south at 14, then back out |
| Filter changes | At two screens of the script, a fixed sequence of filters: a categorical value, a numeric range and a region, each at about 50%, 5% and 0.1% of the items, and a combination |

The filters are the same for every system: categorical, numeric range, text, and polygon.

## Metrics

| Metric | How it is made like-for-like |
|---|---|
| Build or ingest time, peak memory | From the same Parquet file, every step to a servable state included: projection, sort, index, merge. Each build runs under the same memory cap, and its peak is read from its cgroup |
| Size on disk | Final size, and the peak of any scratch space |
| Cold frame latency, p50 and p99 | The server restarted, its own caches deleted and the page cache dropped. A cache that survives a restart, such as Mosaica's figure cache, is deleted, or reported separately as a reopen |
| Warm frame latency, p50 and p99, per zoom | Result caches off (ClickHouse's query cache, Martin's tile cache) for the compute figures; each system's default caching as a separate row |
| Latency of a filter change | The filter-change workload. Not applicable to precomputed tiles, which filter on the client |
| Concurrent viewers: 1, 10, 100 and 1,000 | An open-loop load generator, with distinct filters per viewer to defeat shared caches, and identical viewers reported as well |
| Insert to visible | Insert K points into an empty region and poll the frame until its count rises, with each system's commit, refresh or flush setting stated |
| Delete or suppress to gone | The same polling. Client and CDN caches are out of scope |
| Sustained ingest under read load | Rows a second while the battery runs, with the battery's p99 beside it |

Not built yet: concurrent viewers, insert to visible, delete to gone and sustained ingest. The
harness measures build, size, memory, cold and warm latency and filter changes.

Server time and client time are measured separately. Time to the last byte on the wire comes from
the load generator, pinned to two cores with the server on the rest. Decoding and drawing are
timed apart, in a headless browser, for a sample of frames, since vector tiles, Arrow and images
differ in client cost. Not built yet: the core pinning and the client timing; the harness times
each frame from the first byte sent to the last byte received, in Python, and decodes outside
that time.

## Fairness

- **Each system is tuned as its own documentation says, and the tuning is published.** ClickHouse:
  the sorting key, partitioning, codecs and `max_threads`. PostGIS: `CLUSTER` on the spatial index,
  `shared_buffers`, `work_mem`, parallel workers, and Martin's pool size. Tippecanoe: the drop and
  cluster flags, parallel reading from FlatGeobuf, and a zoom range matched to Mosaica's.
  OpenSearch: heap at half the memory up to 31 GB, the refresh interval and `_source`. Datashader:
  Dask partitions, categorical types, and spatially sorted Parquet. Each project's maintainers are
  asked to review its configuration before anything is published.
- **Caching is reported both ways.** Mosaica keeps a figure cache that survives restarts and lets
  drawing take priority over counting. Other systems have tile caches, query caches and the page
  cache. Compute figures are taken with every result cache off, and deployed figures with each
  system's defaults, side by side.
- **The hardware is the same for every system.** One machine, one run at a time, nothing else
  running, with stable cores. Swap is off or capped the same for every system, and memory caps are
  identical. A system that does not fit under the cap is reported as not fitting, not as slow.
- **The output is measured as well as the speed.** Count error against the oracle and the number
  of points per frame are published, since precomputed tiles are fast partly because they drop
  points, and Mosaica samples them.
- **Percentiles carry their sample.** p50 and p99 per zoom and per density decile, with n and the
  spread over repeated runs. A published figure is the median of at least five runs.
- **The load generator is not the bottleneck.** Its own ceiling is measured, and a figure near it
  is reported as a lower bound.
- **Licences are checked.** Each system's terms are checked for clauses against publishing
  benchmarks, and each dataset's licence is stated with the results.
- **What is published:** a method page with hardware, operating system, versions and image
  digests, configuration files, data checksums and this frame definition; one adapter per system;
  one script per tier that downloads, prepares, builds and runs; the raw JSONL records; and the
  notebook that draws the charts. The losses are shown as prominently as the wins.
