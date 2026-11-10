# Aggregations

An aggregation returns figures about a set of items in place of the items. It gives the set's size, a breakdown by a category or a layer, a histogram of a field, a grid of counts over the map, or a field's smallest, largest and mean value. Every figure is computed over the items the viewer may see, where the viewer is whoever the request's token identifies.

The request is the same JSON on every surface, and a Python dictionary is written the same way. The examples use the arXiv data in `examples/data/arxiv`. Those on `population` use the GeoNames data in `examples/data/geonames`. [Count and chart from Python](../guides/analysis.md) works through them as tasks.

## The request

| Key | Type | What it is |
|---|---|---|
| `view` | string, required | The view the items are counted in. An item with no position in it is not counted. |
| `groupings` | list, required | One [grouping](#groupings) for each table wanted. The answer holds one table for each, in this order. |
| `filters` | [filter](filters.md) | The set counted. Without it the set is every item the viewer may see in the view. |
| `reference` | [filter](filters.md) | A second set to compare with, where `{}` is every item the viewer may see in the view. Each table that counts then has `reference_count` and `lift`. |

```json
{
  "view": "topics",
  "filters": {"title": {"match": "graphene"}},
  "reference": {},
  "groupings": [
    {"count": true},
    {"count": true, "by": {"field": "archive", "top": 3}}
  ]
}
```

A grouping names at least one [figure](#figures). It may divide the set with `by`, with `cells`, or with both.

## Figures

A figure is a key of a grouping that says what each row of its table holds.

| Figure | Written as | Column | Holds |
|---|---|---|---|
| `count` | `"count": true` | `count` | The items of the set in the row. |
| `values` | `"values": [field, ...]` | `values_<field>` | The items holding a finite value in the field. |
| `missing` | `"missing": [field, ...]` | `missing_<field>` | The items holding no value in the field. |
| `min` | `"min": [field, ...]` | `min_<field>` | The smallest value, in the field's type. |
| `max` | `"max": [field, ...]` | `max_<field>` | The largest value, in the field's type. |
| `mean` | `"mean": [field, ...]` | `mean_<field>` | The mean, as a float, or as a UTC timestamp on a `timestamp_us` field. |
| `summary` | `"summary": [field, ...]` | all five above | `values`, `missing`, `min`, `max` and `mean` of each field. |

`count` goes with every grouping. The other six are the field figures.

- **Fields.** A field figure takes number, `timestamp_us` and `bool` fields declared with `index` or `render`. A `bool` counts as 0 and 1, so its mean is the share that are true. Every field figure is exact.
- **Scope.** A field figure covers every item the viewer may see in the view. A grouping that names one takes neither `by` nor `cells`, and a request that names one takes neither `filters` nor `reference`. Not built yet: field figures under a filter or for each group. Read the items and compute them instead.

```json
{"view": "world", "groupings": [{"count": true, "summary": ["population"]}]}
```

| `count` | `values_population` | `missing_population` | `min_population` | `max_population` | `mean_population` |
|---|---|---|---|---|---|
| 150000 | 39542 | 110458 | 1 | 110000000 | 111650.5 |

## Groupings

| Grouping | Written as | One row for each |
|---|---|---|
| [The whole set](#the-whole-set) | no `by` and no `cells` | the set, as a single row |
| [By a category field](#by-a-category-field) | `"by": {"field": name, "top": n}` or `"values": [...]` | value |
| [By a layer](#by-a-layer) | `"by": {"layer": name, "top": n}` or `"artifacts": [...]` | artifact |
| [Histogram](#histogram) | `"by": {"field": name, "bins": n}` | bin |
| [Cells](#cells) | `"cells": {"depth": d}`, alone or beside `by` | cell of the map, within each group |

### The whole set

A grouping with neither `by` nor `cells`, whose table is one row. `{"count": true}` is the size of the set.

### By a category field

A grouping that counts the set by the values of a `category` field. A field of another type is refused.

| Key of `by` | Type | What it does |
|---|---|---|
| `field` | string | The category field. |
| `top` | integer | List the `n` values with the most items, largest first and ties by key. At most the deployment's `max_aggregate_top`. |
| `values` | list of keys | List these values, in the order given. At most `max_aggregate_named`. A key the viewer is not served gets no row. |

`by` takes `top` or `values`. After the listed rows comes a `rest` row, which counts the items holding any other value, and then a `none` row, which counts the items holding no value. Every item holds at most one value, so the rows sum to the set.

```json
{"view": "topics", "groupings": [{"count": true, "by": {"field": "archive", "top": 3}}]}
```

| `group` | `key` | `title` | `count` |
|---|---|---|---|
| `listed` | `cs` | Computer Science | 10478 |
| `listed` | `math` | Mathematics | 10123 |
| `listed` | `astro-ph` | Astrophysics | 6084 |
| `rest` | | | 23315 |

### By a layer

A grouping that counts the set by the artifacts of an annotation layer, such as the papers in each topic.

| Key of `by` | Type | What it does |
|---|---|---|
| `layer` | string | The layer. |
| `level` | integer | The level counted. Required on a layer with several levels and refused on a layer with one. |
| `top` | integer | List the `n` artifacts with the most items of the set. |
| `artifacts` | list of `mosaica_id` | List these artifacts. An id that names nothing the viewer is served gets no row. |
| `cut` | object | On a `nested` or `dag` layer, chooses the artifacts `top` ranks. `{"zoom": z, "bbox": [x0, y0, x1, y1], "budget": b}` ranks the artifacts a map draws at that zoom and box with that cluster budget. The box is in the view's coordinates unless a `space` beside it says otherwise, as for a [`region`](filters.md#region). Without `budget` it ranks the finest artifacts. `top` requires `cut` on these layers, and a layer of any other kind refuses it. |
| `palette_size` | integer | Add each artifact's palette slot among that many colours, as the `slot` column. |

`rest` counts the items that are in an unlisted artifact of the level. `none` counts the items in no artifact of the level. Artifacts can overlap, so the rows can sum to more than the set. The server decides which artifacts a viewer is served from everything the viewer may see, before the filter applies. A filter therefore changes an artifact's count and never whether it is listed.

```json
{"view": "topics", "groupings": [{"count": true, "by": {"layer": "clusters/toponymy", "level": 0, "top": 3}}]}
```

| `group` | `key` | `title` | `count` |
|---|---|---|---|
| `listed` | `9561593462133387664` | Condensed Matter | 3923 |
| `listed` | `2716192028132112914` | AI Systems | 3314 |
| `listed` | `3729937204800416343` | Computational Biology | 2845 |
| `rest` | | | 17222 |
| `none` | | | 22696 |

`key` is the artifact's `mosaica_id`, and a [`member_of`](filters.md#member_of) filter takes it as it is. A `mosaica_id` belongs to one database, so another database built from the same files shows other numbers.

### Histogram

A grouping that counts a number, `timestamp_us` or `bool` field in bins. The field must be declared with `index` or `render`.

| Key of `by` | Type | What it does |
|---|---|---|
| `field` | string | The field binned. |
| `bins` | integer | The most bins wanted, at most `max_aggregate_bins`. |
| `range` | `[lower, upper]` | Cut this range into `bins` equal widths. `rest` then counts the values outside it. |
| `sample` | integer | Count over about this many of the set's items and scale each count to the set's size. |

Without `range`, the server chooses readable edges around the smallest and largest value the viewer may see in the view. It returns the fewest bins of one width that cover both values, and no more than `bins`. On a number field the width is 1, 2, 2.5 or 5 times a power of ten. On a timestamp field it is a round interval from microseconds up to years, such as a day or five years. The edges do not depend on `filters`, so a filtered histogram and an unfiltered one share their bins. A `bool` has a bin for false, from 0 to 1, and one for true, from 1 to 2.

A bin holds the values from `lower` up to but not including `upper`, and the last bin also holds its `upper`. Every bin has a row, including one with a count of 0. After the bins comes a `rest` row for the values in no bin, and then a `none` row for the items with no value.

```json
{"view": "topics", "groupings": [{"count": true, "by": {"field": "submitted_at", "bins": 10}}]}
```

| `group` | `lower` | `upper` | `count` |
|---|---|---|---|
| `listed` | 1990-01-01 | 1995-01-01 | 405 |
| `listed` | 1995-01-01 | 2000-01-01 | 2053 |
| `listed` | 2000-01-01 | 2005-01-01 | 3578 |
| `listed` | 2005-01-01 | 2010-01-01 | 5373 |
| `listed` | 2010-01-01 | 2015-01-01 | 8166 |
| `listed` | 2015-01-01 | 2020-01-01 | 12544 |
| `listed` | 2020-01-01 | 2025-01-01 | 17881 |

With `range`:

```json
{"view": "world", "groupings": [{"count": true, "by": {"field": "population", "bins": 5, "range": [0, 1000000]}}]}
```

| `group` | `lower` | `upper` | `count` |
|---|---|---|---|
| `listed` | 0 | 200000 | 36341 |
| `listed` | 200000 | 400000 | 1608 |
| `listed` | 400000 | 600000 | 547 |
| `listed` | 600000 | 800000 | 263 |
| `listed` | 800000 | 1000000 | 172 |
| `rest` | | | 611 |
| `none` | | | 110458 |

With `sample`, the server counts a large set over part of it. It counts the items whose `mosaica_id` falls below a cut, which selects about `sample` of them, and scales each count to the set's size. It counts exactly when the set is no larger than `sample`, or when `sample` is more than about one item in 64 of the set. `sample.sampled` in the table's head says whether any count was scaled. The edges are exact in both cases.

### Cells

A division of the set into the cells of a grid over the view, used for a density grid. With `by`, each group is divided.

| Key of `cells` | Type | What it does |
|---|---|---|
| `depth` | integer, 0 to 32 | The grid has `2^depth` cells on each side. Up to 16 they are the map's tiles at that zoom. |
| `area` | `[x0, y0, x1, y1]` | List only the cells that meet this box, in the view's coordinates. Each listed cell still counts all of its items. |
| `space` | string | What `area` is written in, in place of the view's coordinates: `"wgs84"` for degrees on a view with a projection, or `"inserted"` for the coordinates the view's items were inserted with. |

There is one row for each cell that holds an item, in ascending order of `cell`. With `by`, the cells of each group follow one another in group order. The cells at `depth` in the area may number at most `max_aggregate_cells`. The default allows the whole view at depth 10.

```json
{"view": "topics", "filters": {"archive": {"eq": "cs"}}, "groupings": [{"count": true, "cells": {"depth": 2}}]}
```

| `cell` | `count` |
|---|---|
| 3 | 2892 |
| 6 | 2426 |
| 9 | 4143 |
| 12 | 1017 |

`cell` holds the cell's position as one number that interleaves the bits of its column and its row. The even bits, from bit 0, are the column and the odd bits, from bit 1, are the row, for `depth` bits of each. Cell 6 above is binary `0110`, so its column is 2 and its row is 1. Columns are counted from `x_min` and rows from `y_min` of the view's `quantisation` in `/v1/meta`. On a view with a projection, `quantisation` is the projection's unit square and row 0 is the northern edge.

Not built yet: a histogram divided into cells. A grouping with `bins` and `cells` is refused.

## The answer

Each table has these columns, in this order, each only where it applies.

| Column | Present with | Holds |
|---|---|---|
| `group` | `by` | `listed` for a row the grouping lists, `rest` for the items in any other group, and `none` for the items in no group. A `rest` or `none` row whose count is 0 is left out. |
| `key` | `by` a field or a layer | A category value's key, or an artifact's `mosaica_id`. Null on `rest` and `none`. |
| `title` | `by` a layer, and `by` a field whose vocabulary holds titles | The value's title, or the artifact's name as the viewer is served it. |
| `slot` | `by` a layer | The artifact's palette slot. Null without `palette_size`. |
| `lower`, `upper` | `bins` | The bin's edges, in the field's type. An integer field has float edges when a bound of `range` is fractional. |
| `cell` | `cells` | The cell's number, as [Cells](#cells) describes. |
| `count` | `count` | The items of the set in the row. |
| `reference_count` | `count` and `reference` | The items of the reference set in the row. |
| `lift` | `count` and `reference` | `(count / total) / (reference_count / reference_total)`. Null where `reference_count` or `total` is 0. |
| `<figure>_<field>` | the field figures | As [Figures](#figures) lists them, in the order `values`, `missing`, `min`, `max`, `mean`. |

With a reference, a row appears where either count is above 0.

Each table also has a head.

| In the head | Present | Holds |
|---|---|---|
| `total` | always | The size of the set. |
| `reference_total` | with `reference` | The size of the reference set. |
| `groups` | with `by` | The groups holding an item of the set, before `top` or the named list cut them down. |
| `sample` | with `bins` and `sample` | `sampled`, whether any count was scaled, and `items`, the items counted. |

On HTTP the head is a frame before the table's rows, as [Wire framing](../openapi/README.md) describes. In Python it is the table's schema metadata `mosaica.head`, as JSON.

A large table comes in pages. The Python client and the CLI read every page and return whole tables. On HTTP, pass the trailer's `next` back as `cursor` until it is null. A histogram's table is always one page. Each page counts the database as it is when that page is read. When the database changed between the pages of one read, `recomposed` in the trailer is true. Python gives it as `mosaica.recomposed`.

## What is refused

Each of these is `422`, with a refusal that says what to write instead.

| In the request | Refused because |
|---|---|
| A grouping with no figure | Every grouping names at least one, such as `"count": true`. |
| A field figure beside `by` or `cells`, or in a request with `filters` or `reference` | Field figures cover the whole view only. |
| A field figure or `bins` on a `category`, `keyword` or `text` field, or on a field with neither `index` nor `render` | They take number, `timestamp_us` and `bool` fields the server can read. |
| `top` or `values` on a field that is not a category | A breakdown by value takes a category. |
| `bins` beside `top` or `values`, or on a layer | `by` takes `bins`, or `top` or a named list, on a field. |
| `bins` beside `cells` | Not built yet: a histogram divided into cells. |
| A layer with several levels and no `level` | The level counted has to be named. |
| `cut` on a layer that is not `nested` or `dag` | `cut` applies to `nested` and `dag` layers. |
| A `depth` above 32 | Positions are stored to 32 bits an axis. |
| More than a limit below allows | The refusal names the limit and the most it allows. |

## Limits

A deployment sets these in [`mosaica.toml`](mosaica-toml.md) and publishes them in `/v1/meta` under `selection`.

| Limit | Default | Bounds |
|---|---|---|
| `max_aggregate_groupings` | 16 | The groupings in one request. |
| `max_aggregate_top` | 1000 | `top`. |
| `max_aggregate_named` | 1000 | The entries of `values` or `artifacts`. |
| `max_aggregate_bins` | 1000 | `bins`. |
| `max_aggregate_cells` | 1,048,576 | The cells at `depth` in the area. |

## Not built yet

- **Breakdowns by value of a `keyword` or integer field.** `top` and `values` take a category.
- **Counts across views.** A request counts in one view.

## Where to ask

| Surface | Call |
|---|---|
| HTTP | `POST /v1/aggregate` |
| Python | `Selection.aggregate` and `Selection.count`; `Viewer.aggregate`; `Database.aggregate` |
| TypeScript | `MosaicaClient.aggregate`; `Store.setAggregate` |
| CLI | `mosaica aggregate`, one grouping a run |

The [HTTP API](http.md) reference states every rule of the route, including how bin edges are computed for each field type.
