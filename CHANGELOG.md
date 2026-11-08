# Changelog

All notable changes to Mosaica are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). One version covers the `mosaica` binary, the Rust
crates, the `mosaica` and `mosaica-native` Python packages, the `@mosaicajs` npm packages and the
container image.

## [Unreleased]

## [0.3.0] - 2026-11-08

An alpha, like 0.2.0. The bundle format has changed again, so 0.3.0 refuses a bundle that 0.2.0
built, and each one has to be built again. A setting and a status field are renamed, the
control plane's compaction requests are numbered and read differently, and a deployment that
named no session or control address now serves them on the viewer's port.

### Changed

- **Bundles from 0.2.0 must be rebuilt, and their logs set aside.** The bundle format is now 52,
  and 0.3.0 refuses to open a bundle of format 50, which 0.2.0 writes, with a message that says to
  rebuild it. Build it again from its declaration and source files with `mosaica build`, or from
  data frames in Python. A rebuild starts from those files alone, so every write made to the
  running 0.2.0 service is left behind: make inserts, edits, deletions, suppressions and
  declarations again on the rebuilt database; until a deletion or suppression is made again, the
  item it named is served. The rebuilt database refuses the old write-ahead log, so move its files
  (`wal-*.log` and `wal-*.sync`, where `[bundle] wal` points) aside, or set `[bundle] wal` to a new
  path.
- **Keyword and category values are stored per block.** A keyword field's ordinals and a
  category's codes are kept per block of 2¹⁶ items in the narrowest width that holds them, so a
  field with few values in a region reads less, and an `eq` or `in` on one is answered from the
  block's codes.
- **The filter cache keeps every condition, and is renamed.** `serve.text_filter_cache_bytes` is
  now `serve.filter_cache_bytes`, and `/control/status`'s `text_filter_cache` is now
  `filter_cache`. Besides `match` and `phrase`, it keeps what a range, an `eq` or an `in` on a
  column matches, each condition apart, so a combined filter reuses its parts. Rename the setting
  in a deployment file; the old name is refused.
- **A compaction request is read by its number.** `POST /control/compact` numbers its requests
  from the time in milliseconds at which the server started, so a number from before a restart is
  never given again, and the new `GET /control/compact/{request}` says whether a request is
  answered, how (`compacted`, `nothing_to_do`, `refused` or `failed`) and how far its compaction
  has gone. The server keeps the latest 1,024 answers; an older one answers `410`, and a number
  from before a restart `404`. `/control/status`'s `compaction.requests` no longer carries
  `last_refusal`; read the request instead.
- **A viewport's filter is evaluated over its tiles' items** where that reads less than a scan of
  the column, and a column filter's scan runs in parallel. Sparse columns are stored packed.
- **Faster histograms.** `/v1/aggregate` bins over a filtered set read only that set's rows, and
  bins over an integer field compare one key.
- **Faster builds and starts.** A build hashes or sorts keyword chunks and merges keys alone, in
  parallel. A server opening a bundle checks every file's size and each segment's permutation, and
  hashes only the few files it reads without checking their structure, so a start no longer reads
  the whole bundle. `mosaica verify` still hashes every file.
- **The three planes share one port unless given their own.** `session` and `control` under
  `[serve]` are now optional, and a plane with no address of its own is served on the viewer's
  address and port, each route keeping its own credential and CORS rules. A deployment that names
  all three addresses is served as before. An open server no longer puts the session and control
  planes one and two ports above the viewer; they share its port unless named.
- **The Python client serves on port 9141 and prints the address.** A database made with
  `mosaica.create()` is served on 9141, or the next free port above it, on this machine only, and
  starting its server prints the address, so a notebook kernel on another machine can be reached
  by forwarding one port. `create()` and `open()` take `port=` to state another; a stated port
  that is taken is refused. The Python package now depends on `tqdm`.

### Added

- **Progress for builds, inserts, flushes and compactions.** `mosaica build`, `mosaica insert`
  and `mosaica compact --wait` take `--progress` (`auto`, `bar`, `plain`, `json` or `none`) and
  show the phase, the step, its count, rate and time remaining, the time elapsed and resident
  memory on stderr. `/control/status` reports the jobs running as `jobs`, and
  `compaction.last_job`. In Python, `commit()` and `compact()` take `progress=`, `True` to draw a
  bar or a function to receive each record, and `Control` gains `compaction_request` and
  `await_compaction`; the TypeScript client gains the same, with the `Progress` types.
- **`mosaica compact --wait` and `--request`.** `--wait` stays until the server has answered the
  request, prints the answer, and exits 1 when it was refused or failed, or the server restarted
  and lost it. `--request N` reads a request made earlier.
- **Filter by `mosaica_id`.** A filter's `mosaica_id` leaf, `{"eq": id}` or `{"in": [...]}`,
  keeps the named items the viewer may see, so one `/v1/items` call reads the fields of the points
  a map shows. The TypeScript client has `namedItems(ids)` to build it.
- **Images on the item card.** The card's `image-fields` (`imageFields` in React) lists fields
  whose value is an image's address, drawn as the image across the card. The browser loads it from
  its host without a referrer, and a value that is not an `http:`, `https:` or same-host address
  is shown as text.
- **`[serve] port_retries` share one port unless given their own.** `session` and `control` under
  `[serve]` are now optional, and a plane with no address of its own is served on the viewer's
  address and port, each route keeping its own credential and CORS rules. A deployment that names
  all three addresses is served as before. An open server no longer puts the session and control
  planes one and two ports above the viewer; they share its port unless named.
- **The Python client serves on port 9141 and prints the address.** A database made with
  `mosaica.create()` is served on 9141, or the next free port above it, on this machine only, and
  starting its server prints the address, so a notebook kernel on another machine can be reached
  by forwarding one port. `create()` and `open()` take `port=` to state another; a stated port
  that is taken is refused.

### Added

- **`[serve] port_retries`** tries that many ports above a taken one in turn, and the announce
  line names the port bound. `mosaica serve` binds its addresses before it opens the bundle, so a
  taken port refuses at once.

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

[0.3.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.3.0
[0.2.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.2.0
[0.1.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.1.0
