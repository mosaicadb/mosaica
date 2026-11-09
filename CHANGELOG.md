# Changelog

All notable changes to Mosaica are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). One version covers the `mosaica` binary, the Rust
crates, the `mosaica` and `mosaica-native` Python packages, the `@mosaicajs` npm packages and the
container image.

## [Unreleased]

### Changed

- `serve.filter_cache_bytes` defaults to 1 GiB, up from 256 MiB.
- The filter cache keeps the rows a condition on a rendered column has read from the map and
  those it matched, so a pan or a repeated viewport reads only the rows it brings into view.
- An `eq` or `in` on a `public` category is answered from its postings where they hold few
  enough items, rather than by reading every row in view. A filter on a rare value over the
  whole map no longer reads every row.

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

[0.3.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.3.0
[0.2.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.2.0
[0.1.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.1.0
