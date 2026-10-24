# Queries

Mosaica answers six kinds of query. This chapter covers each, and the filters most of them take. The examples use the research papers of the [data model](data-model.md): a collection laid out by abstract, with a layer of topics over it.

| Query | Route | Returns |
|---|---|---|
| [Viewport](#viewport) | `POST /v1/viewport` | counts and a sample of points for each map tile |
| [Annotations by tile](#annotations-by-tile) | `POST /v1/artifacts/viewport` | the clusters, topics or regions in each tile, with their counts and positions |
| [Category values](#category-values) | `GET /v1/categories/{column}`, and `GET` or `POST /v1/categories/{column}/suggest` | a category's values, for filter lists and typeahead |
| [Item](#item) | `POST /v1/items/{mosaica_id}` | one item's fields and positions |
| [Bulk read](#bulk-read) | `POST /v1/items`, `POST /v1/artifacts` | every matching item or annotation, in pages of Arrow tables |
| [Aggregate](#aggregate) | `POST /v1/aggregate` | breakdowns, histograms, summaries and density grids |

Every query is answered from the items the viewer may see. A count never includes an item hidden from the viewer, and a filter can only narrow what they see. [Access control](access-control.md) explains how a viewer's visible items are decided. The Python and TypeScript clients wrap each route, and the [capabilities](../reference/capabilities.md) table names the call on each surface. The [HTTP API reference](../reference/http.md) has worked examples of every request.

## Viewport

A map is drawn tile by tile. A viewport request names a view, a zoom level, and the area on screen, as a bounding box or a list of tiles. For each tile, the answer carries:

- **Counts**: `visible`, the items in the tile the viewer may see; `matched`, those that pass the filter; `highlighted`, those that also pass the [highlight](#highlighting); and `served`, how many points the tile drew.
- **A sample of the matching points**, each with its `mosaica_id`, its position and its rendered fields. A category arrives as a numeric code, which [category values](#category-values) turns back into keys and titles.
- **Each point's annotation**, for each layer the request names: the artifact it belongs to, so the client can colour points by topic.
- **Optionally, counts for finer cells** beneath the tile (`underlay_offset`): how many items the viewer may see in each, before any filter. A client can draw them as shading to show density where the points are too many to draw.

Answers stream back as Apache Arrow frames: first every tile's counts, then the points in frames of whole tiles, so a client can start drawing before the whole answer has arrived. A request with `k` set to 0 asks for the counts alone, with no points.

### Which points are drawn

A tile at a wide zoom may hold millions of items, far more than a screen can show. Mosaica draws a sample, chosen by three rules:

- **A floor.** Every tile that holds an item the viewer may see shows at least `k_min` of them, two by default. A viewer who may see very few items never gets a blank map.
- **A threshold.** Each tile draws the same share of its items, so a tile with twice the items draws about twice the points. The share is set from everything the viewer may see in the view, so that an average tile draws `theta_target_marks` points, 16 by default.
- **A cap.** No tile draws more than `k` points, and `k` itself is capped by the server's `k_max_marks`, 500 by default.

Which items are drawn first is decided by a fixed rank derived from each item's `mosaica_id`, so the sample is spread evenly and is the same each time. The sample also nests across zoom levels. A point drawn at one zoom is still drawn when the viewer zooms in, and more points appear around it, so points do not flicker in and out.

The sample is taken from the viewer's visible items. Mosaica never samples from every item and then removes the hidden ones. The share of each tile that is drawn is set before any filter is applied. A filter then leaves out the points that do not match, and a matching point that was drawn stays drawn, so the map thins as a viewer types instead of reshuffling.

!!! warning "Keep `k` steady while zooming in"
    `k` defaults to the server's cap, `k_max_marks`. A client may ask for fewer points per tile, but if it lowers `k` while zooming in, a point it was already showing can disappear.

## Filters

A filter is a JSON expression over the declared fields. Each kind of field has its own operators:

| Field kind | Operators | Example |
|---|---|---|
| Category | `eq`, `in` | `{"archive": {"in": ["cs", "stat"]}}` |
| Keyword | `eq`, `in`, `prefix`, `contains` | `{"doi": {"prefix": "10.1145/"}}` |
| Number, true or false, date and time | `eq`, `in`, `range` | `{"year": {"range": {"gte": 2020, "lt": 2024}}}` |
| Text | `match`, `phrase` | `{"abstract": {"match": "graph neural network"}}` |

A field must be [indexed or rendered](data-model.md#items-and-fields) to be filtered.

- **Categories** take their values' keys, or their codes.
- **Keywords** are compared exactly, case included.
- **Ranges** take `gt` or `gte` for the lower bound and `lt` or `lte` for the upper, and either side can be left open.
- **Dates and times** are given in microseconds since the Unix epoch, such as `1577836800000000` for 1 January 2020. A true-or-false field takes `true` or `false`.
- **Text** is searched by word, ignoring case. `match` finds items containing every word given, or at least a number of them with `{"query": ..., "minimum_should_match": n}`. `phrase` finds the words next to each other, in that order.

Filters combine with `all_of`, `any_of` and `none_of`, nested at most four levels deep:

```json
{"all_of": [
  {"archive": {"eq": "cs"}},
  {"year": {"range": {"gte": 2020}}},
  {"none_of": [{"primary_category": {"in": ["cs.CR", "cs.NI"]}}]}
]}
```

Two more filters work on the map rather than on a field:

- **`region`** keeps the items inside a shape: a box, a circle, an ellipse or a polygon, in the view's coordinates, or with `"space": "wgs84"` in longitude and latitude on a geographic view. It can also name an artifact's published shape, as `{"artifact": <mosaica_id>}`. See [drawn regions](#drawn-regions).
- **`member_of`** keeps the members of one artifact, such as the papers in one topic: `{"member_of": {"layer": "topics", "artifact": 123}}`.

`GET /v1/meta` lists each filterable field and its operators under `filter_operands`. A field that does not exist or cannot be filtered, or an operator the field does not take, is refused with 422.

!!! note "How `none_of` treats missing values"
    A `none_of` keeps the items that hold a value in its field and match none of the expressions inside it. An item with no value in that field is left out: `{"none_of": [{"archive": {"eq": "cs"}}]}` does not return papers with no archive. Every expression inside one `none_of` must name the same field. A text field cannot be negated, because Mosaica keeps no record of which items have text in it. `region` and `member_of` can be negated, and then mean everything outside the shape, or every item outside the artifact.

!!! note "A hidden value matches nothing"
    A filter naming a category value the viewer may not see behaves exactly like one naming a value that does not exist: it matches nothing, and the answer gives no hint which was the case.

!!! note "Large integers"
    JSON numbers lose precision above 2^53. An integer or timestamp value can be sent as a string of digits, such as `"1152921504606846976"`, which is exact across the whole 64-bit range.

### Search results are not ranked

A text search matches or it does not. There is no relevance score. A relevance score is usually computed from how common each word is across the whole collection, and a viewer's results would then depend on documents that viewer cannot see.

### Filtering by a unique field

A field declared `unique` can be filtered with `eq` and `in` even if it is neither indexed nor rendered, since its unique index finds the items holding each value. This is the quick way to find a list of papers by DOI.

## Drawn regions

A viewer can draw a shape on the map and filter to the items inside it. The shape is sent as a `region` filter, so it combines with every other filter, and it can be negated to mean everything outside it.

The test is exact, against each item's stored position. Mosaica splits the shape against the map's grid. Cells wholly inside the shape are counted whole, without reading their items, and only the cells the edge passes through are tested point by point. A large, simple shape therefore costs little more than a small one, and the cost follows the length of the shape's edge rather than its area.

!!! note "Very long edges"
    If splitting a shape's edge would need more cells than the server's `max_region_cells` allows, Mosaica stops at the finest level that fits and counts every cell the edge crosses there as inside. The answer is then exact for a slightly larger shape, and the `x-mosaica-region` header reads `cover; depth=N` instead of `exact`. A polygon with more than `max_region_vertices` vertices, 10,000 by default, is refused.

## Highlighting

A filter narrows what is drawn. A highlight keeps everything that is drawn and marks the items that also match a second expression, so a client can light them up and dull the rest. A request can carry both: filter to computer science papers, say, and highlight those from 2024.

A highlight never changes which points or annotations are served. Each point drawn carries a `highlighted` flag, and each tile counts how many of its matched items are highlighted. In the annotations of each tile, every artifact carries two flags, `matched` and `highlighted`.

## Annotations by tile

The points of a viewport answer carry each point's artifact, but not the artifacts themselves. A client reads those, with their keys, content such as names, counts, centres and boxes, from `POST /v1/artifacts/viewport`.

- **How many per tile.** On a flat, stacked or tiered layer, the request sets `per_tile`, the most artifacts of each level a tile returns. The largest come first, by how many of their members the viewer may see.
- **Counts are over the whole artifact.** The count, centre and box of an artifact cover all of its members the viewer may see, wherever they lie. An artifact that spans several tiles appears in each with the same figures, so a client can draw it once.
- **A filter marks artifacts.** A filter does not change which artifacts are served or their counts, so topics do not appear and disappear as a viewer types. It adds a flag to each artifact saying whether any of its members in that tile match. To count the matching members of each topic, use a bulk read of the layer with the filter, or an [aggregate](#aggregate) by the layer.
- **Hierarchies.** A `nested` or `dag` layer, a topic tree, is cut to the request's `budget` across all the tiles together, and served as one frame before the per-tile frames. Where too many fine topics would show, their parent is served instead. Zooming in moves the cut deeper. With no `budget`, the cut is as fine as the tree allows.

The [annotations](annotations.md) chapter explains which artifacts a viewer is served at all.

## Category values

A client building a filter list asks for a category's values. `GET /v1/categories/{column}` lists them, and `/v1/categories/{column}/suggest` answers as a viewer types.

- **Which values are listed** depends on the vocabulary. A public vocabulary lists every value. A derived one lists only the values carried by at least one item the viewer may see.
- **Codes.** `?codes=` turns the codes a viewport's points carry into keys and titles. A code the viewer may not see is left out, as an unknown one is.
- **Typeahead** matches the typed text against the start of each value's key and title, and the start of each word in its title, or in its key when it has no title. Case is ignored, and accents are not: "cafe" does not find "Café". "neural" finds "Graph Neural Networks". There is no fuzzy matching, and results come in alphabetical order of the text that matched.
- **Counts.** With `counts`, each suggestion carries how many of the viewer's items hold it. A `POST` with `view` and `filters` takes those counts under the filter, and a value the filter excludes is still offered, with a count of zero.

!!! note "One suggestion at a time"
    Each session runs one suggestion request at a time. A request sent while another is running is refused with 429, so wait for each answer, or debounce keystrokes.

## Item

`POST /v1/items/{mosaica_id}` returns one item as the viewer may see it: every field, each view it appears in that the viewer can reach, with its position there in the view's grid units, and any per-view field values.

An item the viewer may not see answers exactly as one that does not exist, with a 404.

The answer also says why the viewer may see the item: the parts of its access label the viewer satisfies, written as label text, such as `team_a` or `public`. It names nothing the viewer does not hold.

## Bulk read

`POST /v1/items` returns every item in a view that the viewer may see and the filter matches, with the fields you name. `POST /v1/artifacts` does the same for the artifacts of one layer, all of them or those a list of `mosaica_id`s names. Use them for exports, tables and analysis.

- **Pages of Arrow.** Results arrive as pages of Apache Arrow record batches. Each response ends with a cursor, and you pass it back to continue, until it is null. The Python client, the TypeScript client and the `mosaica items` and `mosaica artifacts` commands follow the cursor for you.
- **Fields.** Name the fields you want. Each row starts with `mosaica_id`. Two more columns can be asked for: `position`, the item's coordinates in the view, and `labels`, why the viewer may see it. A field an item has no value for is null. A category arrives as its values' keys.
- **Order.** `order: "map"` returns rows by map cell, then by `mosaica_id`. `order: "stored"` follows the order items are stored in, which is faster when the fields are read only from the stored records. With neither, the server chooses.
- **Matched and unmatched.** With `keep_unmatched`, every visible item is returned, each with a `mosaica:matched` flag.
- **Totals.** With `count` on the first request, the answer starts with exact counts of the visible items and of those the filter matches.
- **Artifacts.** A read of artifacts can ask for each one's parents, content, centre, box and shape, all as the viewer sees them. It can read one level, the children of one artifact, or those whose key or title contains some text. With a filter, only artifacts with a matching member come back, each with its count of matching members. With `keep_unmatched`, every artifact comes back, with a count of zero where none match.

!!! note "Positions are rounded to the grid"
    `position` is the stored position converted back to the view's coordinates, so it is within half a grid step of what was inserted. If you need the exact values, declare them as number fields too.

!!! note "A read is not a snapshot"
    Each page is built from the latest data. A deletion or suppression accepted during a read applies from the next page. An item inserted during a read is returned if it lands ahead of the cursor, and no item is returned twice. Not built yet: a read pinned to one version of the data.

A cursor is sealed, so it cannot be read or forged. It works only for the read it came from, and only for a session with the same access. It stays valid across flushes, merges, compactions and restarts, but not across a rebuild of the database or a view dropped and created again. The fields and the filter may change between the requests of one read. The view and the order may not.

## Aggregate

`POST /v1/aggregate` computes figures over a set of items without returning the items: counts by group, histograms, density grids, and a summary of a number or date field. It is the query behind charts, facet counts and breakdowns beside a map.

### The request

| Field | Required | What it does |
|---|---|---|
| `view` | Yes | The view to count in. An item with no position in it is not counted |
| `filters` | No | Which items to count, in the same [filter](#filters) syntax every query takes. Without it, the set is every item the viewer may see in the view |
| `groupings` | Yes | A list of groupings. Each produces one table, in the same order |
| `reference` | No | A second set to compare with, also a filter. `{}` means every item the viewer may see in the view |

For example, this request counts the papers from 2020 onwards, breaks them down by archive, and compares each archive's share with its share of all papers:

```json
{
  "view": "map",
  "filters": {"year": {"range": {"gte": 2020}}},
  "groupings": [
    {"count": true},
    {"count": true, "by": {"field": "archive", "top": 10}}
  ],
  "reference": {}
}
```

It returns two tables. The first has one row, the number of papers from 2020 onwards. The second has a row for each of the ten largest archives, with its count in the set, its count in the reference, and the lift.

### Groupings

A grouping says two things: how to divide the set into rows, and which figures each row holds. `{"count": true}` alone is one row, the size of the set. Add `by`, `cells` or both to divide it:

| Rows | Written as | One row for each | Needs |
|---|---|---|---|
| The whole set | neither `by` nor `cells` | (one row only) | |
| Category values | `"by": {"field": "archive", "top": 10}`, or `"values": ["cs", "math"]` in place of `top` | value | a category that is indexed or rendered, or has a derived vocabulary |
| Annotations | `"by": {"layer": "topics", "level": 1, "top": 10}`, or `"artifacts": [...]` in place of `top` | artifact | a layer the viewer may see. `level` on a layer with several levels |
| Histogram | `"by": {"field": "year", "bins": 20}` | bin | a number or date that is indexed or rendered |
| Density grid | `"cells": {"depth": 8}` | non-empty cell of the view | |
| Grid per group | `"by": {...}, "cells": {...}` | group and cell | as for the `by` part. Not with a histogram |

### Figures

Each grouping names at least one figure:

- **`"count": true`** counts the items of the set in each row.
- **`values`, `missing`, `min`, `max` and `mean`** each list number or date fields that are indexed or rendered, and give one column per field: how many items hold a value, how many hold none, the smallest and largest value, and the mean.
- **`summary`** lists fields and gives all five of those figures for each. `{"count": true, "summary": ["citations"]}` is the number of papers and the spread of their citation counts.

The field figures describe everything the viewer may see in the view. A grouping that names one has neither `by` nor `cells`, and a request that names one has neither `filters` nor `reference`, so a summary can sit beside a filtered histogram as a fixed description of the field. Send it as a request of its own.

### What comes back

Each table is an Arrow table. Its columns depend on the grouping:

- `group`, `key` and `title`, for values and annotations. `group` is `listed` for a value or artifact you asked for, `rest` for the items in any other, and `none` for the items in none at all, so a table's counts add up to the size of the set. An item can be in several artifacts, so a layer's counts can add up to more.
- `lower` and `upper`, for a histogram: each bin's edges.
- `cell`, for a density grid: the cell's position code. At depth 16 or less it is the map tile at that zoom.
- `count`: the items of the set in the row's group or cell.
- `reference_count` and `lift`, with a reference: the row's count in the reference set, and its share of the set divided by its share of the reference. A lift above 1 means the group is more common in the set than in the reference.
- A column for each field figure, named after the figure and the field: `values_citations`, `missing_citations`, `min_citations`, `max_citations` and `mean_citations`. An item holding NaN or an infinity counts in neither `values_` nor `missing_`, and is left out of the others.

Each table also carries a head with the size of the set, `total`, the size of the reference, and how many groups there are in all before `top` keeps the largest.

### Histograms

Give `bins` alone, and Mosaica picks readable edges around the smallest and largest value the viewer may see: multiples of 1, 2, 2.5 or 5 times a power of ten for numbers, and whole seconds, minutes, hours, days, weeks, months or years for dates, with no more bins than you asked for. The edges ignore the filter, so a histogram of a filtered set lines up with one of everything. Give `range` as well, and the range is cut into exactly `bins` equal bins.

A bin holds values from its lower edge up to, but not including, its upper edge. The last bin also holds its upper edge.

On a large set, `sample` speeds a histogram up: Mosaica counts a uniform sample of about that many items and scales each count to the whole set. The table head says whether it did.

### Annotation trees

On a `nested` or `dag` layer, which the map draws as a cut through the tree, `top` needs `cut`: the zoom, area and budget the map's own [annotations](#annotations-by-tile) request uses. The table then lists the topics the map is showing, so a chart beside the map matches it.

### Density grids

At depth `d`, the view is divided into a grid of 2^d by 2^d cells, and each row counts the items in one cell. `area` limits the grid to part of the view. A request for more cells than `selection.max_aggregate_cells` allows is refused, with the deepest depth that fits. The whole view fits at depth 10.

### Large results

A table longer than a page continues through a cursor, as a [bulk read](#bulk-read) does. The Python and TypeScript clients and `mosaica aggregate` follow the cursor for you.

!!! note "Not built yet"
    Field figures per group or over a filtered set; histograms within cells; breakdowns of keyword fields, and of integer fields by value; a grouping of one kind inside another of the same kind, such as cells within cells; and counts across views. Read the items in bulk and compute these yourself.

## Not built yet

- **Searching every category at once.** A typeahead covers one field at a time.
- **Reading one result in parallel.** A bulk read is one chain of requests, each continuing from the last one's cursor.
- **Filters that reshape a topic tree.** A filter marks which topics hold matches. It does not change how deep into a tree the map goes or prune topics it has emptied.
