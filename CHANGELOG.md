# Changelog

All notable changes to Mosaica are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). One version covers the `mosaica` binary, the Rust
crates, the `mosaica` and `mosaica-native` Python packages, the `@mosaicajs` npm packages and the
container image.

## [Unreleased]

### Added

- **Items in a field's order.** `POST /v1/items` takes `order: {"field": <name>, "direction":
  "asc" | "desc"}` for a number, timestamp or bool field, with the items that have no value last
  and equal values in stored order. The TypeScript client takes it as `order`, the Python client
  as an `order` dict, and `mosaica items` as `--sort <field>` with `--desc`. Each response goes
  over every matching item once, so a sorted read of millions of matches is interactive and one
  of billions takes a pass for each response. An item whose value changes during a sorted read
  can be returned twice or not at all. A cursor from an earlier version is refused; start the
  read again.

## [0.4.1] - 2026-11-11

Faster browsing of nested annotation layers, a build report that says what it measures, histogram
and selection fixes in the components, and memory sizing that no longer mistakes page cache for
memory in use. Bundles and clients from 0.4.0 work unchanged.

### Added

- `<mosaica-selection>` takes `title-field` and names each listed mark by that field, read for
  the listed marks in one items request. The explorer passes its own `title-field`.

### Changed

- **Faster browse.** Expanding a row of a nested layer is answered from the neighbourhood of that
  row, not the whole layer. On a layer of about 300,000 artifacts, a children page takes under
  0.05 ms rather than about 470 ms at any depth, a filtered one 0.4 ms rather than about 500 ms,
  and the roots about 190 ms rather than 470 ms.
- **The build's occupancy report counts cells, not positions.** Each view's line now gives points
  per occupied cell, and the warning past ten per cell is `CELLS SHARED` in place of
  `RESOLUTION LOST`. Every position is still stored and served exactly; only the cell, the finest
  unit the map tiles and counts by, is shared. The count and the threshold are unchanged.
- The histogram card draws each bar as its share of its own set, on a scale the whole match sets.
  The scale stays put as the camera moves, and the bars in view keep their shape at any zoom
  rather than shrinking to a pixel when the view holds a small part of the match.
- Numbers in the components are grouped in thousands only from 10,000 up, so a year reads `2012`
  in the item card, the histogram's axis and the filter chips.
- The histogram's count and mean are given to a screen reader only.

### Fixed

- Mosaica counts a cgroup's inactive file pages as available memory. Once the page cache filled
  a container's memory limit, writes could be refused as `insufficient_memory` with a few
  kilobytes available, and the build's default budget and the server's writer sizing shrank to
  match.
- A write submitted while the write executor is stopping is refused with `503` as not ready,
  rather than with the `500` that says it may have taken effect.
- Stepping into a cluster on a layer's card keeps the card's height until the children arrive, so
  a scrolled filter panel keeps its place.
- A callout follows the size of the card it holds, so Show all grows it to fit with no scroll bar
  left.

### Documentation

- The [own-UI tutorial](https://docs.mosaicadb.com/start/typescript-own-ui/) sets a cluster
  spacing, so its world view names the broad topics.

## [0.4.0] - 2026-11-11

Annotation layers you can step through, components usable from the keyboard and by a screen
reader, selections that survive a switch of layout, `mosaica demo`, and faster text indexing and
compaction.

### Breaking

- **Rebuild every bundle.** The bundle format is now 54. Writes made to a 0.3.0 service,
  including deletions and suppressions, are not carried over.
- **Cluster budget is now cluster spacing**, given in CSS pixels per cluster in place of a count:
  `cluster-budget`, `cluster-budget-min` and `cluster-budget-max` become `cluster-spacing`,
  `cluster-spacing-min` and `cluster-spacing-max`; `setClusterBudget` and `clusterBudget` become
  `setClusterSpacing` and `clusterSpacing`; `mosaica-clusterbudgetchange` becomes
  `mosaica-clusterspacingchange`; React's `onClusterBudgetChange` becomes
  `onClusterSpacingChange`; and the Python widget's `cluster_budget*` traits become
  `cluster_spacing*`.
- **A level is chosen per layer.** The `cluster-level` attribute and the deck layer's
  `clusterLevel` prop are replaced by `clusterLevels`, a `Map` from layer to level, and
  `mosaica-levelchange` reports `{layer, level}`.
- `POST /v1/artifacts/browse` refuses `level` together with `parent` (`422`), and refuses a
  cursor from 0.3.0.
- Python: `Selection.within` on a view with a projection takes its box in degrees.
- Component markup: the `most-clusters` parts are now `cluster-density`, the field card's `path`
  part is gone, titles are headings and name–value pairs are `<dl>` lists.

### Added

- **Selections across layouts.** A `region` filter can name the `view` it was drawn in, so a
  selection keeps filtering after switching layout. The filter panel shows each held selection
  as a chip.
- **`mosaica demo NAME`** builds a sample dataset and opens it in the browser
  (`pip install 'mosaica[demo]'`). [Datasets to try](https://docs.mosaicadb.com/start/datasets/)
  lists them.
- **Accessibility.** The components work from the keyboard and announce changes to a screen
  reader. New: `aria-label` on the explorer (`label` on the Python `Map`), a `quiet` attribute,
  typed ranges on histogram cards, and the `--mosaica-focus` and `--mosaica-long-text` tokens.
  See [Accessibility](https://docs.mosaicadb.com/system/accessibility/).
- Boxes take `space`: `"wgs84"` for degrees or `"inserted"` for the coordinates items were
  inserted with, across the viewport, artifacts and aggregate routes and the clients.
- `POST /v1/items` returns every field when `fields` is omitted, in every client.
- Bool fields can be binned and summarised, and get a yes/no field card.
- TypeScript: `status.querying` and `store.cancelQuery()`, with Running query and Cancel in
  `<mosaica-status>`; `AggregateSpec.filters` gives an aggregate its own filter.
- Python: `views()`, `attributes()` and `layers()` on `Database` and `Viewer`.
- The item card has Show more for long values.

### Changed

- **Levelled layers follow the zoom.** A `stacked` or `tiered` layer with no chosen level is
  drawn at the finest level that is not crowded in view; Follow zoom returns a layer to this.
- **The cluster card steps through a hierarchy** a page at a time, with a breadcrumb, without
  changing the map's filter.
- **Withheld annotations' children** attach to their nearest served ancestor in browse, bulk reads
  and map frames, where they used to appear as roots.
- **Faster text indexing.** At a million items, a build's text stage takes 12.8 s rather than
  32.1 s and writes a third of the bytes. Compaction is 20% faster but peaks at 760 MiB rather
  than 377 MiB.
- **Ingest during compaction.** At a million items, writes are acknowledged within 0.35 s while
  a compaction publishes, where they took about 2 s.
- **Faster filtering and visibility.** Rare category values are answered from their postings,
  the filter cache keeps rows across pans, and computing a viewer's visible set uses every free
  core (0.96 s rather than 5.1 s at 10⁹ items on four cores).
- `serve.filter_cache_bytes` defaults to 1 GiB, up from 256 MiB.
- A request after a flush or merge is answered rather than refused with `429`.
- The cluster count follows the map's size, and the explorer's Most clusters slider is now
  Cluster density.
- Unnamed clusters are shown by their key.
- The documentation moved to <https://docs.mosaicadb.com/>, and a hosted demo runs at
  <https://demo.mosaicadb.com/>.
- Component polish: clearer focus order and Escape handling, contrast of at least 4.5:1 for
  faint text, 24 px press targets, no state shown by colour alone, and a better narrow layout.

### Fixed

- A `stacked` layer drawn at one level no longer shows coarser levels' labels over it.
- TypeScript requests combining several filters no longer exceed the server's nesting limit.
- The server stops work on column, keyword and aggregate requests when the client goes away.
- The TypeScript store retries artifact requests refused with `429` or `503`.
- `+` and `-` zoom one level after a click on the map, not two.
- The notebook map keeps key presses from triggering notebook shortcuts.
- Focus no longer jumps when the control holding it is removed, and the hierarchy keeps its open
  rows when a filter changes.
- Mosaica reads a cgroup v1 memory cap, where it used to size itself from the host's memory.
- Files consumed by a merge are deleted once unreferenced, rather than at the next compaction.
- The arXiv sample names every archive and subject class in its `taxonomy/arxiv` layer.

### Documentation

- [Count and chart from Python](https://docs.mosaicadb.com/guides/analysis/), with reference
  pages for [filters](https://docs.mosaicadb.com/reference/filters/) and
  [aggregations](https://docs.mosaicadb.com/reference/aggregations/).
- [Sizing](https://docs.mosaicadb.com/guides/sizing/) covers the hot set, access-attribute cost
  and cache limits under a small memory cap.

## [0.3.0] - 2026-11-08

### Changed

- **Breaking:** bundles built by 0.2.0 must be rebuilt, because the bundle format is now 52.
  Writes made to a 0.2.0 service, including deletions and suppressions, are not carried over.
  ([#223](https://github.com/mosaicadb/mosaica/pull/223))
- **Breaking:** `serve.text_filter_cache_bytes` is renamed `serve.filter_cache_bytes`, and a
  deployment file that uses the old name is refused. The status field `text_filter_cache` is
  renamed `filter_cache`. ([#217](https://github.com/mosaicadb/mosaica/pull/217))
- **Breaking:** compaction request numbers start from the server's start time in milliseconds
  instead of from 1, so a number is not reused after a restart. ([#219](https://github.com/mosaicadb/mosaica/pull/219))
- **Breaking:** when a deployment names no session or control address, those planes are served on
  the viewer's port. An open server used to put them one and two ports above it. ([#215](https://github.com/mosaicadb/mosaica/pull/215))
- The filter cache also keeps range, `eq` and `in` conditions on columns, so panning and zooming
  with a column filter does not evaluate it again. ([#217](https://github.com/mosaicadb/mosaica/pull/217))
- The Python client serves on port 9141 by default, or the next free port above it, and prints
  its address. `create()` and `open()` take `port=`. The `mosaica` package now depends on `tqdm`.
  ([#215](https://github.com/mosaicadb/mosaica/pull/215))
- Keyword and category filters ([#223](https://github.com/mosaicadb/mosaica/pull/223)), column filters ([#217](https://github.com/mosaicadb/mosaica/pull/217)), histograms ([#221](https://github.com/mosaicadb/mosaica/pull/221)), builds
  ([#214](https://github.com/mosaicadb/mosaica/pull/214)) and server start-up ([#216](https://github.com/mosaicadb/mosaica/pull/216)) are faster.

### Added

- `GET /control/compact/{request}` reports whether a compaction request has been answered, how it
  ended and how far its compaction has got. `mosaica compact` takes `--wait` and `--request`, and
  the Python and TypeScript clients can wait for a request or read one. ([#219](https://github.com/mosaicadb/mosaica/pull/219))
- Builds, inserts and compactions report their progress. The CLI shows it with `--progress`,
  `/control/status` lists running jobs under `jobs`, and the clients pass each record to a
  callback. ([#219](https://github.com/mosaicadb/mosaica/pull/219))
- A filter can name items by `mosaica_id`, and the TypeScript client builds one with
  `namedItems(ids)`. ([#220](https://github.com/mosaicadb/mosaica/pull/220))
- The item card draws the fields listed in `image-fields` as images. ([#220](https://github.com/mosaicadb/mosaica/pull/220))
- `[serve] port_retries` makes `mosaica serve` try that many ports above a taken one. ([#215](https://github.com/mosaicadb/mosaica/pull/215))

### Removed

- **Breaking:** `compaction.requests.last_refusal` in `/control/status`, which
  `GET /control/compact/{request}` replaces. ([#219](https://github.com/mosaicadb/mosaica/pull/219))

### Documentation

- A guide to reaching a notebook's map when Jupyter or marimo runs on another machine. ([#218](https://github.com/mosaicadb/mosaica/pull/218))

## [0.2.0] - 2026-11-05

### Changed

- **Breaking:** bundles built by 0.1.0 must be rebuilt, because the bundle format is now 50.
  Writes made to a 0.1.0 service, including deletions and suppressions, are not carried over.
- **Breaking:** the write-ahead log format is now 44, so a 0.1.0 log is refused.
- Phrase search reads word positions from the index instead of each item's text. Bundles with
  text fields are larger and take longer to build as a result.
- The results of `match` and `phrase` filters are kept between requests for every viewer with the
  same access, up to `serve.text_filter_cache_bytes` (256 MiB by default).
- A new viewer's first frames are faster, and so are filtered maps and aggregates.
- An aggregate returns its first page even when its stream deadline has already passed.
- A request stops evaluating its filter when its client disconnects, and so does
  `POST /v1/artifacts/browse`.

### Security

- A server could replay a write-ahead log written for another database, such as the log of a
  bundle that had since been rebuilt. That could expose a deleted or suppressed item and hide an
  unrelated one. Each log now records its database, and a log for another database is refused.

### Fixed

- Some JSON numbers with seventeen significant digits were read one step away from their true
  value, which could drop an item lying exactly on the edge of a box filter.
- A `phrase` filter over a group-scoped text field matched nothing.
- Inserting the same item in two batches could leave a group-scoped field with a different value
  from each, where the second batch should have counted as a repeat or an edit.
- Building a text field with very many distinct words used about 0.8 GB per 100 million words
  outside the build's memory budget.

### Documentation

- A documentation website at <https://mosaicadb.github.io/mosaica/>, published from each release
  tag.
- A security policy in SECURITY.md, covering the latest release.
- A comparison with ClickHouse, PostgreSQL and OpenSearch, timing a per-viewer map of GBIF species
  records at 10 million to a billion records.

## [0.1.0] - 2026-10-27

The first public release, an alpha. Formats, the HTTP API and the client libraries may change in
any later release, and a bundle built by one version may be refused by the next.

### Added

- **Interactive maps over very large corpora from one machine.** Mosaica builds a bundle from
  Parquet files, or from data frames in Python, and serves a pannable, zoomable map over it.
  Geometry is stored in Morton order, so a map tile at any zoom is one contiguous range of rows,
  and the server answers a viewport with a sample of points, counts and densities without scanning
  the tile. The build streams within a memory budget, and the design target is billions of points on
  one machine.
- **Any records with a 2D layout**: geographic coordinates, or an embedding projection such as
  t-SNE or UMAP. One corpus can carry several layouts that share item identity, so a viewer can
  switch between them and keep their selection.
- **Filters and search** over categories, numbers, dates, keywords and full text, and over drawn
  regions (box, circle, ellipse and polygon), composable with one another. Highlight mode lights
  the matches and dims the rest.
- **Annotation layers**: clusters, hierarchies including DAGs, regions and hulls, with labels.
- **Item cards** for a selected point, and **bulk reads** of the items and annotations a viewer may
  see, page by page as Apache Arrow tables.
- **Live ingest into a running service.** Insert, delete and suppress items while the server is
  serving; a deletion or suppression applies to every request that starts after it is accepted. A
  write-ahead log makes every accepted write survive a restart, and anything that can be declared
  at a build can be declared at a running service.
- **Per-viewer access control over the whole map.** Each item carries an access label, an
  expression over terms. A viewer's visible set is computed once per session, and every count,
  density, cluster, label and sample the viewer receives is computed from that set alone, not only
  which items they can open. Viewers authenticate with a password, an API key or an OIDC access
  token, or an integrator's backend authorises a session for them.
- **Four ways in:**
  - the HTTP API, described in OpenAPI, which reaches every capability: declare, insert, delete
    and suppress, query, filter and annotate;
  - the Python package `mosaica`, which declares, inserts from data frames, removes, suppresses,
    publishes annotation layers, and queries, filters and counts with pyarrow results, with a
    notebook map widget (`mosaica[widget]`) and the `mosaica-native` platform wheel that carries the
    binary;
  - the TypeScript packages: `@mosaicajs/client`, whose headless store queries and filters the map
    and whose control-plane client declares, ingests, deletes and suppresses, and publishes
    annotation layers; `@mosaicajs/deck` (a deck.gl layer); `@mosaicajs/components` (web
    components and a single-file bundle); and `@mosaicajs/react` (hooks and React wrappers);
  - the `mosaica` command line, which builds a bundle from a declaration and its source files,
    checks, verifies and serves it; on a running server, declares, inserts from Parquet, Arrow
    IPC or NDJSON, deletes, suppresses and lifts suppressions, publishes and grows annotation
    layers, drops a layer or a view of a view group, brings a publication forward, asks for a
    compaction and reports status (`declare`, `insert`, `delete`, `suppress`, `unsuppress`,
    `publish`, `grow`, `drop`, `flush`, `compact` and `status`); reads items, annotations and
    aggregates in bulk; and manages principals, groups, API keys, grants, sessions and OIDC
    providers.
- **Distribution**: release binaries for Linux (glibc and musl, x86_64 and aarch64), macOS (arm64
  and x86_64); Python wheels on PyPI; npm packages; a multi-architecture
  container image on GHCR; and the Rust crates on crates.io, which are internal to the binary and
  carry no stable API.

[0.4.1]: https://github.com/mosaicadb/mosaica/releases/tag/v0.4.1
[0.4.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.4.0
[0.3.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.3.0
[0.2.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.2.0
[0.1.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.1.0
