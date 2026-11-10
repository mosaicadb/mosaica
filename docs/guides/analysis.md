# Count and chart from Python

When you want figures about a map rather than the map itself, ask the server for them. It counts a selection, breaks it down by a category or by a layer, bins a date or a number into a histogram, compares one set of items with another, and counts each cell of a grid for a density plot. Each answer is a small table, worked out by the server over the items the reader may see, so nothing has to be read into pandas first.

This guide makes each of those requests from the Python client, against the 50,000 arXiv papers that ship in the repository. The [filters](../reference/filters.md) and [aggregations](../reference/aggregations.md) pages of the reference list every operator and every grouping. This guide shows the ones you'll reach for first.

You'll need the repository cloned, Python 3.12 or later, and the client with pandas and NumPy. The density plot also uses matplotlib.

```bash
pip install mosaica pandas numpy matplotlib
```

## A database to count

Run this from the root of the clone. It loads the papers into one view, `topics`, with their archive, subject class, submission date and title, and adds Toponymy's named topics as a layer. [Analysing a map with Mosaica](../start/python.md#analysing-a-map-with-mosaica) explains each call.

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

A field can be counted and filtered once it is rendered or indexed, which is why the two categories and the date are rendered here and the title is indexed.

Everything below works the same on a database someone else runs. `mosaica.connect(url, token)` returns a reader with the same `view`, `aggregate` and `items`, and `db.viewer(terms)` reads your own database as someone who holds only the access terms you name. Every figure such a reader gets is counted over the items it may see.

## Count a selection

`db.view(name)` is a selection that holds a whole view. `filter` narrows it by a field and `within` by a box on the map, and each returns a new selection, so one selection can be the start of several. `count` asks the server how many items the selection holds.

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

`within` takes `(min_x, min_y, max_x, max_y)` in the view's coordinates. On a view with a projection those are the projected coordinates, and the [`region`](../reference/filters.md#region) filter takes a box in longitude and latitude.

``` {.python notest}
recent_cs.within((0, 0, 10, 10)).count()
```

```
3075
```

The [filters reference](../reference/filters.md) has the whole list, with shapes other than a box, the members of one topic, and `all_of`, `any_of` and `none_of` to combine them.

## Ask for a breakdown

`aggregate` takes a list of groupings and returns one pyarrow table for each, in the same order. A grouping says what to count and how to divide it. `{"count": True}` alone is the size of the selection, which is what `count()` sends, and adding `by` gives a row for each group.

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

`top` lists the values with the most items. The `rest` row counts the items holding any other value, and a `none` row, when there is one, the items holding no value, so the counts add up to the selection. Each table carries its totals in its schema metadata:

``` {.python notest}
import json

json.loads(classes.schema.metadata[b"mosaica.head"])
```

```
{'grouping': 1, 'groups': 37, 'total': 6429}
```

`total` is the size of the set counted and `groups` the number of values that hold at least one of its items, here the 37 subject classes that hold one of these papers.

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

Grouping by a layer counts the selection by that layer's annotations, such as the papers in each topic. A layer with levels needs the level named.

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

`none` counts the papers in no topic of this level. Each `key` is the annotation's `mosaica_id`, which differs from one database to the next, so yours will show other numbers. A `member_of` filter takes it as it is, which lets one answer feed the next question. Which archives do the papers in "AI Systems" come from?

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

An item can be in several annotations of a layer whose annotations overlap, so the rows of such a breakdown can sum to more than the selection.

## Draw a histogram

`bins` in place of `top` counts a number, a date or a true-or-false field in bins. You give the most bins you want, and the server picks readable edges that need no more than that.

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

The edges come from the smallest and largest value in the whole view, whatever the filter, so a filtered histogram has the same bins as an unfiltered one and the two can share an axis:

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

To choose the bins yourself, add `"range": [lower, upper]`. The range is then cut into `bins` equal widths, and a `rest` row counts the values outside it. `{"field": "submitted_at", "bins": 12, "range": ["2023-01-01", "2024-01-01"]}` gives twelve bins of 30 days and 10 hours each across 2023, which are equal widths and so not calendar months.

## Compare with a reference

A count says how many of a selection fall in a group. To see whether that is more or fewer than you'd expect, pass a second set as `reference`. `{}` is every item the reader may see in the view. Each row then has its count in the reference and the lift.

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

`summary` names number, date or true-or-false fields and returns how many items hold a value, how many hold none, and the smallest, largest and mean value. `min`, `max`, `mean`, `values` and `missing` ask for one of those each.

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

Until then, read the fields you need with `items` and work the figures out in pandas. This reads two fields of the 10,478 computer science papers:

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

There is a row for each cell that holds an item, 291 of the 4,096 here. A cell's number interleaves its column and its row: the even bits are the column, counted from the view's `x_min`, and the odd bits the row, counted from its `y_min`. This function separates them, and the lines after it lay the counts out as an array.

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

The array sums to the 10,478 papers of the selection. To draw it in the view's coordinates, give matplotlib the view's extent, which `db.meta()` publishes as the view's `quantisation`:

``` {.python notest}
import matplotlib.pyplot as plt

extent = next(v for v in db.meta()["views"] if v["id"] == "topics")["quantisation"]
plt.imshow(
    density,
    origin="lower",
    extent=(extent["x_min"], extent["x_max"], extent["y_min"], extent["y_max"]),
)
```

On a view with a projection, such as `web_mercator`, the extent is the projection's unit square and row 0 is the northern edge, so draw it with `origin="upper"`.

Three variations cover most density plots.

- **A finer grid over part of the view.** `"cells": {"depth": 8, "area": [0, 0, 10, 10]}` lists only the cells that meet that box. Each listed cell still counts all of its items, including those outside the box.
- **A grid for each group.** Put `by` and `cells` in one grouping, as in `{"count": True, "by": {"field": "archive", "values": ["cs", "math"]}, "cells": {"depth": 6}}`. The table then has a row for each group and cell, with `rest` rows for the other archives.
- **Where a selection is concentrated.** With `reference={}` each cell has a `lift`, which is above 1 where the selection is denser than the view as a whole.

Not built yet: a histogram divided into cells. A grouping with `bins` and `cells` together is refused.

### Counts that come with a sample

`sample` returns the points a map would draw, thinned so that each tile carries a bounded number, and it reports counts as well. They answer a different question from `cells`.

``` {.python notest}
drawn = cs.sample(zoom=0)
json.loads(drawn.schema.metadata[b"mosaica.counts"])
```

```
{'visible': 50000, 'matched': 10478, 'highlighted': 10478, 'served': 500}
```

`visible` is every item the reader may see in the tiles sampled, `matched` those that pass the filter, and `served` the points returned. `sample(zoom, underlay_offset=n)` also returns `sub_cells`, a grid of counts `n` levels finer than the tiles. That grid ignores the filter and covers everything the reader may see, which suits shading drawn under the points. For the density of a filtered selection, use `cells`.

## Where to go next

- The [aggregations reference](../reference/aggregations.md) lists every grouping and figure, the columns of each table and what is refused.
- The [filters reference](../reference/filters.md) lists every operator for each type of field.
- [`Selection`](../reference/python/reading.md#mosaica._viewer.Selection) and [`Viewer.aggregate`](../reference/python/reading.md#mosaica._viewer.Viewer.aggregate) document the calls used here, including how a large table is read in pages.
