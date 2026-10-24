# Queries

Mosaica answers six kinds of query. This chapter covers each, and the filters they all share. The examples use the research papers of the [data model](data-model.md): a collection laid out by abstract, with a layer of topics over it.

| Query | Route | Returns |
|---|---|---|
| [Viewport](#viewport) | `POST /v1/viewport` | counts and a sample of points for each map tile |
| [Annotations by tile](#annotations-by-tile) | `POST /v1/artifacts/viewport` | the clusters, topics or regions in each tile, with their counts and positions |
| [Category values](#category-values) | `GET /v1/categories/{column}`, and `/suggest` | a category's values, for filter lists and typeahead |
| [Item](#item) | `POST /v1/items/{mosaica_id}` | one item's fields and positions |
| [Bulk read](#bulk-read) | `POST /v1/items`, `POST /v1/artifacts` | every matching item or annotation, in pages of Arrow tables |
| [Aggregate](#aggregate) | `POST /v1/aggregate` | breakdowns, histograms, summaries and density grids |

Every query is answered from the items the viewer may see. A count never includes an item hidden from the viewer, and a filter can only narrow what they see. [Access control](access-control.md) explains how a viewer's visible items are decided. The Python and TypeScript clients wrap each route, and the [capabilities](../reference/capabilities.md) table names the call on each surface.

## Viewport

A map is drawn tile by tile. A viewport request names a view, a zoom level, and the area on screen, as a bounding box or a list of tiles. For each tile, the answer carries:

- **Three counts**: `visible`, the items in the tile the viewer may see; `matched`, those that pass the filter; and `highlighted`, those that also pass the [highlight](#highlighting).
- **A sample of points to draw**, each with its `mosaica_id`, its position and its rendered fields.
- **Each point's annotation**, for each layer the request names: the artifact it belongs to, so the client can colour points by topic.
- **Optionally, counts for finer cells** beneath the tile, which a client can draw as shading to show density where the points are too many to draw.

Answers stream back as Apache Arrow frames, a tile at a time, so a client can start drawing before the whole answer has arrived. A request with `k` set to 0 asks for the counts alone, with no points.

### Which points are drawn

A tile at a wide zoom may hold millions of items, far more than a screen can show. Mosaica draws a sample, chosen by three rules:

- **A floor.** Every tile that holds an item the viewer may see shows at least a few. A viewer who may see very few items never gets a blank map.
- **A threshold.** Dense tiles show more points than sparse ones, in proportion to how many items the viewer may see across the whole view.
- **A cap.** No tile returns more than a set number of points, `k`, which bounds cost and clutter.

Which items are drawn first is decided by a fixed rank derived from each item's `mosaica_id`, so the sample is spread evenly and is the same each time. The sample also nests across zoom levels. A point drawn at one zoom is still drawn when the viewer zooms in, and more points appear around it, so points do not flicker in and out.

The sample is drawn from the viewer's own visible items, never from everything with the hidden ones removed. The density rules count against everything the viewer may see, not the filtered set, so the number of points in a tile does not jump as a viewer types a filter.

!!! warning "Keep `k` steady while zooming in"
    `k` defaults to the server's cap. A client may ask for fewer points per tile, but if it lowers `k` while zooming in, a point it was already showing can disappear.

## Filters

A filter is a JSON expression over the declared fields. Each kind of field has its own operators:

| Field kind | Operators | Example |
|---|---|---|
| Category | `eq`, `in` | `{"archive": {"in": ["cs", "stat"]}}` |
| Keyword | `eq`, `in`, `prefix`, `contains` | `{"doi": {"prefix": "10.1145/"}}` |
| Number, true or false, date and time | `eq`, `in`, `range` | `{"year": {"range": {"gte": 2020, "lt": 2024}}}` |
| Text | `match`, `phrase` | `{"abstract": {"match": "graph neural network"}}` |

A field must be [indexed or rendered](data-model.md#items-and-fields) to be filtered. A category accepts its values' keys, or their codes. A `range` takes `gt` or `gte` for its lower bound and `lt` or `lte` for its upper, and either side can be left open.

Text search is by word. `match` finds items containing every word given, or at least a number of them with `{"query": ..., "minimum_should_match": n}`, and `phrase` finds the words in order. Words are matched regardless of case.

Filters combine with `all_of`, `any_of` and `none_of`:

```json
{"all_of": [
  {"archive": {"eq": "cs"}},
  {"year": {"range": {"gte": 2020}}},
  {"none_of": [{"primary_category": {"in": ["cs.CR", "cs.NI"]}}]}
]}
```

Two more filters work on the map rather than on a field:

- **`region`** keeps the items inside a shape: a box, a circle, an ellipse or a polygon, in the view's coordinates or, on a geographic view, in longitude and latitude. See [drawn regions](#drawn-regions).
- **`member_of`** keeps the members of one artifact, such as the papers in one topic.

!!! note "Rules for `none_of`"
    A `none_of` must name a single field, and the values to exclude. A free-text field cannot be negated. Both rules keep a negation from reaching items whose values the viewer cannot see. `region` and `member_of` can be negated.

!!! note "A hidden value matches nothing"
    A filter naming a category value the viewer may not see behaves exactly like one naming a value that does not exist: it matches nothing, and the answer gives no hint which was the case. A field or operator that does not exist is refused.

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
    If a shape's edge crosses more cells than the server's `max_region_cells` setting allows, the remaining edge cells are counted as if they were inside. The answer is then exact for a slightly larger shape, and the response's `x-mosaica-region` header says so.

## Highlighting

A filter narrows what is drawn. A highlight keeps everything that is drawn and marks the items that also match a second expression, so a client can light them up and dull the rest. A request can carry both: filter to computer science papers, say, and highlight those from 2024.

A highlight never changes which points or annotations are served. Each point carries two flags, `matched` and `highlighted`, and each tile counts how many of its matched items were highlighted.

## Annotations by tile

The points of a viewport answer carry each point's artifact, but not the artifacts themselves. A client reads those, with their titles, counts, centres and boxes, from `POST /v1/artifacts/viewport`, tile by tile.

- **How many per tile.** The request sets `per_tile`, the most artifacts of each level a tile returns. The largest come first, by how many of their members the viewer may see.
- **Counts are over the whole artifact.** The count, centre and box of an artifact cover all of its members the viewer may see, wherever they lie. An artifact that spans several tiles appears in each with the same figures, so a client can draw it once.
- **Filters mark, they do not hide.** A filter does not change which artifacts are served or their counts, so topics do not appear and disappear as a viewer types. It adds a flag to each artifact saying whether any of its members in that tile match. To count the matching members of each topic, use a [breakdown by layer](#groupings).
- **Hierarchies.** A `nested` or `dag` layer, a topic tree, is cut to the request's budget across all the tiles together: where too many fine topics would show, their parent is served instead. Zooming in moves the cut deeper.

The [annotations](annotations.md) chapter explains which artifacts a viewer is served at all.

## Category values

A client building a filter list asks for a category's values. `GET /v1/categories/{column}` lists them, and `/v1/categories/{column}/suggest` answers as a viewer types.

- **Which values are listed** depends on the vocabulary. A public vocabulary lists every value. A derived one lists only the values carried by at least one item the viewer may see.
- **Typeahead** matches the typed text against the start of each value's key and title, and the start of each word in either, ignoring case and accents. "neural" finds "Graph Neural Networks". There is no fuzzy matching, and results come in the order of the matched text, not by popularity.
- **Counts.** A suggestion can carry how many of the viewer's items hold each value, under the filter the viewer has applied. A value the filter excludes is still offered, with a count of zero.

## Item

`POST /v1/items/{mosaica_id}` returns one item as the viewer may see it: every field, each view it appears in that the viewer can reach with its position there, and any per-view field values.

An item the viewer may not see answers exactly as one that does not exist, with a 404.

The answer also says why the viewer may see the item: the parts of its access label the viewer satisfies, written as label text, such as `team_a` or `public`. It names nothing the viewer does not hold.

## Bulk read

`POST /v1/items` returns every item in a view that the viewer may see and the filter matches, with the fields you name. `POST /v1/artifacts` does the same for the artifacts of one layer, or for a list of `mosaica_id`s. Use them for exports, tables and analysis.

- **Pages of Arrow.** Results arrive as pages of Apache Arrow record batches. Each response ends with a cursor, and you pass it back to continue, until it is null. The Python client, the TypeScript client and the `mosaica items` and `mosaica artifacts` commands follow the cursor for you.
- **Fields.** Name the fields you want. Each row starts with `mosaica_id`. Two extra fields are available: `position`, the item's coordinates in the view, and `labels`, why the viewer may see it. A field an item has no value for is null. A category arrives as its values' keys.
- **Matched and unmatched.** With `keep_unmatched`, every visible item is returned, each with a `mosaica:matched` flag.
- **Totals.** With `count` on the first request, the answer starts with exact counts of the visible items and of those the filter matches.
- **Artifacts.** A read of artifacts can ask for each one's parents, content, centre, box and shape, all as the viewer sees them. It can read one level, the children of one artifact, or those whose key or title contains some text. With a filter, each artifact also carries its count of matching members.

!!! note "Positions are rounded to the grid"
    `position` is the stored position converted back to the view's coordinates, so it is within half a grid step of what was inserted. If you need the exact values, declare them as number fields too.

!!! note "A read is not a snapshot"
    Each page is built from the latest data. A deletion or suppression accepted during a read applies from the next page. An item inserted during a read is returned if it lands ahead of the cursor, and no item is returned twice. Not built yet: a read pinned to one version of the data.

A cursor is sealed, so it cannot be read or forged. It works only for the read it came from, and only for a session with the same access. It stays valid across restarts and background maintenance.

## Aggregate

`POST /v1/aggregate` computes figures over a set of items without returning the items: counts by group, histograms, a summary of a number field, and density grids. It is the query behind charts, facet counts and breakdowns beside a map.

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
    {},
    {"by": {"field": "archive", "top": 10}}
  ],
  "reference": {}
}
```

It returns two tables. The first has one row, the number of papers from 2020 onwards. The second has a row for each of the ten largest archives, with its count in the set, its count in the reference, and the lift.

### Groupings

| Grouping | Written as | One row for each | Needs |
|---|---|---|---|
| Size | `{}` | the whole set | |
| Category values | `{"by": {"field": "archive", "top": 10}}`, or `"values": ["cs", "math"]` in place of `top` | value | a category that is indexed or rendered, or has a derived vocabulary |
| Annotations | `{"by": {"layer": "topics", "level": 1, "top": 10}}`, or `"artifacts": [...]` in place of `top` | artifact | a layer the viewer may see. `level` on a layer with several levels |
| Histogram | `{"by": {"field": "year", "bins": 20}}` | bin | a number or date that is indexed or rendered |
| Summary | `{"by": {"field": "citations", "summary": true}}` | (one row only) | a number or date that is indexed or rendered |
| Density grid | `{"cells": {"depth": 8}}` | non-empty cell of the view | |
| Grid per group | `{"by": {...}, "cells": {...}}` | group and cell | as for the `by` part. Not with a histogram or a summary |

### What comes back

Each table is an Arrow table. Its columns depend on the grouping:

- `count`: the items of the set in the row's group or cell.
- `group`, `key` and `title`, for values and annotations. `group` is `listed` for a value or artifact you asked for, `rest` for the items in any other, and `none` for the items in none at all, so a table's counts add up to the size of the set. An item can be in several artifacts, so a layer's counts can add up to more.
- `lower` and `upper`, for a histogram: each bin's edges.
- `cell`, for a density grid: the cell's position code. At depth 16 or less it is the map tile at that zoom.
- `reference_count` and `lift`, with a reference: the row's count in the reference set, and its share of the set divided by its share of the reference. A lift above 1 means the group is more common in the set than in the reference.

Each table also carries a head with the size of the set, `total`, the size of the reference, and how many groups there are in all before `top` keeps the largest.

A summary has its own columns: `items`, every item the viewer may see in the view; `count`, those with a finite value; `none`, those with no value; and `min`, `max` and `mean`.

!!! note "A summary ignores the filter"
    A summary describes everything the viewer may see in the view, whatever `filters` and `reference` say, so it can sit beside a histogram as a fixed description of the field. Not built yet: a summary over a filtered set.

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
    Sums, means or other statistics per group; a summary over a filtered set; histograms within cells; breakdowns of keyword fields, and of integer fields by value; a grouping of one kind inside another of the same kind, such as cells within cells; and counts across views. Read the items in bulk and compute these yourself.

## Not built yet

- **Searching every category at once.** A typeahead covers one field at a time.
- **Reading one result in parallel.** A bulk read is one chain of requests, each continuing from the last one's cursor.
- **Filters that reshape a topic tree.** A filter marks which topics hold matches. It does not change how deep into a tree the map goes or prune topics it has emptied.
