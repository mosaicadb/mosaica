# Queries

A client builds everything it shows from a handful of requests: the points of the map, the clusters, topics or regions over them, the values for a filter list, an item's card, a table of records, and the figures behind a chart. This chapter explains what each one returns and how filters work across them. The examples use a collection of research papers with a layer of topics over them.

=== "HTTP"

    | To show | Returns | Route |
    |---|---|---|
    | [The map](#drawing-the-map) | the counts and a sample of points in each tile on screen | `POST /v1/viewport` |
    | [Topics and regions](#annotations-on-the-map) | the artifacts in each tile, with names, counts and positions | `POST /v1/artifacts/viewport` |
    | [A filter list or typeahead](#filter-lists-and-typeahead) | a category's values | `GET /v1/categories/{column}`, `/v1/categories/{column}/suggest` |
    | [An item's card](#reading-records) | one item's fields | `POST /v1/items/{mosaica_id}` |
    | [A table or an export](#reading-records) | every matching item or artifact | `POST /v1/items`, `POST /v1/artifacts` |
    | [A chart or breakdown](#charts-and-breakdowns) | counts by group, histograms and density grids | `POST /v1/aggregate` |

=== "Python"

    `Database` reads a database the Python client runs itself, and `Viewer` reads from a server elsewhere.

    | To show | Returns | Call |
    |---|---|---|
    | [The map](#drawing-the-map) | the counts and a sample of points in each tile on screen | `Database.map`, `Viewer.map`, `Selection.sample` |
    | [Topics and regions](#annotations-on-the-map) | the artifacts in each tile, with names, counts and positions | `Database.viewport_artifacts`, `Viewer.viewport_artifacts` |
    | [A filter list or typeahead](#filter-lists-and-typeahead) | a category's values | `Database.categories`, `Viewer.categories` |
    | [An item's card](#reading-records) | one item's fields | `Database.item`, `Viewer.item` |
    | [A table or an export](#reading-records) | every matching item or artifact | `Database.items`, `Database.artifacts`, and the same on `Viewer` |
    | [A chart or breakdown](#charts-and-breakdowns) | counts by group, histograms and density grids | `Database.aggregate`, `Viewer.aggregate` |

=== "TypeScript"

    `Store` holds a map's state and sends the requests it needs. `MosaicaClient` sends one request.

    | To show | Returns | Call |
    |---|---|---|
    | [The map](#drawing-the-map) | the counts and a sample of points in each tile on screen | `Store.setView`, `MosaicaClient.viewport` |
    | [Topics and regions](#annotations-on-the-map) | the artifacts in each tile, with names, counts and positions | `Store.setLayers`, `MosaicaClient.viewportArtifacts` |
    | [A filter list or typeahead](#filter-lists-and-typeahead) | a category's values | `MosaicaClient.categories`, `Store.suggest` |
    | [An item's card](#reading-records) | one item's fields | `Store.pick`, `MosaicaClient.item` |
    | [A table or an export](#reading-records) | every matching item or artifact | `MosaicaClient.items`, `MosaicaClient.artifacts` |
    | [A chart or breakdown](#charts-and-breakdowns) | counts by group, histograms and density grids | `Store.setAggregate`, `MosaicaClient.aggregate` |

=== "CLI"

    The `mosaica` command reads from a running server with a session token.

    | To show | Returns | Command |
    |---|---|---|
    | [The map](#drawing-the-map) | the counts and a sample of points in each tile on screen | Not built yet |
    | [Topics and regions](#annotations-on-the-map) | the artifacts in each tile, with names, counts and positions | Not built yet |
    | [A filter list or typeahead](#filter-lists-and-typeahead) | a category's values | Not built yet |
    | [An item's card](#reading-records) | one item's fields | Not built yet. `mosaica items` with an `eq` filter on a unique field reads the fields you name of one item in one view |
    | [A table or an export](#reading-records) | every matching item or artifact | `mosaica items`, `mosaica artifacts` |
    | [A chart or breakdown](#charts-and-breakdowns) | counts by group, histograms and density grids | `mosaica aggregate` |

Whichever you send, the answer is computed from the items the viewer may see, as [access control](access-control.md) decides them, so a count never includes a hidden item and a filter can only narrow what the viewer sees. The [HTTP API reference](../reference/http.md) gives every field of every request, with worked examples.

## Drawing the map

A client draws the map tile by tile. It names a view, a zoom level and the area on screen, and for each tile Mosaica returns:

- the items the viewer may see there, how many of them match the filter, and how many of those match the [highlight](#highlighting);
- a sample of the matching points, each with its `mosaica_id`, its position and its rendered fields;
- each point's artifact in every annotation layer the request names, so the client can colour points by cluster or topic.

The counts arrive before the points, so a client can start drawing before the whole answer is in. Where there are too many points to draw, a client can also ask for counts in finer cells within each tile, to draw as shading.

??? info "Finer-cell counts ignore the filter"
    These counts cover everything the viewer may see, whatever the filter. For filtered density, ask for a [density grid](#charts-and-breakdowns) instead.

### Which points are drawn

A tile at a wide zoom can hold millions of items, so Mosaica draws a sample of them. Without a filter, each tile draws about the same share of its items, so a crowded area still looks crowded, and every tile with a matching item draws at least a couple of points, so a sparse area never looks empty. A cap per tile keeps the answer to a size a browser can draw.

The sample is the same on every request, and it nests across zoom levels: a point drawn at one zoom is still drawn when the viewer zooms in, with more appearing around it. Adding a filter works the same way. A filtered tile draws every match, up to the cap, so the points that matched and were already drawn stay where they were, and the other matches appear around them.

Because the sample is taken from the items the viewer may see, a viewer with access to a few thousand items sees those items drawn, rather than a sample of everything with most of it removed.

## Filters

You write a filter as a JSON expression over the declared fields. A field can be filtered once it's [indexed or rendered](data-model.md#items-and-fields), and each kind of field takes its own operators:

| Field kind | Operators | Example |
|---|---|---|
| Category | `eq`, `in` | `{"archive": {"in": ["cs", "stat"]}}` |
| Keyword | `eq`, `in`, `prefix`, `contains` | `{"doi": {"prefix": "10.1145/"}}` |
| Number, true or false, date | `eq`, `in`, `range` | `{"year": {"range": {"gte": 2020, "lt": 2024}}}` |
| Text | `match`, `phrase` | `{"abstract": {"match": "graph neural network"}}` |

Keywords are compared exactly, case included, while text is matched by word, ignoring case: `match` finds items containing every word, and `phrase` the words together and in order. A unique field, such as a DOI, can be filtered with `eq` and `in` even when it's neither indexed nor rendered, which is the quick way to find a list of items. Dates can be written as text, such as `"2020-01-01"`, or in microseconds since 1970.

??? info "Writing dates"
    1 January 2020 is `"2020-01-01"` or `1577836800000000`. A date alone is midnight UTC, and a date with a time needs an offset, as in `"2020-01-01T09:30:00Z"` or `"2020-01-01 10:30:00+01:00"`; a space may stand for the `T`. A date in a range is its first instant, so a range covering 2020 ends at `"lt": "2021-01-01"`; `"lte": "2020-12-31"` would leave out most of 31 December. A string of digits, such as `"20200101"`, is microseconds.

You combine filters with `all_of`, `any_of` and `none_of`:

```json
{"all_of": [
  {"archive": {"eq": "cs"}},
  {"year": {"range": {"gte": 2020}}},
  {"none_of": [{"primary_category": {"in": ["cs.CR", "cs.NI"]}}]}
]}
```

Two more filters work on the map rather than on a field: `region` keeps the items inside a shape the viewer drew, such as a box, circle, ellipse or polygon, and `member_of` keeps the members of one artifact, such as the papers on one topic. `member_of` names the artifact by its `mosaica_id` or by the key you published it under, so a clustering you added as a layer can be filtered to cluster 37 with `{"member_of": {"layer": "hdbscan", "key": "37"}}`. Both combine with every other filter and can be negated. A drawn shape is tested exactly against each item's stored position, but because Mosaica counts the map cells wholly inside it without reading them and tests only the items near its edge, a large shape costs little more than a small one.

!!! note "`none_of` keeps items with no value"
    `{"none_of": [{"archive": {"eq": "cs"}}]}` keeps every paper the viewer may see whose archive is not `cs`, including papers with no archive. The expressions inside one `none_of` may name different fields, and may be free-text searches.

Search results aren't ranked. A relevance score is usually worked out from how common each word is across the whole collection, which would make a viewer's results depend on documents they can't see.

### Highlighting

Where a filter changes what's drawn, a highlight keeps everything drawn and marks the items that also match a second expression, so a client can light them up and dull the rest. You can send both at once, filtering to computer science papers, say, and highlighting those from 2024. Each point then carries a `highlighted` flag, and each tile counts its highlighted items.

## Annotations on the map

The points carry each one's artifact, but not the artifacts themselves, so a client reads those from the annotations request. It returns the artifacts in each tile, with their names and other content, their counts, centres and boxes.

On a layer with levels, each tile returns a limited number of artifacts of each level, largest first by the members the viewer may see, so a small cluster in a crowded area appears as the viewer zooms in. An artifact's count, centre and box cover all of its members the viewer may see, wherever they are, so one that spans several tiles has the same figures in each and can be drawn once. On a `nested` or `dag` layer, a tree of any depth, the request can instead set a budget for the whole screen: where too many fine artifacts would show, their parent comes back in their place, and zooming in moves the cut deeper.

A filter doesn't change which artifacts come back or their counts, so artifacts don't come and go as the viewer types. Instead, each artifact carries a flag saying whether any of its members in the tile match the filter, and another for the highlight. To count each artifact's matching members, ask for a [breakdown by the layer](#charts-and-breakdowns). The [annotations](annotations.md) chapter explains which artifacts a viewer may see at all.

## Filter lists and typeahead

To build a filter list, a client asks for a category's values, with their titles, and can ask for each value's count under the current filter. A public vocabulary lists every value, while a derived one lists only the values held by at least one item the viewer may see.

As the viewer types, the suggestions match the start of each value's key or title, or of any word in its title, ignoring case, so "neural" finds "Graph Neural Networks". Each session runs one suggestion at a time, so a client waits for each answer before sending the next.

??? info "What typeahead does not match"
    Accents are not folded, so "cafe" does not find "Café", and there is no fuzzy matching.

## Reading records

When a viewer opens an item, its card comes from one request, which returns every field, each view the item is in with its position there, and which parts of its access label let the viewer see it. An item the viewer may not see answers as one that doesn't exist.

For many items, a bulk read returns every matching item in a view, with the fields you name, as Apache Arrow tables. The results arrive in pages, each ending with a cursor to continue from, and the Python and TypeScript clients and the `mosaica items` command follow it for you. Each page is built from the latest data, so a deletion during a read applies from the next page, and no item is returned twice. The same read over a layer returns its artifacts, with their parents, content and shapes, and with a filter only the artifacts with a matching member come back, each with its count of matching members.

??? info "Positions are rounded"
    A position read back is rounded to the view's grid. Declare the coordinates as number fields too if you need them exactly.

## Charts and breakdowns

For a chart, an aggregate request works out the figures without returning the items: the counts behind a bar chart, a histogram or a heat map. You list groupings, and each one returns a table:

| Grouping | One row for each | For example |
|---|---|---|
| The whole set | (one row) | how many papers match the filter |
| Category values | value | papers per archive, the ten largest |
| Annotations | artifact | papers per topic at one level |
| Histogram | bin | papers per year |
| Density grid | cell of the map | a heat map of matching papers |

A grouping by values or annotations can also be divided into cells, such as a density grid for each value, and a breakdown by a `nested` or `dag` layer names the same cut the map uses, so the chart lists the artifacts on screen.

The counts are over the items that match the filter. To compare, add a second filter, the reference: each row then gains its count in that set and the lift, how much more common the group is among the matches than in the reference. This request breaks the papers since 2020 down by archive, against all papers:

```json
{
  "view": "map",
  "filters": {"year": {"range": {"gte": 2020}}},
  "reference": {},
  "groupings": [
    {"count": true},
    {"count": true, "by": {"field": "archive", "top": 10}}
  ]
}
```

The first table is one row, the number of papers since 2020. The second has a row for each of the ten largest archives, then a `rest` row for every other archive and a `none` row for papers with no archive, so the counts add up.

A histogram picks readable bin edges, such as whole decades or months, around the values the viewer may see. The edges ignore the filter, so a filtered histogram lines up with an unfiltered one beside it.

A grouping can also ask for a field's figures, such as `{"summary": ["citations"]}`: how many items hold a value, and the smallest, largest and mean. These are taken over everything the viewer may see in the view, so they go in a request of their own, and one with a filter or a reference is refused. Not built yet: field figures under a filter or per group.

The [HTTP API reference](../reference/http.md) lists every grouping, the columns of each table, and how a large table continues over several pages.

## Not built yet

- **Statistics per group**, such as the mean of a number field in each category value. Read the items in bulk and compute them.
- **Other breakdowns**: of keyword fields, of integer fields by value, histograms divided into cells, and counts across views.
- **Searching every category at once.** Typeahead covers one field at a time.
- **Reading one result in parallel.** A bulk read is one chain of requests.
- **Filters that reshape a tree.** A filter marks which artifacts hold matches. It does not change how deep into the tree the map goes.
- **A read pinned to one version of the data.**
