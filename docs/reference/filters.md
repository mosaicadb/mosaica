# Filters

A filter is an expression that selects items. It is the same JSON on every surface, and a Python dictionary is written the same way. The examples use the arXiv data in `examples/data/arxiv`. Those that filter on `population` or `geonameid` use the GeoNames data in `examples/data/geonames`.

An expression is a leaf or a combinator. A leaf names one field and one operator, as in `{"archive": {"eq": "cs"}}`, or is one of the three leaves that name no field: [`region`](#region), [`member_of`](#member_of) and [`mosaica_id`](#mosaica_id). A combinator is [`all_of`, `any_of` or `none_of`](#combining) over a list of expressions.

A filter selects among the items the viewer may see, where the viewer is whoever the request's token identifies. It can hide items and cannot reveal any.

## Which fields can be filtered

A field takes filters when it is declared with `index` or `render`. `keyword` and `text` fields cannot be rendered, so they need `index`. A field declared `unique` takes `eq` and `in` with neither. It takes its type's other operators once it is indexed.

`/v1/meta` lists the fields a database filters by as `filter_operands`, and gives each field's operators. In Python that is `db.meta()["filter_operands"]`:

```json
[
  {"column": "entity_id", "family": "numeric", "operands": ["eq", "in"]},
  {"column": "arxiv_id", "family": "keyword", "operands": ["eq", "in", "prefix", "contains"]},
  {"column": "archive", "family": "category", "operands": ["eq", "in"]},
  {"column": "primary_category", "family": "category", "operands": ["eq", "in"]},
  {"column": "submitted_at", "family": "numeric", "operands": ["eq", "in", "range"]},
  {"column": "title", "family": "text", "operands": ["match", "phrase"]}
]
```

## Operators by field type

The types are those an [`[[attribute]]`](schema-toml.md#attribute) declares.

| Field type | Operators | A value is | Example |
|---|---|---|---|
| `category` | `eq`, `in` | the value's key, or its code as an integer | `{"archive": {"in": ["cs", "stat"]}}` |
| `keyword` | `eq`, `in`, `prefix`, `contains` | a string, compared exactly | `{"arxiv_id": {"prefix": "2401."}}` |
| `text` | `match`, `phrase` | words to search for | `{"title": {"match": "graphene"}}` |
| `u8` to `u64`, `i8` to `i64`, `f32`, `f64` | `eq`, `in`, `range` | a number | `{"population": {"range": {"gte": 1000000}}}` |
| `timestamp_us` | `eq`, `in`, `range` | a date, a date and time with an offset, or microseconds since 1970 | `{"submitted_at": {"range": {"gte": "2020-01-01", "lt": "2021-01-01"}}}` |
| `bool` | `eq`, `in`, `range` | `true` or `false`, or 1 or 0 | `{"is_capital": {"eq": true}}` |

The shipped examples declare no `bool` field, so `is_capital` stands for one of your own.

An operator outside the field's list is refused with `422`. The refusal names the operators the field takes, and calls the field a column, as every refusal does:

```
column 'archive' is a category column, which takes ["eq", "in"]; it does not take 'range' (HTTP 422)
```

## Operators

Each leaf carries one operator. To apply two tests to one field, write two leaves under [`all_of`](#combining).

### `eq`

A test that keeps the items whose value equals the one given. It applies to every type except `text`.

```json
{"archive": {"eq": "cs"}}
```

Of the 50,000 papers, 10,478 match.

### `in`

A test that keeps the items whose value is any of those listed. It applies to every type except `text`.

```json
{"archive": {"in": ["cs", "stat"]}}
```

11,486 papers match: the 10,478 in `cs` and the 1,008 in `stat`.

### `range`

A test that keeps the items whose value lies between bounds. It applies to number, `timestamp_us` and `bool` fields. A range carries at least one of the bounds `gte`, `gt`, `lte` and `lt`, and at most one lower and one upper bound.

```json
{"population": {"range": {"gte": 100000, "lt": 1000000}}}
```

Of the 150,000 GeoNames places, 5,812 match. A range with `gt` and `gte` together is refused with ``column 'population': `range` carries two bounds on the same side``.

### `prefix`

A test on a `keyword` field that keeps the items whose value starts with the string given.

```json
{"arxiv_id": {"prefix": "2401."}}
```

302 papers match. Their identifiers were issued in January 2024.

### `contains`

A test on a `keyword` field that keeps the items whose value holds the string given anywhere in it.

```json
{"arxiv_id": {"contains": "hep-th"}}
```

869 papers match. Each has an identifier in the older form, such as `hep-th/9901001`.

### `match`

A search of a `text` field that keeps the items holding every word given, in any order.

```json
{"title": {"match": "neural network"}}
```

203 titles match. To keep the items holding at least some of the words, give an object with the words as `query` and the number required as `minimum_should_match`:

```json
{"title": {"match": {"query": "graph neural quantum", "minimum_should_match": 2}}}
```

102 titles hold at least two of the three words.

### `phrase`

A search of a `text` field that keeps the items holding the words given next to each other and in that order.

```json
{"title": {"phrase": "neural network"}}
```

193 titles match, ten fewer than `match` finds for the same words.

## Values

**Categories.** A category value is named by its key in the vocabulary, such as `"cs"`, or by its integer code. One list may mix keys and codes. A value the vocabulary does not hold, or one the viewer may not see, matches no item and is not refused.

**Keywords.** A keyword is compared byte for byte, so case counts: `{"arxiv_id": {"contains": "HEP-TH"}}` matches nothing.

**Text.** A text field is searched by word, ignoring case, so `"Graphene"` and `"graphene"` find the same 234 titles. Words are matched as written and are not reduced to a stem: `"network"` finds 751 titles and `"networks"` finds 1,169. Results are not ranked.

**Numbers.** An integer may be written as a string of its decimal digits, optionally after `-` or `+`. Use that form for an integer past 2^53, which a JSON number does not carry exactly.

**Dates and times.** A `timestamp_us` value is written in any of three forms.

| Form | Example | Read as |
|---|---|---|
| A date | `"2020-01-01"` | midnight UTC at the start of that day |
| A date and time with an offset | `"2020-01-01T09:30:00Z"`, `"2020-01-01 10:30:00+01:00"` | that instant, with at most six digits of fractional seconds |
| Microseconds since the Unix epoch | `1577836800000000`, `"1577836800000000"` | that instant |

A date is its first instant, so a range covering 2020 ends at `"lt": "2021-01-01"`. `"lte": "2020-12-31"` would leave out everything on 31 December after midnight. A date and time with no offset is refused:

```
column 'submitted_at': '2020-01-01T00:00:00' is a date-time with no offset; add Z or an offset such as +01:00 after the time (HTTP 422)
```

In Python, [`Selection.filter`](python/reading.md#mosaica._viewer.Selection.filter) also takes a `datetime`, a `date`, a pandas `Timestamp` or a NumPy `datetime64` and sends it as microseconds.

**Unique fields.** `eq` and `in` on a unique field are answered from its index. Use them to find items by an identifier of your own:

```json
{"geonameid": {"in": [2964574, 2643743]}}
```

That finds Dublin and London. An identifier held only by an item the viewer may not see matches nothing.

## Leaves that name no field

### `region`

A leaf that keeps the items positioned inside a shape on the map. It carries one shape, in the coordinates of the view the request names.

| Shape | Written as | Example |
|---|---|---|
| `bbox` | `[min_x, min_y, max_x, max_y]`, with its edges inside | `{"region": {"bbox": [0, 0, 10, 10]}}` |
| `circle` | `[centre_x, centre_y, radius]` | `{"region": {"circle": [0, 0, 5]}}` |
| `ellipse` | `[centre_x, centre_y, a, b, angle_in_degrees]` | `{"region": {"ellipse": [0, 0, 8, 3, 30]}}` |
| `polygon` | a list of at least three `[x, y]` vertices, closed for you | `{"region": {"polygon": [[0, 0], [10, 0], [5, 10]]}}` |
| `artifact` | the `mosaica_id` of an artifact on this view, whose members are kept | `{"region": {"artifact": "9561593462133387664"}}` |

On the arXiv view `topics` the bbox holds 13,132 papers, the circle 7,390, the ellipse 6,115 and the polygon 6,534. The artifact is the topic "Condensed Matter" and holds 3,923.

On a view with a projection, the view's coordinates are the projected ones. For `web_mercator` they lie in a unit square. To give a shape in longitude and latitude, add `"space": "wgs84"`:

```json
{"region": {"bbox": [-10.7, 51.3, -5.3, 55.5], "space": "wgs84"}}
```

That box around Ireland holds 341 of the GeoNames places. `"space": "wgs84"` on a view with no projection is refused.

The `x-mosaica-region` response header says how exact an answer over a `region` is. Python gives it as the table's `mosaica.region` metadata. The value is `exact` unless the shape's outline crosses more grid cells than the deployment's `max_region_cells`. Past that limit the value is `cover; depth=<d>`, and the answer is exact for a slightly larger shape that covers the one asked for. A polygon with more vertices than `max_region_vertices` is refused.

In Python, [`Selection.within(box)`](python/reading.md#mosaica._viewer.Selection.within) adds a `bbox` region in the coordinates the rows were inserted with. On a view with a projection those are longitude and latitude, and on any other view the view's own. `within` sends `"space": "inserted"`, which the server reads as `wgs84` on a view with a projection and as `view` on one without. For a box in a projected view's own coordinates, write the `region` filter with `"space": "view"`. The other boxes a request carries take `space` in the same way: the `bbox` of `POST /v1/viewport` and of `POST /v1/artifacts/viewport`, and an aggregate's `cells.area` and `cut.bbox`.

### `member_of`

A leaf that keeps the members of one artifact of a layer, such as the papers in one topic. It names the layer, and names the artifact in one of two ways.

| Names the artifact by | Written as |
|---|---|
| Its `mosaica_id`, as responses give it | `{"member_of": {"layer": "clusters/toponymy", "artifact": "9561593462133387664"}}` |
| The key it was published under, and its level | `{"member_of": {"layer": "clusters/toponymy", "key": "tp3-000000", "level": 3}}` |

`level` defaults to 0, the only level of a layer that declares none. The second example keeps 92 papers.

A layer the viewer's `/v1/meta` does not list is refused with `422`, and so is a level the layer does not hold. An artifact or key that names nothing the viewer is served matches no item.

### `mosaica_id`

A leaf that keeps the items whose `mosaica_id` is given. It takes `eq` or `in`.

```json
{"mosaica_id": {"in": ["9711030025428260651", "12573669445876031793"]}}
```

Use it to read fields of points already drawn. An identifier matches nothing when it names nothing, names an artifact, or names an item the viewer may not see. A `mosaica_id` belongs to one database, so these two name items only in the database they were read from.

## Combining

A combinator carries a list of expressions, each a leaf or another combinator.

| Combinator | Keeps the items that | With an empty list |
|---|---|---|
| `all_of` | match every expression | every item |
| `any_of` | match at least one expression | no item |
| `none_of` | match none of the expressions | every item |

```json
{"all_of": [
  {"archive": {"eq": "cs"}},
  {"submitted_at": {"range": {"gte": "2020-01-01"}}},
  {"none_of": [{"primary_category": {"in": ["cs.CR", "cs.NI"]}}]}
]}
```

6,042 papers match: those in computer science since 2020, outside cryptography and networking.

`none_of` keeps an item that holds no value in the field tested. `{"none_of": [{"archive": {"eq": "cs"}}]}` keeps every paper whose archive is not `cs`, including any paper with no archive. The expressions under `none_of` may name different fields, and may be text searches or regions.

Each object in an expression carries one key, and two fields in one object are refused:

```
a filter node must carry exactly one key, a column name or `all_of`/`any_of`/`none_of`, and this one carries 2; join several under `all_of` or `any_of` (HTTP 422)
```

An expression nests at most four levels deep, where a leaf counts as one level. A deeper expression is refused:

```
the filter nests 5 deep and the limit is 4; flatten it (HTTP 422)
```

## What is refused

| In the expression | Answer |
|---|---|
| A field that is not declared, or is neither indexed, rendered nor unique | `422`, naming the fields that can be filtered |
| An operator the field's type does not take | `422`, naming the operators it takes |
| A value the field does not hold, or that the viewer may not see | No items match |
| A layer in `member_of` that the viewer is not served | `422` |
| An artifact, key or `mosaica_id` that names nothing the viewer is served | No items match |

A value that exists and one that does not answer alike, so a filter does not tell a viewer whether a value is held by items they may not see.

## Where a filter goes

| Surface | Takes a filter as |
|---|---|
| HTTP | `filters` in the body of `POST /v1/viewport`, `/v1/items`, `/v1/aggregate`, `/v1/artifacts`, `/v1/artifacts/browse`, `/v1/artifacts/viewport` and `/v1/categories/{column}/suggest` |
| Python | `Selection.filter` and `Selection.within`; `filters=` on `Viewer.items`, `Viewer.aggregate`, `Viewer.artifacts`, `Viewer.categories` and the same methods of `Database` |
| TypeScript | `filters` on the `MosaicaClient` methods; `Store.setFilters` |
| CLI | `--filters` on `mosaica items`, `mosaica artifacts` and `mosaica aggregate` |

A highlight is a second expression in the same form. It marks the points that match and leaves every point drawn. HTTP takes it as `highlight` on the two viewport routes, and Python takes it as `highlight=` on `Selection.sample`. The reference set of an [aggregation](aggregations.md#the-request) is a third use of the same form.

[Capabilities](capabilities.md) names every call that takes a filter, and [Queries](../system/queries.md#filters) explains how filters are answered.
