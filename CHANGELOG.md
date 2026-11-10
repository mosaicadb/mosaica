# Changelog

All notable changes to Mosaica are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). One version covers the `mosaica` binary, the Rust
crates, the `mosaica` and `mosaica-native` Python packages, the `@mosaicajs` npm packages and the
container image.

## [Unreleased]

### Changed

- **A filtered map answers faster.** Testing each point of the map against a filter's matches
  finds the matching item's block directly rather than searching for it, and starts reading each
  block before it is needed: about nine times faster where the matches are many.
- **A viewer's first map and a database's open are faster on a machine with cores to spare.**
  Working out which points a viewer may see now runs on every core the server has free and does
  less work on one: at 10⁹ items, measured on four cores, 0.96 s rather than 5.1 s for a viewer
  who sees a quarter of them, and 3.0 s on one core. Opening a database checks each map's points
  in 2.8 s rather than 19 s on four cores.
- **A filter is kept crossed onto the map.** Where a filter's matches are few beside the map's
  points, the points they fall on are kept for every viewer with the same access, so panning and
  zooming with the filter does not work them out again. Deletions and suppressions still apply to
  the next request. `serve.filter_rows_cache_bytes` bounds it (256 MiB by default, `0` keeps
  none), and `GET /control/status` reports it as `filter_rows_cache`.
- **A viewer is no longer refused with 429 after a flush or merge.** While the server brings each
  viewer's visible set up to date after a flush or merge, a viewer it has not reached yet used to
  be refused until it had. Now that request brings its own visible set up to date and is answered.
  `refresh_in_flight` in `GET /control/status` still reports the work in progress.

## [0.2.0] - 2026-11-05

An alpha, like 0.1.0. The bundle format has changed, so 0.2.0 refuses a bundle that 0.1.0 built,
and each one has to be built again. The HTTP API keeps its version, and the Python and TypeScript
clients behave as they did in 0.1.0.

### Changed

- **Bundles from 0.1.0 must be rebuilt, and their logs set aside.** The bundle format is now 50,
  and 0.2.0 refuses to open a bundle of format 48, which 0.1.0 writes, with a message that says to
  rebuild it. Build it again from its declaration and source files with `mosaica build`, or from
  data frames in Python. A rebuild starts from those files alone, so every write made to the
  running 0.1.0 service is left behind: inserts, edits, deletions, suppressions and declarations.
  Make them again on the rebuilt database; until a deletion or suppression is made again, the item
  it named is served. 0.2.0 also refuses 0.1.0's write-ahead log, so move its files (`wal-*.log`
  and `wal-*.sync`, where `[bundle] wal` points) aside, or set `[bundle] wal` to a new path.
- **Phrase search reads word positions from the index.** A text field's index now records where
  each word falls in each item, and a `phrase` filter or highlight is answered from those positions
  instead of by reading the text of every item that holds all its words. The positions add to a
  bundle's size on disc and to a build's work.
- **A search is kept between requests.** The items each `match` and `phrase` filter matches are
  kept for every viewer with the same access, so panning and zooming with a search does not run it
  again. Deletions and suppressions still apply to the next request, and the kept answer is dropped
  when the indexed text changes at a flush, coalesce or compaction. `serve.text_filter_cache_bytes`
  bounds it (256 MiB by default, `0` keeps none), and `GET /control/status` reports it as
  `text_filter_cache`. The status's `region_cache` gains `waiters_now` and `oversized_admissions`.
  When several requests start the same search at once, one runs it and the others wait for its
  answer. A request that has waited `serve.single_flight_wait_ms` runs the search itself, where a
  wait that long for any other shared cache is refused with 429.
- **Faster first frames for a new viewer.** The count of occupied tiles that sets how many points a
  tile draws is now shared by every viewer with the same access and kept on disc, so a second
  session and a restart reuse it. At deep zooms over large visible sets it is estimated from a
  sample of the viewer's own tiles, which can move a tile's number of drawn points slightly.
- **Faster filtered maps and aggregates.** A dense tile under a filter is drawn from the stored
  sample bands where that reads less than a scan, and `/v1/aggregate` evaluates a filter bounded by
  a drawn region only within the region's tiles.
- **An aggregate always returns its first page.** A `/v1/aggregate` response whose stream deadline
  passed before its first page used to end with a trailer alone, and now delivers that page first,
  as a records read does.
- **Filters stop when the client goes away.** A request whose client disconnects stops evaluating
  its filter, and `POST /v1/artifacts/browse` is now cancelled the same way.

### Fixed

- **A server refuses a write-ahead log written for another database.** Each log now records the
  database it belongs to, and a server refuses to replay one written for any other, such as the log
  of a bundle that has since been rebuilt, with a message that says to move it aside. Before, the
  server replayed the old log's writes onto the new bundle by internal id, so a deleted or
  suppressed item could reappear and an unrelated one be hidden. The log format is now 44.
- **JSON numbers are read as the correctly rounded double.** 0.1.0 could read some numbers with
  seventeen significant digits one step away from their true value, which could move a box's edge
  past an item lying on it and drop that item from the answer.
- A `phrase` filter over a group-scoped text field matched nothing, and now matches as it does over
  any other text field.
- Two batches that added the same item through two views of one key could each keep their own
  value for a group-scoped field. The second batch is now checked again, so an equal value counts
  as a repeat and a different one as an edit.
- Building a text field with very many distinct words held one 8-byte offset per word in memory
  outside the build's memory budget, about 0.8 GB at 100 million words. The offsets are now written
  to a scratch file, and so are those of the word positions this release adds, which would have
  doubled the figure.

### Documentation

- **A documentation website** at <https://mosaicadb.github.io/mosaica/>, with search, published
  from each release tag so that it describes the release.
- **Security policy.** SECURITY.md now supports the latest release, and a security fix ships in
  the next release.
- **Compared with other databases**, a page and a README section that time one per-viewer map of
  GBIF species records served by Mosaica, ClickHouse, PostgreSQL and OpenSearch, each enforcing the
  viewer's access with its own access control, from 10 million to a billion records.

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

[0.2.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.2.0
[0.1.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.1.0
