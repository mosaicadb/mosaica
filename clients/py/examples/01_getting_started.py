# Mosaica's first tutorial, as a marimo notebook: `marimo edit clients/py/examples/01_getting_started.py`.
#
# `01_getting_started.ipynb` beside it is generated from this file, and is not edited by hand:
#
#     marimo export ipynb clients/py/examples/01_getting_started.py -o clients/py/examples/01_getting_started.ipynb
#
# The docs site includes the code between each pair of `--8<--` markers.

import marimo

__generated_with = "0.25.2"
app = marimo.App(width="medium")


@app.cell(hide_code=True)
def _():
    try:
        import marimo as mo
    except ImportError:
        mo = None
    return (mo,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    # Your first map in Python

    This notebook is for a Python user who has never used Mosaica. We'll take 150,000 places
    from GeoNames, a free gazetteer of the world's place names, read them into a pandas data
    frame, and turn the frame into an interactive map in the notebook. Then we'll ask the same
    data some questions from Python: which places have a million people or more, which have
    "San" in their name, how many places there are of each kind, and where one misfiled village
    really is.

    On the way you'll meet the three steps every Mosaica database is made with: declaring what
    the data holds, inserting rows, and committing them. You'll also see what each declaration
    costs and what it buys.

    It takes about ten minutes. You need Python 3.10 or later and a clone of the Mosaica
    repository, which holds the data.

    1. Install the `mosaica` package.
    2. Read the places into pandas.
    3. Make a database, declare its columns, insert the places and commit them.
    4. Draw the map, then filter and search it.
    5. Count the places by kind.
    6. Find a place GeoNames files under the wrong region.

    ## Install Mosaica

    In a virtual environment, install the package with the notebook map, pandas and JupyterLab:

    <!-- --8<-- [start:install] -->
    ```bash
    pip install "mosaica[widget]" pandas jupyterlab
    ```
    <!-- --8<-- [end:install] -->

    `mosaica` is the Python package. It brings the Mosaica server with it, compiled for your
    platform, so you don't need Rust or a separate install. The `[widget]` part adds the map.

    Until `mosaica` is published, install it from a clone of the repository instead, with
    `mosaica-native`, the package that carries the server, first. That needs Rust and Node.js
    22.15 or later:

    ```bash
    git clone https://github.com/mosaicadb/mosaica
    pip install ./mosaica/clients/py-native
    pip install "./mosaica/clients/py[widget]" pandas jupyterlab
    ```

    ## Read the places

    The places are in `examples/data/geonames/` in the repository. GeoNames lists over 13
    million features; this extract keeps every town and city of 15,000 people or more, and a
    sample of everything else, from lakes and farms to undersea ridges. The next cell finds the
    directory from where this notebook is.
    """)
    return


@app.cell
def _():
    # The data is in examples/data/geonames/ at the top of the repository. Jupyter runs a
    # notebook in its own directory, and marimo gives the notebook's path as __file__.
    from pathlib import Path

    here = Path(__file__).parent if "__file__" in globals() else Path.cwd()
    geonames = here.resolve().parents[2] / "examples" / "data" / "geonames"
    return (geonames,)


@app.cell
def _(geonames):
    # --8<-- [start:read]
    import pandas as pd

    places = pd.read_parquet(geonames / "points.parquet", dtype_backend="numpy_nullable")
    places.head()
    # --8<-- [end:read]
    return (places,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Each row is one place: its GeoNames id, its longitude and latitude, its name, the kind of
    feature it is, its country, the region it is filed under, its population and its country's
    code. `feature_class` holds one of nine words, such as `settlement`, `water` or `mountain`.
    `division` is the place's first-level division, such as a state or a province, followed by
    its country, because division names repeat across countries. Most places have no population
    figure, and pandas shows `<NA>` for them.

    `dtype_backend="numpy_nullable"` asks pandas for its types that allow a missing value.
    Without it, the gaps would turn `population` into a column of floats.

    ## Make a database
    """)
    return


@app.cell
def _():
    # --8<-- [start:create]
    import mosaica

    db = mosaica.create()
    # --8<-- [end:create]
    return (db,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    `create()` with no argument makes the database in a temporary directory, which is deleted
    when the notebook's kernel stops. Pass a path, as in `mosaica.create("places-db")`, to keep
    it.

    ## Declare what it holds

    A Mosaica database holds items, which here are places, and one or more views. A view is a
    layout of the items on a map, with a coordinate for each one. We declare one view, `world`,
    and then the columns.
    """)
    return


@app.cell
def _(db, places):
    # --8<-- [start:declare]
    db.declare_view("world", projection="web_mercator")
    db.declare_columns(
        places,
        category=["feature_class", "country", "division"],
        render=["feature_class"],
        index=["name", "feature_class", "division", "population"],
    )
    # --8<-- [end:declare]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    `projection="web_mercator"` says the view's coordinates are longitude and latitude, to be
    drawn with the projection web maps use. Without it, a view takes plain x and y
    coordinates, such as the output of UMAP or t-SNE, and draws them as they are.

    `declare_columns` reads the data frame's column types, not its values, and declares every
    column. A column is a detail unless `render`, `index` or `category` names it. We name
    `feature_class`, `country` and `division` as categories. Each holds a short list of values
    repeated across many rows, and a category is what the map colours by and counts by. The
    other string columns are declared as text, `geonameid` and `population` as 64-bit
    integers, and `lon` and `lat` as 64-bit floats.

    Every declared column is stored, and shown when someone opens a place. A column can also be
    **rendered**: its value is sent with every point drawn, so the map can colour or size
    points by it. It can be **indexed**: Mosaica builds an index on it, so it can be filtered
    and counted.

    Each has a cost. A rendered column makes every point the server sends larger, so render
    only what the map needs, which here is the feature class for colour. An index takes disk
    and build time, so index the columns people will filter on. We index `name` to search it,
    `population` to filter on a range, and `feature_class` and `division` to filter and count by
    them. The rest are details, which cost least.

    The report under the cell lists every column and what it was declared as. Its second line
    is about the categories. Each category takes its values from a vocabulary, the list of
    values it can hold. An open vocabulary grows as new values arrive. Public means anyone who
    can see the map may see the list.

    ## Insert the places

    Inserting hands a table to something declared and names the columns it reads. The view
    needs a longitude and a latitude for each place.
    """)
    return


@app.cell
def _(db, places):
    # --8<-- [start:insert]
    db.insert("world", places, lon="lon", lat="lat")
    # --8<-- [end:insert]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The other columns are read too, because they have the names of declared columns. The report
    lists every column read, and would list any it ignored.

    Nothing has been sent anywhere yet. Inserts collect until you commit.

    ## Commit
    """)
    return


@app.cell
def _(db):
    # --8<-- [start:commit]
    db.commit()
    # --8<-- [end:commit]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The first commit builds the database. It places every item on the view's grid and stores
    neighbouring places next to each other on disk, so that drawing one part of the map reads
    one stretch of the file. Then it starts a Mosaica server for the database on a free local
    port.

    From now on the notebook talks to that server, the same way a browser or another program
    would. A later commit sends its rows to the running server, which adds them without a
    rebuild.

    ## Draw the map
    """)
    return


@app.cell
def _(db):
    # --8<-- [start:map]
    db.map(colour_by="feature_class")
    # --8<-- [end:map]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The map draws every place, coloured by feature class. Drag to pan and scroll to zoom, and
    more places appear as you zoom in. At a wide zoom the map draws a sample of the points,
    chosen so that dense and sparse areas both show, and the server sends only the points in
    view.

    The panel on the left counts the places in view. Open the Feature class card with the arrow
    on its right, and it lists the classes with a count for each, largest first. Hover over a
    class and two buttons appear beside it. The highlighter picks out that class's places on the
    map and dims the rest, and the funnel filters the map to that class. Click the coloured
    square beside a class to change its colour.

    ## Filter from Python

    The filters you click on the map can be written in Python too. `db.view("world")` is a
    selection, which is the view as you see it, and `filter` narrows it.
    """)
    return


@app.cell
def _(db):
    # --8<-- [start:filter]
    big = db.view("world").filter({"population": {"range": {"gte": 1_000_000}}})
    big.count()
    # --8<-- [end:filter]
    return (big,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    613 places have a population of a million or more. A filter is a dictionary from a column
    to a test. `range` takes bounds named `gt`, `gte`, `lt` and `lte`, and works on any indexed
    number column.

    The same selection draws as a map.
    """)
    return


@app.cell
def _(big):
    # --8<-- [start:filter-map]
    big.map(colour_by="feature_class")
    # --8<-- [end:filter-map]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Most are cities. GeoNames gives regions and metropolitan areas a population too, so Tokyo
    Prefecture and the San Francisco-Oakland-Berkeley metro area are among them, in other
    colours. The filter appears as a chip in the panel. Remove the chip to see every place
    again.

    ## Search by name

    Text columns are searched by word with `match`.
    """)
    return


@app.cell
def _(db):
    # --8<-- [start:search]
    db.view("world").filter({"name": {"match": "san"}}).count()
    # --8<-- [end:search]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    1,310 places have "san" as a word in their name, such as San Diego and Isla San Martín.
    `match` ignores case, and finds whole words, so Santiago and Santa Cruz are not among them.

    ## Count by kind of place

    `aggregate` counts how items are distributed. Here we group every place by its feature
    class.
    """)
    return


@app.cell
def _(db):
    # --8<-- [start:counts]
    (by_class,) = db.aggregate("world", [{"by": {"field": "feature_class", "top": 9}, "count": True}])
    by_class.to_pandas()
    # --8<-- [end:counts]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    `aggregate` takes a list of groupings and returns one table for each, which is why we unpack
    a list of one. A grouping says what to group by and what to work out for each group. Here
    `by` groups by feature class, `top` is how many values to list, the largest first, and
    `"count": True` asks for the number of places in each. The answer is an Arrow table, like
    every table Mosaica returns, and `to_pandas()` turns it into a data frame.

    Settlements are the largest class, then water, then buildings. The last row, in the group
    `none`, counts the 47 places GeoNames gives no class. The `title` column holds a display
    name for each value where one was declared, and we declared none. These are the numbers the
    map's Feature class card showed, because the card makes the same request.

    ## Find a misfiled place

    East Nusa Tenggara is a chain of islands at the south-eastern end of Indonesia. Filter to
    it, count it and draw it.
    """)
    return


@app.cell
def _(db):
    # --8<-- [start:outlier]
    nusa_tenggara = db.view("world").filter({"division": {"eq": "East Nusa Tenggara, Indonesia"}})
    nusa_tenggara.count()
    # --8<-- [end:outlier]
    return (nusa_tenggara,)


@app.cell
def _(nusa_tenggara):
    # --8<-- [start:outlier-map]
    nusa_tenggara.map(colour_by="feature_class")
    # --8<-- [end:outlier-map]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The map opens on the whole world, where the region is a small cluster of points. Scroll to
    zoom in on it. The islands of East Nusa Tenggara lie east of longitude 118°, but one point
    sits on its own far to the west of them. To find out which place it is, read the region's
    places into pandas with their longitude and latitude, and sort them from west to east.
    """)
    return


@app.cell
def _(nusa_tenggara):
    # --8<-- [start:outlier-find]
    region = nusa_tenggara.items(["geonameid", "name", "lon", "lat"]).to_pandas()
    region.sort_values("lon").head()
    # --8<-- [end:outlier-find]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    `items` reads every place in the selection with the columns you name. `lon` and `lat` are
    there because we declared them as details. `mosaica_id` is the id Mosaica gives each item.

    The westernmost place is Rokot, at longitude 99.7°, a village in the Mentawai Islands off
    West Sumatra. The next is nearly 20° further east. GeoNames files Rokot under East Nusa
    Tenggara, and its `geonameid` is what you'd take back to GeoNames to correct the record.

    ## What you built

    You read a file into pandas, declared its columns, inserted it and committed it. That gave
    you a Mosaica database and a server answering for it. You drew it as a map, filtered it by
    population and by region, searched it by name, counted it by class, and found a misfiled
    village, all through that server.

    The decisions that carry over to your own data are the declarations: which columns to
    render, which to index and which to leave as details, and whether your coordinates are
    longitude and latitude or a plain layout.
    """)
    return


if __name__ == "__main__":
    app.run()
