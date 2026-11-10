# Changelog

All notable changes to Mosaica are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). One version covers the `mosaica` binary, the Rust
crates, the `mosaica` and `mosaica-native` Python packages, the `@mosaicajs` npm packages and the
container image.

## [Unreleased]

### Added

- `POST /v1/viewport` takes `space` beside `bbox`: `"wgs84"` gives the box in degrees of
  longitude and latitude on a view with a map projection. The TypeScript client's
  `ViewportRequest` and `region` filter take it too.
- `POST /v1/items` takes `fields` as optional. A request without it returns every field the view
  reads under its bare name, and its cursor is refused once that set of fields changes.
  `mosaica items --fields`, `ItemsRequest.fields` in TypeScript and `fields` on the Python
  `items` calls are optional in the same way.
- Python: `views()`, `attributes(view=None)` and `layers(view=None)` on `Database` and `Viewer`
  list what `/v1/meta` describes, each attribute with the filter operators it takes as
  `operands`.
- The TypeScript store reports a query in flight. `status.querying` is true from a change of
  filters, highlight, `member_of` clauses, selected region or regions held in other views until
  the map, the artifacts and the counts have loaded, and `store.cancelQuery()` goes back to the ones that last finished loading.
  `<mosaica-status>` reads Running query, with a Cancel button, once a query has run for a quarter
  of a second, and a `<mosaica-filter>` box shows a spinner for the query it started. Cancel fires
  `mosaica-filterchange`, `mosaica-clausechange` and `mosaica-selectchange` for what it went back
  to.
- `<mosaica-explorer>` takes an `aria-label`, which names its map for a screen reader, and the
  Python `Map` takes `label` for the same purpose, as do `db.map`, `viewer.map` and
  `selection.map`. A map's canvas is named "Map" without one, and its description lists its keys
  either way.
- The components announce what changes without the focus moving: the state of the view on the
  status strip and over the map, a query's counts once they have loaded, a selected region's
  count, a card opening in the explorer, how many suggestions a search box lists, and whether
  Copy id copied. Pans and zooms announce nothing.
- A histogram field card has a Set range button, and its range box has From and To fields that
  take a typed number or date, so a range needs no drag and can fall between bins. The card
  writes out the range its filter and its highlight hold, and lists its bins' counts in a table
  for a screen reader. Its new parts are `range`, `brush-from` and `brush-to`.
- The item card has a Show more button, part `more`, under a long value that is cut short, and
  Show all on the compact card shows the headline whole.
- `--mosaica-focus` sets the colour of the focus ring, which is 2 px wide on every control and on
  the map's canvas.
- `<mosaica-layer-picker>` says in words which layers are drawn with a layer, in the part
  `linked`.
- The notebook map writes what it refuses of `filters`, `colour_by` or `basemap` in a line above
  the map, as well as sending it to the kernel.
- The `canvas-store` example's canvas takes the arrow keys, `+` and `-`. Every example names its
  map, has a heading and a main landmark, and the two tutorials' pages stack their columns in a
  narrow window.
- The documentation has an [Accessibility](docs/system/accessibility.md) page: the keys each
  element takes, what a screen reader is told, what the page supplies, the contrast to keep when
  overriding a theme token, and what is not built, which includes reaching the map's points from
  the keyboard.

### Changed

- **Breaking:** in the Python client, `Selection.within` on a view declared with a projection
  takes its box in degrees, `(min_lon, min_lat, max_lon, max_lat)`, the coordinates the rows were
  inserted with. It took the view's own unit-square coordinates before, so a box in degrees
  counted nothing. `sample` and `map` on the selection follow it. For a unit-square box, write
  `filter({"region": {"bbox": [...], "space": "view"}})`.
- `serve.filter_cache_bytes` defaults to 1 GiB, up from 256 MiB.
- The filter cache keeps the rows a condition on a rendered column has read from the map and
  those it matched, so a pan or a repeated viewport reads only the rows it brings into view.
- The filter cache keeps the map rows of the items a filter answered over the whole map matches,
  for a filter answered from the items' values and naming no unique field, so a later viewport
  with the same filter reads them instead of finding them again.
- An `eq` or `in` on a `public` category is answered from its postings where they hold few
  enough items, rather than by reading every row in view. A filter on a rare value over the
  whole map no longer reads every row.
- A filter over the map tests each row against its matches through a table of the matches'
  blocks, reading each block before it is needed: about nine times faster per row where the
  matches are many.
- Working out which rows a viewer may see runs on every core the server has free, and does less
  work on one. At 10⁹ items on four cores it takes 0.96 s rather than 5.1 s for a viewer who sees a
  quarter of them, and 3.0 s on one core. Opening a database checks each view's rows in 2.8 s
  rather than about 6 s on four cores.
- A request after a flush or merge is no longer refused with 429 while the server brings each
  viewer's visible set up to date. It brings its own up to date and is answered.
  `refresh_in_flight` in `GET /control/status` still reports the work in progress.
- A field card's row always shows its Highlight and Filter buttons beside its counts, each filled
  with a solid icon while it is on. They used to take the counts' place under the pointer or the
  focus. The range box sits under the histogram, where it used to float over it. A value the
  filter leaves out is drawn in faint text, and long value and cluster names wrap.
- In a value or cluster search box, Enter chooses the suggestion the arrow keys reached. It used
  to take the first row, which could change as counts arrived. Home and End go to the first and
  last suggestion.
- The explorer's narrow layout opens the Item sheet when an item is picked, ends a sheet above
  the status strip, and draws the `top-right` and `status` slots. Its part `tabs` is a toolbar of
  buttons with `aria-expanded`, where it was a list of tabs.
- Tab from the map goes on in the page's order, to the region's Clear button, the selection card
  and the status strip, and then to the card beside the point. The Layers popover takes the focus
  as it opens and closes when the focus leaves it.
- Points with no colouring are a deep blue on a light map, and the pale blue they were on a dark
  one.
- The status strip says "Nothing visible to you" in words, wraps its cells onto further lines
  where it is short of room, and draws a ring for Updating where Up to date is a solid dot.
- The map takes Escape only when it cleared a shape, a region or a pick, so a dialog that holds a
  map still closes. Escape dismisses the hover tooltip wherever the focus is, and the tooltip and
  the selected region's tag stay inside the map.
- Panel and card titles are headings, an `<h2>` for a panel or card and an `<h3>` inside one,
  pairs of a name and a value are `<dl>` lists, and the rows of the selection card and of the
  artifact card's parents and children hold buttons. Page styles that select these parts by
  element need the new elements.
- A highlight's chip carries the word "highlight", the entry chosen in a menu carries a tick, and
  a pressed segment and a chosen row carry a bar, so no state is shown by colour alone. Under
  forced colours these states take the system's highlight colours.
- Faint text reaches 4.5:1 on every surface, and the off switch's track and the warning dot reach
  3:1. Small buttons, sliders, switches, swatches and a chip's × are 24 px targets.
- The colour picker wraps a palette's colours onto rows of 24 px squares, and keeps text in its
  hex field that is not a colour, marked and with the format to write. Add field's list closes
  on a press outside it and when the focus leaves it.

### Fixed

- When a client goes away, Mosaica stops working on its request inside a column, range, `in` or
  keyword filter and inside an aggregate's passes over a column, as it already did inside a text
  filter. Those requests used to run to their end, so a viewer changing filters quickly left the
  server computing answers nobody was waiting for. A filter condition several requests are
  waiting on is worked out until the last of them has gone, so a viewer who pans while a new
  filter is first evaluated does not start it again.
- The plain-HTML example's server abandons its request to Mosaica when the page abandons its
  own. It used to read every proxied answer to the end, so Mosaica kept working on requests the
  page had dropped. It also no longer exits when Mosaica fails part of the way through an answer.
- After a change of filters the TypeScript store asks for a layer's artifacts at once, where it
  used to wait 200 ms as it does after a camera move.
- A `<mosaica-filter>` box empties when the store's filters no longer hold what it sent, and does
  not send text typed before the filters were replaced underneath it.
- The TypeScript store sends a request for a layer's artifacts again when Mosaica sheds it
  (`429`) or is still starting (`503`), as it does a map request. It used to give up at once, so
  the clusters and their labels stayed off the map until the camera next moved.
- After a click on the map, `+` and `-` zoom one level for each press. They zoomed two.
- The notebook map keeps a key pressed in it from the notebook's shortcuts, so that typing in a
  search box or pressing an arrow key on the map does what it does in a page. The map carries
  `data-lm-suppress-shortcuts` for JupyterLab and stops the key at its own element, apart from
  Tab and an Escape it did not use. This has not been tried in a running JupyterLab.
- In the explorer's narrow layout the status strip and the bar of buttons stay inside the
  explorer in a short window, where the map used to be drawn over them, and a menu is no larger
  than the window.
- Focus stays on a control when the one it was on is removed: on the next chip's × after a chip
  is removed, on the hierarchy's row after a filter fetches its rows again, in the list of
  density colours while the arrow keys move through it, and on the swatch when the colour picker
  closes. A field card's row keeps the focus when its value moves in the list, and keeps its
  counts while it has the focus.
- The hierarchy keeps its open rows when a filter changes, where pressing Filter on a nested row
  used to close the tree.
- A search box whose list closed no longer reads "No value starts with that" while values
  match.

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
