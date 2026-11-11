# Changelog

All notable changes to Mosaica are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). One version covers the `mosaica` binary, the Rust
crates, the `mosaica` and `mosaica-native` Python packages, the `@mosaicajs` npm packages and the
container image.

## [Unreleased]

## [0.4.0] - 2026-11-11

Mosaica 0.4.0 lets a viewer step through an annotation layer. A levelled layer is drawn at a finer
level as the map zooms in, the cluster card walks a hierarchy a page at a time without changing
the filter, and a withheld annotation's children hang from the nearest annotation the viewer is
served. The web components can now be used from the keyboard, by a screen reader and in a small
window. A selection drawn in one layout goes on filtering after a switch to another, and
`mosaica demo` opens a dataset on a map with one command. Text indexes build in less than half
the time they took, and an ingest is no longer held up while a compaction writes its files.

### Upgrading from 0.3.0

- Rebuild every bundle, because the bundle format is now 54 and 0.4.0 refuses a bundle built by
  0.3.0. Writes made to a 0.3.0 service, including deletions and suppressions, are not carried
  over.
- Rename the cluster budget to the cluster spacing wherever you set it, and give it in CSS pixels
  in place of a count of clusters. Replace the `cluster-level` attribute and the `clusterLevel`
  prop with `clusterLevels`, and read `{layer, level}` from `mosaica-levelchange`. The entries
  under Changed list each name.
- Send `parent` without `level` to `POST /v1/artifacts/browse`, which now refuses the two
  together with `422`. A browse cursor from 0.3.0 is refused.
- In Python, give `Selection.within` on a view declared with a projection its box in degrees.
- Check page styles that select the components' parts or elements: the `most-clusters` parts are
  now `cluster-density`, the field card's rows have no `path` part, titles are headings and pairs
  of a name and a value are `<dl>` lists.
- `serve.filter_cache_bytes` now defaults to 1 GiB. Under a memory cap of a few GiB, set it and
  the other cache limits as [Sizing](https://docs.mosaicadb.com/guides/sizing/) describes.

### Added

#### Trying Mosaica

- `mosaica demo NAME...` builds a dataset and opens its map in a browser, with pickers for the
  dataset and for whom to view it as. Install it with `pip install 'mosaica[demo]'`, or run
  `uvx --from 'mosaica[demo]' mosaica demo places`. The datasets are the two in-git samples, the
  whole GeoNames dump and GBIF's occurrence snapshot, whole or sampled, and
  [Datasets to try](https://docs.mosaicadb.com/start/datasets/) describes them.
  `mosaica.datasets.load` does the same from Python and returns the `Database`.

#### Queries and the HTTP API

- **A selection drawn in one layout filters another.** A `region` filter leaf takes `view`, naming
  the view its shape or artifact is in. The TypeScript store keeps one selection for each view, so
  a selection goes on filtering after a switch to another layout and is selected again on the way
  back, and `<mosaica-filter-panel>` shows each one held as a chip, such as "Lasso in Topics". Its
  cost grows with the number of rows inside the selection.
- A box takes `space`: `"wgs84"` for degrees on a view with a map projection, or `"inserted"` for
  the coordinates the view's items were inserted with. `POST /v1/viewport`,
  `POST /v1/artifacts/viewport`, `POST /v1/aggregate` and the `region` filter take it, and so do
  the TypeScript request types and the Python `viewport_artifacts`, `Viewer.map` and
  `Database.map`.
- `fields` is optional on `POST /v1/items` and in every client, and a request without it returns
  every field the view reads.
- `bins` and the summary figures take a bool field, with one bin for false and one for true, and a
  bool's field card lists yes and no with their counts.
- In TypeScript, `AggregateSpec.filters` gives an aggregate a filter of its own, joined with
  `all_of` to what it counts.

#### Clients

- Python: `views()`, `attributes()` and `layers()` on `Database` and `Viewer` list what `/v1/meta`
  describes, each attribute with the filter operators it takes.
- The TypeScript store reports a query in flight in `status.querying`, and `store.cancelQuery()`
  goes back to the last results that finished loading. `<mosaica-status>` shows Running query,
  with a Cancel button, and a `<mosaica-filter>` box shows a spinner for the query it started.
- The notebook map writes what it refuses of `filters`, `colour_by` or `basemap` above the map, as
  well as sending it to the kernel.

#### Accessibility

The components can be used from the keyboard and by a screen reader, and the
[Accessibility](https://docs.mosaicadb.com/system/accessibility/) page says which keys each
element takes, what a screen reader is told and what is not built yet.

- The components announce changes that do not move the focus, such as a query's counts once they
  have loaded or a card opening, and leave pans and zooms unannounced. A boolean `quiet` attribute
  on `<mosaica-map>`, `<mosaica-status>` and `<mosaica-selection>` keeps an element from
  announcing what another already says.
- `<mosaica-explorer>` takes an `aria-label` that names its map, and the Python `Map`, `db.map`,
  `viewer.map` and `selection.map` take `label` for the same purpose.
- A histogram field card's range box takes a typed number or date in From and To fields, so a
  range needs no drag and can fall between bins.
- The item card has a Show more button under a long value that is cut short.
- `--mosaica-focus` sets the colour of the focus ring, and `--mosaica-long-text: normal` wraps a
  title or name too long for its line where the default cuts it with an ellipsis.
- Every example names its map and has a heading and a main landmark, and the `canvas-store`
  example's canvas takes the arrow keys, `+` and `-`.

### Changed

- **Breaking:** bundles built by 0.3.0 must be rebuilt, because the bundle format is now 54.
  Writes made to a 0.3.0 service, including deletions and suppressions, are not carried over.
- **Text indexes are built faster and write fewer bytes.** The build, a flush and a compaction
  now write text and keyword indexes the same way, merging term ranges on several threads, with
  memory allowances and thread counts set from the memory the machine has. At a million items on
  four threads, a build's text stage took 12.8 s rather than 32.1 s and wrote 380 MB rather than
  1,223 MB, and the whole build took 27.5 s rather than 48.7 s. A compaction of a million items
  took 12.0 s rather than 15.0 s, and the server's peak memory during it rose from 377 MiB to
  760 MiB, because the compaction now holds in memory what it used to spill to disc.
- **An ingest is answered while a compaction is publishing.** A compaction writes its derived
  files on its own thread, and the previous data directory is deleted on another, where both used
  to run on the thread that answers every write. With 256-row writes sent back to back during a
  compaction of a million items, the slowest acknowledgement took 0.35 s at most, where it took
  about 2 s.
- A compaction writes each column file once, at its final size, where it used to write a
  temporary copy first. A compaction of 4 million items wrote 319 MB rather than 487 MB.
- **A withheld annotation's children hang from its nearest served ancestors everywhere.** Browse
  (`POST /v1/artifacts/browse`), the map's artifacts frames and bulk reads (`POST /v1/artifacts`)
  now name a parent the way the map's cut already attached it: where a viewer is not served an
  annotation, its served children list the annotations above it that the viewer is served, in
  `parent_ids` and `parents`, under `parent`, in the children form and in `child_count`. Before,
  browse and bulk reads showed such a child as a root, and a frame named no parent for it. A map
  frame names only the parents it carries, and does not climb past a level the request did not
  read. Browse with no `level` on a stacked or tiered layer now lists the top-level annotations,
  and searches, at every level; name a `level` for one level, as before. A bulk read that names
  `parents` or `parent` reads every level above the ones it walks.
- **Browse refuses `level` beside `parent`, and breaks a tie on the filtered count by size.**
  `POST /v1/artifacts/browse` lists a parent's children at every level, so a request naming both
  `level` and `parent` is now a `422`; send `parent` alone. Under `filters`, rows with the same
  `matched_count` are ordered by `masked_count`, largest first, before `mosaica_id`, so an
  annotation matching nothing no longer lists above a larger one that also matches nothing. A
  page's `next` cursor now carries both counts, so a cursor from an earlier build is refused.
- **A level is chosen for each layer.** The explorer keeps a level for each `stacked` or `tiered`
  layer, and choosing one, on a card or in Colour by, leaves the other layers drawn at theirs. A
  `nested` or `dag` layer is never cut at a level. `mosaica-levelchange` now reports
  `{layer, level}`. The `cluster-level` attribute of `<mosaica-map>`, `<mosaica-filter-panel>` and
  `<mosaica-colour-editor>` is replaced by a `clusterLevels` property, a `Map` from layer name to
  level, and the deck layer's `clusterLevel` prop by `clusterLevels`. `<mosaica-field-card>` keeps
  its one `level`.
- **A levelled layer deepens as the map zooms in.** Where a `stacked` or `tiered` layer's levels
  declare no zoom range and no level is chosen for it, the map draws, colours and counts it at the
  finest level that is not crowded in view, which is the coarsest while every level is. A level is
  crowded when a tile in view came back with its full `per_tile` of it, or when more of its
  artifacts are drawn in view than the cluster count the map's size and cluster spacing give, so
  the world view draws a broad level, and Cluster density in the explorer moves the zoom at which
  each level is drawn, and shows while such a layer is drawn. A layer starts at its coarsest level,
  moves to a finer level once that level is within four fifths of the count and back once it is
  over it, and the level is decided once every tile of a view has arrived. With no cluster spacing,
  as on a bare `<mosaica-map>`, only full tiles crowd a level. The card's level choice and Colour by offer
  Follow zoom, which clears the layer's level. A layer whose levels declare zoom ranges is drawn
  at the coarsest level its range serves, as before. The store's `artifacts` projection carries
  `levelsInView`, each level's count drawn in view and whether a tile came back full of it, and
  `followedLevels`, the level each layer is drawn at where none is chosen.
- **The clusters a map shows follow its size.** The cluster budget, a fixed count, is replaced by
  a cluster spacing: each cluster is given a square of the map on screen that many CSS pixels on a
  side, and the count is the map's area over the square's, at least 1. One count cuts a `nested`
  or `dag` layer and crowds a level that follows the zoom, and it is worked out again as the map is
  resized, which `<mosaica-map>` now reports to the store. `cluster-budget` on `<mosaica-map>` and
  `<mosaica-explorer>` is now `cluster-spacing`, `cluster-budget-min` and `cluster-budget-max` are
  `cluster-spacing-min` and `cluster-spacing-max`, the store's `artifacts.budget` option,
  `setClusterBudget` and `clusterBudget` are `artifacts.clusterSpacing`, `setClusterSpacing` and
  `clusterSpacing`, with `clusterCount` for the count, `mosaica-clusterbudgetchange` is
  `mosaica-clusterspacingchange` with `{spacing}`, the React `onClusterBudgetChange` is
  `onClusterSpacingChange`, and the Python widget's `cluster_budget`, `cluster_budget_min` and
  `cluster_budget_max` are `cluster_spacing`, `cluster_spacing_min` and `cluster_spacing_max`. The
  explorer starts at 100 px, 96 clusters on a map 1,200 by 800 px, in place of 1,000 clusters. Its
  Most clusters slider is now Cluster density, with the parts `cluster-density` and
  `cluster-density-value` in place of `most-clusters` and `most-clusters-value`, and it runs from
  400 px at its Fewer end to 50 px at its More end, reading out the count the map shows.
  `clustersInView(spacing, width, height)` gives the count.
- **The cluster card steps through a layer apart from the filter.** On a `tiered`, `nested` or
  `dag` layer, `<mosaica-field-card>` lists the children of its position, or the top-level
  clusters, from the browse route a page at a time. A row with children ends in a button with their
  number that steps into it, which moves the card and leaves the map's filter as it is. A
  breadcrumb from All down to the position steps back, and ticks a crumb the filter is on. Each row
  counts the items matching every filter, the layer's own clauses among them, beside every item of
  it the viewer may see, so a row whose matching count is 0 is still listed, and Filter adds a
  clause that combines with the others by AND. The layer's filter and highlight clauses stand as
  chips under the breadcrumb, each with a × that removes it, wherever the clause was set. A
  `tiered` layer's rows stand under their levels' headings, a `dag` row names its other parents,
  and once every child has loaded a last row counts the position's items in none of them. On a
  `stacked` layer the card ranks the clusters at the chosen or drawn level under every filter, and
  stepping into one ranks the next level's clusters by their items inside it. After a step the card says
  what loaded and moves focus to the rows. A `flat` layer's card is unchanged. The new parts are
  `crumbs`, `crumb`, `crumb-name`, `chips`, `chip`, `chip-name`, `chip-remove`, `step`, `level`,
  `also`, `remainder` and `refusal`, and the `path` part is gone from the card's rows.
- `<mosaica-hierarchy>` gives a screen reader each row's counts with what each counts, and asks
  for the name of a `dag` row's other parent where no page has served it.
- `<mosaica-cluster-filter>` searches every level of a levelled layer in one request with no
  `level`, and walks the parents of a `tiered` layer's matches. A step of a path, there and in the
  colour editor, names every served parent of the step below it.
- A cluster with no name is called by its key in `<mosaica-field-card>`'s rows, crumbs and chips,
  `<mosaica-cluster-filter>`'s rows and paths and `<mosaica-hierarchy>`'s rows, drawn in the
  secondary ink and the identifiers' font with `data-key-name`, and a clause set on it carries the
  key as its `label`. Unnamed stands only where a cluster has neither. A field card inside
  `<mosaica-filter-panel>` leaves out the chips the panel already shows, through its new
  `hide-chips` attribute, which the panel sets except under `controls-only`.
- The documentation website moves to <https://docs.mosaicadb.com/>, and a hosted demo of 73.6
  million Overture Maps places and 50,000 arXiv papers runs at <https://demo.mosaicadb.com/>. The
  Python packages' `Documentation` link and the `mosaica demo` page's Docs link point to the new
  site.
- **Breaking:** in the Python client, `Selection.within` on a view declared with a projection
  takes its box in degrees, `(min_lon, min_lat, max_lon, max_lat)`, the coordinates the rows were
  inserted with. It took the view's own unit-square coordinates before, so a box in degrees
  counted nothing. `sample` and `map` on the selection follow it. For a unit-square box, write
  `filter({"region": {"bbox": [...], "space": "view"}})`.
- Points with no colouring are drawn in the theme's secondary ink, a grey, on a light map, at
  3.8:1 against the default light background at `point-opacity="1"` and 2.8:1 at opacity 0.8.
  They were a pale blue at 1.6:1. On a dark map they are the pale blue they were. The map draws
  them again when its ground changes.
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
- A field card's Highlight and Filter buttons stay in the Tab order while they are not drawn,
  and are always drawn on a screen with no pointer that hovers. Before a row's counts a tick
  marks a value the filter keeps and a pen one that is highlighted. A screen reader is given a
  row's counts at all times, each with what it counts.
- In a value or cluster search box, Enter chooses the suggestion the arrow keys reached. It used
  to take the first row, which could change as counts arrived. Home and End go to the first and
  last suggestion.
- The explorer's narrow layout opens the Item sheet when an item is picked, ends a sheet above
  the status strip, scrolls a sheet's content above its footer, and draws the `top-right` and
  `status` slots. A sheet that reaches up over the map's tools makes the map behind it inert until
  it closes. The part `tabs` is a toolbar of buttons with `aria-expanded`, where it was a list of
  tabs, and `tabs` and `strip-row` are in the page only in the narrow layout.
- A row of `<mosaica-hierarchy>` shows its Highlight, Filter and Fit buttons in place of its
  counts while the pointer or the focus is in it, and Escape puts them away and shows the counts
  again. Where there is no pointer to hover with, both are drawn.
- Tab from the map goes on in the page's order, to the region's Clear button, the selection card
  and the status strip, and then to the card beside the point. The Layers popover takes the focus
  as it opens and closes when the focus leaves it.
- The status strip says "Nothing visible to you" in words, wraps its cells onto further lines
  where it is short of room, and draws a ring for Updating where Up to date is a solid dot.
  Counts that wait for an answer, or stand from before a refusal, are drawn in the faint text
  colour at 4.5:1, where they were greyed to 40% opacity.
- The map takes Escape only when it cleared a shape, a region or a pick, so a dialog that holds a
  map still closes. Escape dismisses the hover tooltip wherever the focus is, and with the focus
  outside the map and the explorer the page's own Escape still acts. The tooltip and the selected
  region's tag stay inside the map. With `ground` unset the map follows a change of the page's or
  the system's colour scheme at once.
- Panel and card titles are headings, an `<h2>` for a panel or card and an `<h3>` inside one,
  pairs of a name and a value are `<dl>` lists, and the rows of the selection card and of the
  artifact card's parents and children hold buttons. Page styles that select these parts by
  element need the new elements.
- A highlight's chip carries a pen, the entry chosen in a menu carries a tick, and a pressed
  segment and a chosen row carry a bar, so no state is shown by colour alone. Under
  forced colours these states take the system's highlight colours, with the focus ring drawn
  inside the control.
- Faint text reaches 4.5:1 on every surface, and the off switch's track and the warning dot reach
  3:1. Small buttons, sliders, switches, swatches, a chip's text and its ×, Clear all and a field
  card's "N more" take a press from 24 px around them, drawn at the size they were.
- The colour picker wraps a palette of more than ten colours onto rows of 24 px squares, and
  keeps text in its hex field that is not a colour, marked and with the format to write. In a
  window too narrow for it to stand beside the colour editor, it opens under the row it was
  opened from, or over that row where there is more room. Add field's list closes on a press
  outside it and when the focus leaves it.

### Fixed

- A `stacked` layer drawn at one level names and outlines that level's artifacts alone. The map
  used to draw every coarser level's names and outlines over it, since nothing links a stacked
  layer's levels.
- `withMembers`, `withRegion` and the store's filters take the operands of an `all_of` they join
  into their own `all_of`, so the TypeScript client's requests under several filter controls,
  clauses and selected regions stay within the four levels of nesting the server accepts. Such a
  request used to be refused.
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
  closes. A field card's row keeps the focus when its value moves in the list.
- The hierarchy keeps its open rows when a filter changes, where pressing Filter on a nested row
  used to close the tree.
- A search box whose list closed no longer reads "No value starts with that" while values
  match.
- The arXiv example dataset names every archive and subject class of its `taxonomy/arxiv` layer,
  such as Astrophysics for `astro-ph`, with arXiv's title for it, so its rows in the hierarchy and
  the cluster card no longer read Unnamed and browse finds them by name.
- The build and the server read a cgroup v1 memory cap as well as a cgroup v2 one. Under a v1 cap
  they used to size themselves from the host's memory, so the cap killed them. The build's
  detected budget no longer has a 2 GiB floor, so under a 1 GiB cap it plans 815 MiB.
- The segments a merge consumes and the extents a coalesce consumes are deleted once no kept
  publication names them. They used to stay on disc until the next compaction, which the default
  schedule may hold off for a day, so a service taking writes kept a growing amount of data it no
  longer used.

### Documentation

- [Count and chart from Python](https://docs.mosaicadb.com/guides/analysis/) works through
  counts, breakdowns, histograms, comparisons and density grids from the Python client, and the
  [Filters](https://docs.mosaicadb.com/reference/filters/) and
  [Aggregations](https://docs.mosaicadb.com/reference/aggregations/) reference pages list every
  operator and grouping.
- [Sizing](https://docs.mosaicadb.com/guides/sizing/) says how much memory to give the data a
  viewer reads often, from a measured example at 100 million items, what each distinct set of
  access attributes costs, and how to set the cache limits under a memory cap smaller than their
  defaults.

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

[0.4.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.4.0
[0.3.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.3.0
[0.2.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.2.0
[0.1.0]: https://github.com/mosaicadb/mosaica/releases/tag/v0.1.0
