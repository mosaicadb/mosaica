# Your first map in Python

This tutorial is for a Python user who has never used Mosaica. We'll take 150,000 places from GeoNames, a free gazetteer of the world's place names, read them into a pandas data frame, and turn the frame into an interactive map in a notebook. Then we'll ask the same data some questions from Python: which places have a million people or more, which have "San" in their name, how many places there are of each kind, and where one misfiled village really is.

On the way you'll meet the three steps every Mosaica database is made with: declaring what the data holds, inserting rows, and committing them. You'll also see what each declaration costs and what it buys.

It takes about ten minutes. You need Python 3.10 or later and a clone of the Mosaica repository, which holds the data. The whole tutorial is also a notebook in the clone, `clients/py/examples/01_getting_started.ipynb`, with a marimo version beside it, so you can run it there instead of pasting each block.

1. Install the `mosaica` package.
2. Read the places into pandas.
3. Make a database, declare its columns, insert the places and commit them.
4. Draw the map, then filter and search it.
5. Count the places by kind.
6. Find a place GeoNames files under the wrong region.

## Install Mosaica

Clone the repository, which holds the data:

```bash
git clone https://github.com/mosaicadb/mosaica ~/mosaica
```

In a virtual environment, install the package with the notebook map, pandas and JupyterLab:

--8<-- "clients/py/examples/01_getting_started.py:install"

`mosaica` is the Python package. It brings the Mosaica server with it, compiled for your platform, so you don't need Rust or a separate install. The `[widget]` part adds the map.

!!! note ""
    Not published yet: the `mosaica` package on PyPI. Until it is, install it from a clone of the repository, with `mosaica-native`, the package that carries the server, first. That needs Rust, a C compiler and Node.js 22.15 or later. [Install and configure](install.md) has the details.

    ```bash
    pip install ~/mosaica/clients/py-native
    pip install "$HOME/mosaica/clients/py[widget]" pandas jupyterlab
    ```

Start JupyterLab with `jupyter lab`, open a new notebook, and paste each block below into a cell of its own.

## Read the places

The places are in `examples/data/geonames/` in the clone. GeoNames lists over 13 million features. This extract keeps every town and city of 15,000 people or more, and a sample of everything else, from lakes and farms to undersea ridges. Point a variable at the directory first.

```{.python notest}
from pathlib import Path

geonames = Path("~/mosaica/examples/data/geonames").expanduser()
```

Then read the places into pandas. JupyterLab shows the first rows as a table, which looks like this.

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:read"
```

```
   geonameid       lon       lat                          name feature_class feature_code country          division  population country_code
0         61   48.0226   32.0683                  Rasan Dakhām    settlement        P.PPL    Iran   Khuzestan, Iran        <NA>           IR
1        171  48.93378  32.42393                     Gol-e Bīd      building       S.CRRL    Iran   Khuzestan, Iran        <NA>           IR
2        213  48.92843  32.61583  Rūdkhāneh-ye Faşlī-ye Pānsār         water       H.STMI    Iran   Khuzestan, Iran        <NA>           IR
3        305  51.80313  35.94511            Kūh-e Yowrd Qeydar      mountain         T.MT    Iran  Māzandarān, Iran        <NA>           IR
4        349     52.05     35.65         Soltān Darreh-ye Bālā    settlement        P.PPL    Iran      Tehran, Iran        <NA>           IR
```

Each row is one place: its GeoNames id, its longitude and latitude, its name, the kind of feature it is, its country, the region it is filed under, its population and its country's code. `feature_class` holds one of nine words, such as `settlement`, `water` or `mountain`, except for 47 places GeoNames gives no class. `division` is the place's first-level division, such as a state or a province, followed by its country, because division names repeat across countries. Most places have no population figure, and pandas shows `<NA>` for them.

`dtype_backend="numpy_nullable"` asks pandas for its types that allow a missing value. Without it, the gaps would turn `population` into a column of floats.

## Make a database

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:create"
```

`create()` with no argument makes the database in a temporary directory, which is deleted when Python exits, or by the next `create()` if the process ended without cleaning up. Pass a path, as in `mosaica.create("places-db")`, to keep it.

## Declare what it holds

A Mosaica database holds items, which here are places, and one or more views. A view is a layout of the items on a map, with a coordinate for each one. We declare one view, `world`, and then the columns.

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:declare"
```

```
declared 10 columns: geonameid (i64), lon (f64), lat (f64), name (text, index), feature_class (category, render), feature_code (text), country (category), division (category, index), population (i64, index), country_code (text)
and 3 open vocabularies, public, so every reader may list their values: feature_class, country, division
```

`projection="web_mercator"` says the view's coordinates are longitude and latitude, to be drawn with the projection web maps use. Without it, a view takes plain x and y coordinates, such as the output of UMAP or t-SNE, and draws them as they are.

`declare_columns` reads the data frame's column types, not its values, and declares every column. `category` says which string columns are categories: `feature_class`, `country` and `division` each hold a short list of values repeated across many rows, and a category stores each value once, however many places hold it. The other string columns are declared as text, `geonameid` and `population` as 64-bit integers, and `lon` and `lat` as 64-bit floats.

Every declared column is stored, and shown when someone opens a place. A column can also be **rendered**: its value is sent with every point drawn, so the map can colour or size points by it, and it can be filtered and, if it is a category, counted. Or it can be **indexed**: Mosaica builds an index on it, so it can be filtered, and a category counted, without travelling with the points. A column that is neither rendered nor indexed is a **detail**. `country` is a category and a detail, so it is shown when a place is opened, but the map can't be filtered or counted by it.

Each has a cost. A column can only be made rendered before the first commit, but a rendered column makes every point the server sends larger, so render only what the map needs. Here that is the feature class, for colour; it can then be filtered and counted by as well, without an index. An index takes disc and build time, so index the columns people will filter on. We index `name` to search it, `population` to filter on a range, and `division` to filter and count by it. The rest are details, which cost least.

The report lists every column and what it was declared as. Its second line is about the categories. Each category takes its values from a vocabulary, the list of values it can hold. An open vocabulary grows as new values arrive. Public means anyone who can see the map may see the list.

## Insert the places

Inserting hands a table to something declared and names the columns it reads. The view needs a longitude and a latitude for each place.

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:insert"
```

```
world: 150,000 rows (read geonameid, lon, lat, name, feature_class, feature_code, country, division, population, country_code)
```

The other columns are read too, because they have the names of declared columns. The report lists every column read, and would list any it ignored.

Nothing has been sent anywhere yet. Inserts collect until you commit.

## Commit

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:commit"
```

```
built 150,000 items from world: 150,000 rows
  1 view: world
  in 2.1 s; serving at 127.0.0.1:45843
  view 'world' (rows): read geonameid, lon, lat, name, feature_class, feature_code, country, division, population, country_code; ignored nothing
```

The first commit builds the database. It places every item on the view's grid and stores neighbouring places next to each other on disc, so that drawing one part of the map reads one stretch of the file. Then it starts a Mosaica server for the database on a free local port. Yours will be a different one.

From now on the notebook talks to that server, the same way a browser or another program would. A later commit sends its rows to the running server, which adds them without a rebuild.

## Draw the map

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:map"
```

The map draws every place, coloured by feature class. Drag to pan and scroll to zoom. The map aims to draw up to 250,000 points, so all 150,000 places are drawn even with the whole world in view. With more items than that, the map draws a sample at a wide zoom, chosen so that dense and sparse areas both show, and fills in more as you zoom in.

The panel on the left counts the places in view. Its Feature class card lists the five largest classes with a count for each, and "4 more in view" opens the rest. Once you zoom in, each row shows two counts: the places in view, then after a slash the places on the whole map. Hover over a class and two buttons take the place of its counts. The highlighter picks out that class's places on the map and dims the rest, and the funnel filters the map to that class. Click the coloured square beside a class to change its colour.

## Filter from Python

The filters you click on the map can be written in Python too. `db.view("world")` is a selection, which is the view as you see it, and `filter` narrows it.

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:filter"
```

```
613
```

613 places have a population of a million or more. A filter is a dictionary from a column to a test. `range` takes bounds named `gt`, `gte`, `lt` and `lte`, and works on any indexed number column.

The same selection draws as a map.

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:filter-map"
```

Most are cities. GeoNames gives regions and metropolitan areas a population too, so Tokyo Prefecture and the San Francisco-Oakland-Berkeley metro area are among them, in other colours. The filter appears as a chip in the panel. Remove the chip to see every place again.

## Search by name

Text columns are searched by word with `match`.

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:search"
```

```
1310
```

1,310 places have "san" as a word in their name, such as San Diego and Isla San Martín. `match` ignores case, and finds whole words, so Santiago and Santa Cruz are not among them.

## Count by kind of place

`aggregate` counts how items are distributed. Here we group every place by its feature class.

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:counts"
```

```
    group         key  count
0  listed  settlement  78724
1  listed       water  22791
2  listed    building  21978
3  listed    mountain  15848
4  listed      region   4595
5  listed        area   4308
6  listed      forest   1089
7  listed        road    483
8  listed    undersea    137
9    none         NaN     47
```

`aggregate` takes a list of groupings and returns one table for each, which is why we unpack a list of one. A grouping says what to group by and what to work out for each group. Here `by` groups by feature class, `top` is how many values to list, the largest first, and `"count": True` asks for the number of places in each. The answer is an Arrow table, like every table Mosaica returns, and `to_pandas()` turns it into a data frame.

Settlements are the largest class, then water, then buildings. The last row, in the group `none`, counts the 47 places GeoNames gives no class. While the whole world is in view, the map's Feature class card shows the same numbers for the nine classes, because it makes the same request over the places in view.

## Find a misfiled place

East Nusa Tenggara is a chain of islands at the south-eastern end of Indonesia. Filter to it, count it and draw it. `eq` keeps the places whose value is exactly the one given.

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:outlier"
```

```
314
```

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:outlier-map"
```

The map opens framed on all 314 places. The islands of East Nusa Tenggara are the cluster at the bottom right, east of longitude 118°. One point sits on its own at the top left, far to the west of them. To find out which place it is, read the region's places into pandas with their longitude and latitude, and sort them from west to east.

```{.python notest}
--8<-- "clients/py/examples/01_getting_started.py:outlier-find"
```

```
              mosaica_id  geonameid        name        lon      lat
0   12625214655987642250    7612808       Rokot   99.69063 -2.08728
39   8495083937813982874    7411484  Galluwawai  119.02320 -9.58230
38  14897736624976177688    7411544  Ngikahanga  119.02580 -9.55810
40  11580208477392569200    7556109      Kanaka  119.03400 -9.65290
43  14414396836725556075    7556480    Radamata  119.20070 -9.54830
```

`items` reads every place in the selection with the columns you name. `lon` and `lat` are there because we declared them as details. `mosaica_id` is the id Mosaica gives each item, and yours will differ, because every database scrambles them with a key of its own.

The westernmost place is Rokot, at longitude 99.7°, a village in the Mentawai Islands off West Sumatra. The next is nearly 20° further east. GeoNames files Rokot under East Nusa Tenggara, and its `geonameid` is what you'd take back to GeoNames to correct the record.

## What you built

You read a file into pandas, declared its columns, inserted it and committed it. That gave you a Mosaica database and a server answering for it. You drew it as a map, filtered it by population and by region, searched it by name, counted it by class, and found a misfiled village, all through that server.

The decisions that carry over to your own data are the declarations: which columns to render, which to index and which to leave as details, and whether your coordinates are longitude and latitude or a plain layout.

## Next

- The [Python client reference](../reference/python/index.md) covers every call, from `create()` to the notebook map.
- [Install and configure](install.md) shows the two files the client wrote for you, and how to serve the same database without Python.
- [The data model](../system/data-model.md) says more about views, fields and vocabularies.
