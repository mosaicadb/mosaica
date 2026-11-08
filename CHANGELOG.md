# Changelog

All notable changes to Mosaica are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). One version covers the `mosaica` binary, the Rust
crates, the `mosaica` and `mosaica-native` Python packages, the `@mosaicajs` npm packages and the
container image.

## [Unreleased]

## [0.3.0] - 2026-11-08

### Changed

- **Bundle format 52.** Bundles built by 0.2.0 must be rebuilt, and writes made to a 0.2.0
  service, including deletions and suppressions, are not carried over.
- **The filter cache covers column filters.** Ranges, `eq` and `in` are kept between requests as
  `match` and `phrase` already were. `serve.text_filter_cache_bytes` is renamed
  `serve.filter_cache_bytes`, and the status field `text_filter_cache` is renamed `filter_cache`.
- **Compaction requests are read one at a time** through the new
  `GET /control/compact/{request}`, which replaces `compaction.requests.last_refusal` in
  `/control/status`.
- **The session and control planes share the viewer's port** unless `[serve]` gives them their
  own address.
- **The Python client serves on port 9141** by default and prints its address. `create()` and
  `open()` take `port=`. The package now depends on `tqdm`.
- **Faster filters, histograms, builds and start-up.**

### Added

- **Progress reporting** for builds, inserts and compactions, through `--progress` on the CLI,
  `jobs` in `/control/status`, and a callback in the Python and TypeScript clients.
- **`mosaica compact --wait` and `--request`** to wait for a compaction request or read one.
- **Filter by `mosaica_id`**, with `namedItems(ids)` in the TypeScript client.
- **Images on the item card**, from the fields listed in `image-fields`.
- **`[serve] port_retries`**, which tries ports above a taken one.

## [0.2.0] - 2026-11-05

### Changed

- **Bundle format 50.** Bundles built by 0.1.0 must be rebuilt, and writes made to a 0.1.0
  service, including deletions and suppressions, are not carried over. The write-ahead log format
  is now 44, so 0.1.0's log is refused too.
- **Phrase search reads word positions from the index**, which makes bundles with text fields
  larger and their builds longer.
- **Searches are kept between requests** for every viewer with the same access, up to
  `serve.text_filter_cache_bytes` (256 MiB by default). Concurrent identical searches run once,
  and a request that waits longer than `serve.single_flight_wait_ms` runs its own.
- **Faster first frames for a new viewer, and faster filtered maps and aggregates.**
- **An aggregate always returns its first page**, even when its stream deadline has passed.
- **Filters stop when the client disconnects**, as does `POST /v1/artifacts/browse`.

### Fixed

- A server replayed a write-ahead log written for another database, such as one from before a
  rebuild, which could expose a deleted or suppressed item and hide an unrelated one. Each log now
  records its database, and a mismatched log is refused.
- Some JSON numbers with seventeen significant digits were read one step away from their true
  value, which could drop an item lying on the edge of a box.
- A `phrase` filter over a group-scoped text field matched nothing.
- Two batches adding the same item through two views of one key could each keep a different value
  for a group-scoped field.
- Building a text field with very many distinct words held about 0.8 GB per 100 million words
  outside the build's memory budget.

### Documentation

- **A documentation website** at <https://mosaicadb.github.io/mosaica/>, published from each
  release tag.
- **A security policy** in SECURITY.md, covering the latest release.
- **A comparison with ClickHouse, PostgreSQL and OpenSearch**, timing a per-viewer map of GBIF
  species records at 10 million to a billion records.

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
