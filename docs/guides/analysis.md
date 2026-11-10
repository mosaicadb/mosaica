# Count and chart from Python

When you want figures about the items on a map, ask the server for them. It counts a selection, breaks it down by a category or by a layer, bins a date or a number into a histogram, compares one set of items with another, and counts each cell of a grid for a density plot. The server works each answer out over the items you may see and returns it as a small table, so you don't have to read the items into pandas first.

This guide makes each of those requests from the Python client, against the 50,000 arXiv papers that ship in the repository. It uses a few operators and groupings, and the [filters](../reference/filters.md) and [aggregations](../reference/aggregations.md) reference pages list them all.

You'll need the repository cloned, Python 3.12 or later, and the client with pandas and NumPy. The density plot also uses matplotlib.

```bash
pip install mosaica pandas numpy matplotlib
```

## A database to count

Run this from the root of the clone. It loads the papers into one view, `topics`. Each paper carries its archive, subject class, submission date and title, and Toponymy's named topics are added as a layer. [Analysing a map with Mosaica](../start/python.md#analysing-a-map-with-mosaica) explains each call.

``` {.python notest}
from pathlib import Path

import pandas as pd

import mosaica

arxiv = Path("examples/data/arxiv")
frame = pd.read_parquet(arxiv / "points.parquet")

db = mosaica.create()
db.declare_view("topics")
db.declare_vocabulary("archive", closed=True, title="Archive")
db.declare_vocabulary("primary_category", closed=True, title="Subject class")
db.declare_columns(
    frame,
    skip=["x", "y", "categories"],
    category=["archive", "primary_category"],
    keyword=["arxiv_id"],
    unique=["entity_id"],
    render=["archive", "primary_category", "submitted_at"],
    index=["title", "arxiv_id"],
)
db.declare_layer(
    "clusters/toponymy",
    kind="tiered",
    levels=[(0, "16 topics"), (1, "46 topics"), (2, "161 topics"), (3, "574 topics")],
)
db.declare_labels("topics/toponymy", of="clusters/toponymy")

db.insert("archive", arxiv / "archive.parquet", key="key")
db.insert("primary_category", arxiv / "primary_category.parquet", key="key")
db.insert("topics", frame, x="x", y="y")
db.insert("clusters/toponymy", artifacts=arxiv / "clusters-toponymy.parquet")
db.insert(
    "clusters/toponymy",
    members=arxiv / "clusters-toponymy-members.parquet",
    columns={"entity_id": "entity"},
)
db.insert("topics/toponymy", arxiv / "topics-toponymy.parquet")
db.commit()
```

A field can be counted and filtered once it is rendered or indexed. Here the two categories and the date are rendered, and the title is indexed.

Everything below works the same on a database someone else runs. `mosaica.connect(url, token)` returns a reader for such a database, with the same `view`, `aggregate` and `items`. A reader is whoever a request's token identifies, and every figure it gets is counted over the items it may see. To read your own database as someone who holds only certain access terms, call `db.viewer(terms)`.

## Count a selection

To count part of a view, first say which part. `db.view(name)` is a selection that holds a whole view. `filter` narrows a selection by a field, and `within` narrows it by a box on the map. Each returns a new selection, so you can start several from the same one. `count` asks the server how many items a selection holds.

``` {.python notest}
import datetime

papers = db.view("topics")
cs = papers.filter({"archive": {"eq": "cs"}})
recent_cs = cs.filter({"submitted_at": {"range": {"gte": datetime.date(2020, 1, 1)}}})

papers.count(), cs.count(), recent_cs.count()
```

```
(50000, 10478, 6429)
```

A filter is a dictionary that maps a field to one test. Which tests a field takes depends on its type:

``` {.python notest}
papers.filter({"archive": {"in": ["cs", "stat"]}}).count()        # a category: eq, in
papers.filter({"title": {"match": "graphene"}}).count()           # text: match, phrase
papers.filter({"arxiv_id": {"prefix": "2401."}}).count()          # a keyword: eq, in, prefix, contains
papers.filter({"submitted_at": {"range": {"lt": "2000-01-01"}}}).count()  # a number or a date: eq, in, range
```

`within` takes `(min_x, min_y, max_x, max_y)` in the coordinates the rows were inserted with. On a view with a projection those are longitude and latitude, so `db.view("world").within((-10.7, 51.3, -5.3, 55.5))` is a box around Ireland. To give a box in such a view's projected coordinates, use the [`region`](../reference/filters.md#region) filter with `"space": "view"`.

``` {.python notest}
recent_cs.within((0, 0, 10, 10)).count()
```

```
3075
```

The [filters reference](../reference/filters.md) lists the rest: regions of other shapes, the `member_of` filter that keeps the members of one topic, and `all_of`, `any_of` and `none_of`, which combine filters.

## Ask for a breakdown

A count is one number. To see how a selection divides, call `aggregate` with a list of groupings. It returns one pyarrow table for each grouping, in the same order. A grouping says what to count and how to divide it. `{"count": True}` on its own gives the size of the selection, and `count()` sends exactly that. Add `by` to get a row for each group.

``` {.python notest}
[size, classes] = recent_cs.aggregate(
    [
        {"count": True},
        {"count": True, "by": {"field": "primary_category", "top": 5}},
    ]
)
classes.to_pandas()
```

```
    group    key                                    title  count
0  listed  cs.CV  Computer Vision and Pattern Recognition   1421
1  listed  cs.LG                         Machine Learning   1381
2  listed  cs.CL                 Computation and Language    785
3  listed  cs.RO                                 Robotics    382
4  listed  cs.CR                Cryptography and Security    274
5    rest    NaN                                      NaN   2186
```

`top` lists the values with the most items. The `rest` row counts the items that hold any other value. When some items hold no value, a `none` row counts them. With those rows the counts add up to the selection. Each table carries its totals in its schema metadata:

``` {.python notest}
import json

json.loads(classes.schema.metadata[b"mosaica.head"])
```

```
{'grouping': 1, 'groups': 37, 'total': 6429}
```

`total` is the size of the set counted. `groups` is the number of values that hold at least one of its items, and these papers fall in 37 subject classes.

To count particular values, name them:

``` {.python notest}
[named] = papers.aggregate(
    [{"count": True, "by": {"field": "archive", "values": ["cs", "stat", "q-bio"]}}]
)
named.to_pandas()
```

```
    group    key                 title  count
0  listed     cs      Computer Science  10478
1  listed   stat            Statistics   1008
2  listed  q-bio  Quantitative Biology    582
3    rest    NaN                   NaN  37932
```

`db.aggregate(view, groupings, filters=...)` is the same call with the view and the filter passed as arguments.

## Count by a layer

To count a selection by a clustering, group by its layer. Each row is then one of the layer's annotations, such as a topic. Mosaica's calls and reference pages name an annotation an artifact. A layer with levels needs the level named.

``` {.python notest}
[topics] = papers.aggregate(
    [{"count": True, "by": {"layer": "clusters/toponymy", "level": 0, "top": 5}}]
)
topics.to_pandas()[["group", "key", "title", "count"]]
```

```
    group                   key                  title  count
0  listed   5181630941776965921       Condensed Matter   3923
1  listed   9734042235579792079             AI Systems   3314
2  listed   3995158766810875216  Computational Biology   2845
3  listed  16226756225815602489     Integrable Systems   2620
4  listed   1611147678718191952     Geometric Analysis   1918
5    rest                   NaN                    NaN  12684
6    none                   NaN                    NaN  22696
```

`none` counts the papers in no topic of this level. Each `key` is the annotation's `mosaica_id`. Your database will show other numbers, because a `mosaica_id` belongs to the database that issued it. Pass a key to a `member_of` filter as it is to ask a further question about one topic. Which archives do the papers in "AI Systems" come from?

``` {.python notest}
ai = topics.to_pandas().set_index("title").loc["AI Systems", "key"]
in_ai = papers.filter({"member_of": {"layer": "clusters/toponymy", "artifact": ai}})

[ai_archives] = in_ai.aggregate([{"count": True, "by": {"field": "archive", "top": 3}}])
ai_archives.to_pandas()
```

```
    group   key                                       title  count
0  listed    cs                            Computer Science   2872
1  listed  eess  Electrical Engineering and Systems Science    202
2  listed  math                                 Mathematics    114
3    rest   NaN                                         NaN    126
```

Where a layer's annotations overlap, an item can be in several of them, and the rows can then sum to more than the selection.

## Draw a histogram

To see how a number, a date or a true-or-false field is distributed, replace `top` with `bins`. You give the most bins you want, and the server picks readable edges that need no more than that.

``` {.python notest}
[years] = papers.aggregate([{"count": True, "by": {"field": "submitted_at", "bins": 10}}])
years.to_pandas()
```

```
    group                     lower                     upper  count
0  listed 1990-01-01 00:00:00+00:00 1995-01-01 00:00:00+00:00    405
1  listed 1995-01-01 00:00:00+00:00 2000-01-01 00:00:00+00:00   2053
2  listed 2000-01-01 00:00:00+00:00 2005-01-01 00:00:00+00:00   3578
3  listed 2005-01-01 00:00:00+00:00 2010-01-01 00:00:00+00:00   5373
4  listed 2010-01-01 00:00:00+00:00 2015-01-01 00:00:00+00:00   8166
5  listed 2015-01-01 00:00:00+00:00 2020-01-01 00:00:00+00:00  12544
6  listed 2020-01-01 00:00:00+00:00 2025-01-01 00:00:00+00:00  17881
```

Ten bins would each be about three and a half years wide, so the server chose five-year bins and returned seven. `years.to_pandas().plot.bar(x="lower", y="count")` draws it.

The server takes the edges from the smallest and largest value in the whole view and ignores the filter. A filtered histogram therefore has the same bins as an unfiltered one, and the two can share an axis:

``` {.python notest}
[cs_years] = cs.aggregate([{"count": True, "by": {"field": "submitted_at", "bins": 10}}])
years.to_pandas()[["lower", "count"]].assign(cs=cs_years.column("count").to_pandas())
```

```
                      lower  count    cs
0 1990-01-01 00:00:00+00:00    405     2
1 1995-01-01 00:00:00+00:00   2053    15
2 2000-01-01 00:00:00+00:00   3578    64
3 2005-01-01 00:00:00+00:00   5373   251
4 2010-01-01 00:00:00+00:00   8166   986
5 2015-01-01 00:00:00+00:00  12544  2731
6 2020-01-01 00:00:00+00:00  17881  6429
```

To choose the bins yourself, add `"range": [lower, upper]`. The range is then cut into `bins` equal widths, and a `rest` row counts the values outside it. `{"field": "submitted_at", "bins": 12, "range": ["2023-01-01", "2024-01-01"]}` cuts 2023 into twelve bins of 30 days and 10 hours each. The bins have equal widths, so they do not line up with calendar months.

## Compare with a reference

A count says how many of a selection fall in a group. To see whether that is more or fewer than you'd expect, pass a second set as `reference`. Here it is `{}`, the empty filter, which stands for every item the reader may see in the view. Each row then also has the group's count in the reference, and a `lift` that compares the two.

``` {.python notest}
[graphene] = papers.filter({"title": {"match": "graphene"}}).aggregate(
    [{"count": True, "by": {"field": "archive", "top": 3}}], reference={}
)
graphene.to_pandas()
```

```
    group       key             title  count  reference_count      lift
0  listed  cond-mat  Condensed Matter    205             5887  7.440703
1  listed   physics           Physics     21             3411  1.315503
2  listed  quant-ph   Quantum Physics      4             2115  0.404114
3    rest       NaN               NaN      4            38587  0.022150
```

`lift` is the group's share of the selection divided by its share of the reference. Condensed Matter holds 205 of the 234 graphene papers and 5,887 of all 50,000, so a graphene paper is 7.4 times as likely to be in it as a paper picked from the whole view. A lift near 1 means the group is as common in the selection as anywhere else.

The reference can be any filter, such as `reference={"archive": {"eq": "cond-mat"}}` to compare within one archive.

## Read a field's range and mean

For the range and the mean of a number, a date or a true-or-false field, ask for its `summary`. It returns how many items hold a value, how many hold none, and the smallest, largest and mean value. `min`, `max`, `mean`, `values` and `missing` ask for one of those each.

``` {.python notest}
[dates] = papers.aggregate([{"count": True, "summary": ["submitted_at"]}])
dates.to_pandas().T
```

```
                                                     0
count                                            50000
values_submitted_at                              50000
missing_submitted_at                                 0
min_submitted_at             1990-11-16 00:00:00+00:00
max_submitted_at             2024-09-19 16:59:02+00:00
mean_submitted_at     2015-06-08 05:46:15.172260+00:00
```

These figures cover every item the reader may see in the view. Not built yet: the same figures under a filter or for each group. Asking for them on a narrowed selection is refused:

```
summary is computed over the whole visible set only; ask for it in a grouping without by or cells, in a request without filters or reference (HTTP 422)
```

To get those figures for a narrowed selection, read the fields you need with `items` and work them out in pandas. For the 10,478 computer science papers:

``` {.python notest}
records = cs.items(["primary_category", "submitted_at"]).to_pandas()
records.groupby("primary_category").submitted_at.agg(["count", "min", "median"]).sort_values(
    "count", ascending=False
).head(3)
```

```
                  count                       min                        median
primary_category                                                               
cs.CV              1923 2000-03-26 23:18:43+00:00 2022-01-25 15:24:37.000+00:00
cs.LG              1844 2002-12-08 18:52:33+00:00 2021-10-28 16:28:42.500+00:00
cs.CL              1040 1999-02-25 14:41:32+00:00 2022-05-01 03:57:53.500+00:00
```

## Count a density grid

For a heat map, ask for `cells`. The server divides the view into a grid with `2 ** depth` cells on each side and counts the selection in each. Up to depth 16 these are the tiles a map draws at that zoom.

``` {.python notest}
depth = 6
[grid] = cs.aggregate([{"count": True, "cells": {"depth": depth}}])
grid.to_pandas().head()
```

```
   cell  count
0   821      2
1   852      2
2   857      1
3   867      2
4   871      9
```

There is a row for each cell that holds an item. Here 291 of the 4,096 cells do. A cell's number interleaves the bits of its column and its row. The even bits are the column and the odd bits are the row. Columns are counted from the view's `x_min` and rows from its `y_min`.

``` {.python notest}
import numpy as np


def column_and_row(cell, depth):
    """Each cell's column and row: the even bits and the odd bits of its number."""
    cell = np.asarray(cell, dtype=np.int64)
    column = np.zeros_like(cell)
    row = np.zeros_like(cell)
    for bit in range(depth):
        column |= ((cell >> (2 * bit)) & 1) << bit
        row |= ((cell >> (2 * bit + 1)) & 1) << bit
    return column, row


column, row = column_and_row(grid.column("cell").to_numpy(), depth)
density = np.zeros((2**depth, 2**depth), dtype=np.int64)
density[row, column] = grid.column("count").to_numpy()

density.shape, density.sum(), density.max()
```

```
((64, 64), np.int64(10478), np.int64(298))
```

The array sums to the 10,478 papers of the selection. To draw it in the view's coordinates, give matplotlib the view's extent. `db.meta()` publishes the extent as the view's `quantisation`:

``` {.python notest}
import matplotlib.pyplot as plt

extent = next(v for v in db.meta()["views"] if v["id"] == "topics")["quantisation"]
plt.imshow(
    density,
    origin="lower",
    extent=(extent["x_min"], extent["x_max"], extent["y_min"], extent["y_max"]),
)
```

A view with a projection, such as `web_mercator`, has the projection's unit square as its extent, and its row 0 is the northern edge. Draw such a view with `origin="upper"`.

To look closer at part of the view, raise `depth` and add `area`, as in `"cells": {"depth": 8, "area": [0, 0, 10, 10]}`. The table then lists only the cells that meet that box. Each listed cell still counts all of its items, including those outside the box.

For a grid of each group, put `by` and `cells` in one grouping, as in `{"count": True, "by": {"field": "archive", "values": ["cs", "math"]}, "cells": {"depth": 6}}`. The table then has a row for each cell of each group, and `rest` rows count the other archives.

To find where a selection is concentrated, pass `reference={}`. Each cell then has a `lift`, and a lift above 1 marks a cell where the selection is denser than the view as a whole.

Not built yet: a histogram divided into cells. A grouping with `bins` and `cells` together is refused.

### Counts that come with a sample

`sample` returns the points a map would draw. It thins them so that each tile carries a bounded number, and it also reports counts for the tiles it sampled.

``` {.python notest}
drawn = cs.sample(zoom=0)
json.loads(drawn.schema.metadata[b"mosaica.counts"])
```

```
{'visible': 50000, 'matched': 10478, 'highlighted': 10478, 'served': 500}
```

`visible` counts every item the reader may see in the tiles sampled, `matched` counts those that pass the filter, and `served` counts the points returned. `highlighted` counts the matched items that also pass a `highlight` filter, and equals `matched` when none is given.

`sample(zoom, underlay_offset=n)` also returns `sub_cells`, a grid of counts `n` levels finer than the tiles. That grid ignores the filter and covers everything the reader may see, because it is meant for shading drawn under the points. For the density of a filtered selection, use `cells`.

## Where to go next

[`Selection`](../reference/python/reading.md#mosaica._viewer.Selection) and [`Viewer.aggregate`](../reference/python/reading.md#mosaica._viewer.Viewer.aggregate) document the calls used here, including how a large table is read in pages.
