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

`POST /v1/aggregate` is Mosaica's analytics query. It summarises the items a viewer may see, without returning them: how many papers each archive or topic holds, how many were published each year, the range and mean of their citation counts, where on the map they are densest, and which archives are over-represented in a selection compared with everything.

A request names a set of items, as a filter, and one or more groupings, and gets back one Arrow table for each. Every figure is computed from the items the viewer may see, in the same way as the map's counts, so a chart beside the map agrees with the map's own counts.

The figures are counts, apart from a summary's smallest, largest and mean. Not built yet: sums, means or other statistics per group. Read the items in [bulk](#bulk-read) to compute those.

### Groupings

| Grouping | The table |
|---|---|
| None | One row: how many items are in the set |
| By a category's values | One row per value |
| By the artifacts of one level of a layer | One row per artifact, such as per topic |
| By bins of a number or date | A histogram |
| A summary of a number or date | One row over everything the viewer may see in the view, ignoring the filter: the count, how many have no value, the smallest, the largest and the mean |
| By cells of the view | One row per non-empty cell, a density grid |
| Values or artifacts, then cells | A density grid for each group |

Each grouping lists the `top` values by count, or the values you name. Two extra rows make the counts add up: `rest`, the items in any other group, and `none`, the items in no group at all. An item can belong to several artifacts, so a layer's rows can add up to more than the set.

A category can be grouped if it is indexed or rendered. A number or date can be binned or summarised under the same condition.

### Comparing with a reference

A request can name a second set, the reference. Each row then also carries the reference's count and the lift: the group's share of the set divided by its share of the reference. With `{}` as the reference, which means everything the viewer may see, the lift shows which archives or topics are over-represented in a selection.

### Histograms

Without a range, the bins' edges are readable values around the smallest and largest value the viewer may see: multiples of 1, 2, 2.5 or 5 times a power of ten for numbers, and whole seconds, minutes, hours, days, weeks, months or years for dates. The edges ignore the filter, so a histogram of the filtered set lines up with one of everything, and the edges stay put as filters change. A bin holds values from its lower edge up to, but not including, its upper edge. The last bin also holds its upper edge.

A histogram can be computed from a sample, for speed on a large set. Ask for a sample size, and Mosaica counts a uniform sample of about that many items and scales each count up. The response says whether it did.

### Topic trees

For a `nested` or `dag` layer, a breakdown can ask for the same cut the map is drawing, with `cut` and the map's zoom, area and budget, so a chart beside the map lists the topics the map shows.

### Density grids

A cell at depth `d` divides the view into a grid of 2^d by 2^d cells. Up to depth 16 a cell is a map tile. A density grid can be limited to an area, and a request for more than `selection.max_aggregate_cells` cells is refused with the deepest depth that fits. The whole view fits at depth 10.

!!! note "Not built yet"
    A summary over a filtered set; histograms within cells; breakdowns of keyword fields and of integer fields by value; one grouping of a kind inside another of the same kind, such as cells within cells; and counts across views. Read the items in bulk and compute these yourself.

## Not built yet

- **Searching every category at once.** A typeahead covers one field at a time.
- **Reading one result in parallel.** A bulk read is one chain of requests, each continuing from the last one's cursor.
- **Filters that reshape a topic tree.** A filter marks which topics hold matches. It does not change how deep into a tree the map goes or prune topics it has emptied.
