# Mosaica's second tutorial, as a marimo notebook: `marimo edit clients/py/examples/02_analysing_a_map.py`.
#
# `02_analysing_a_map.ipynb` beside it is generated from this file, and is not edited by hand:
#
#     marimo export ipynb clients/py/examples/02_analysing_a_map.py -o clients/py/examples/02_analysing_a_map.ipynb

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
    # Analysing a map with Mosaica

    This notebook is for a data scientist who has a corpus of documents and an analysis of
    their own, and wants to look at the two together. We'll use 50,000 arXiv papers that come
    with two layouts and a set of named topics, cluster the papers ourselves with scikit-learn,
    and then put our clusters and the published topics on the same map. The counts, histograms
    and comparisons in steps 3 and 4 are worked out by the Mosaica server, which holds the
    papers, and pandas only receives the answers.

    By the end you'll know how to give a database several layouts of the same items, how to
    attach clusterings and their names as layers, and how to ask the server how one clustering
    relates to another. It takes about fifteen minutes. If you haven't met Mosaica before, the
    first tutorial, `01_getting_started`, explains declaring, inserting and committing more
    slowly, with places on a world map.

    1. Load the papers into two views, one per layout, with arXiv's names for its archives and
       subject classes.
    2. Add the sample's named topics as a layer on both views.
    3. Cluster the papers with HDBSCAN, add the clusters as a second layer, and compare one of
       them with the topics.
    4. Ask the server how the papers are distributed: by topic, by year, and by archive among
       the papers about graphene.
    5. Combine a title search, a date range and a topic, draw the result in both views side by
       side, and read the matching papers into pandas.

    ## Install

    You need Python 3.12 or later. In a virtual environment, install the client with the
    notebook map, pandas and scikit-learn. In JupyterLab:

    ```bash
    pip install 'mosaica[widget]' pandas scikit-learn jupyterlab
    ```

    In marimo:

    ```bash
    pip install 'mosaica[widget]' pandas scikit-learn marimo
    ```

    ## Read the papers

    The papers are a sample of 50,000 from arXiv's listings, kept in `examples/data/arxiv/` in
    the Mosaica repository. Each has a position in two dimensions computed from an embedding of
    the paper, so that papers on similar subjects sit close together. The next cell uses the
    repository's copy when this notebook runs from a clone of it, and otherwise downloads the
    seven files it needs from GitHub into a folder called `arxiv` where you started the
    notebook, the first time only.
    """)
    return


@app.cell
def _():
    # Jupyter runs a notebook in its own directory, and marimo gives the notebook's path as
    # __file__.
    from pathlib import Path
    from urllib.request import urlretrieve

    here = Path(__file__).parent if "__file__" in globals() else Path.cwd()
    arxiv = (here / "../../../examples/data/arxiv").resolve()
    if not arxiv.is_dir():
        arxiv = Path("arxiv")
        arxiv.mkdir(exist_ok=True)
        for _name in ["points", "points-umap200", "archive", "primary_category",
                      "clusters-toponymy", "clusters-toponymy-members", "topics-toponymy"]:
            if not (arxiv / f"{_name}.parquet").exists():
                urlretrieve("https://raw.githubusercontent.com/mosaicadb/mosaica/main/"
                            f"examples/data/arxiv/{_name}.parquet", arxiv / f"{_name}.parquet")
    return (arxiv,)


@app.cell
def _(arxiv):
    import pandas as pd

    papers = pd.read_parquet(arxiv / "points.parquet")
    papers.head()
    return papers, pd


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Each row is one paper. Most of the columns are arXiv's own data about it: `arxiv_id`, the
    paper's arXiv identifier; `archive`, the part of arXiv it was submitted to, such as `cs` or
    `math`; `primary_category`, its subject class within that archive, such as `math.FA` for
    functional analysis; `submitted_at`, the date of submission; and `title`. `categories`
    lists every class the paper is filed under, which we won't use here.

    Two things in the file are there for Mosaica. `entity_id` is an identifier, which every
    other file uses to say which paper it means. `x` and `y` are the paper's position in the
    first layout, which is what a map draws.

    The second layout is in a file of its own. It carries only the identifier and a second
    position, computed with different settings.
    """)
    return


@app.cell
def _(arxiv, pd):
    wide = pd.read_parquet(arxiv / "points-umap200.parquet", columns=["entity_id", "x", "y"])
    wide.head()
    return (wide,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Both layouts were made with UMAP, which places each paper by looking at its nearest
    neighbours in the embedding. The first used 15 neighbours, which keeps small groups of
    closely related papers together and so breaks subjects into many small islands. The second
    used 200, which pulls whole fields into broad regions. The first is better for finding a
    niche, and the second for seeing how fields sit against each other, which is why we keep
    both.

    ## 1. Two views of the same papers

    A Mosaica database holds items, here papers, and one or more views. A view is one layout
    of the items, and a map draws one view. `create()` makes the database in a temporary
    directory, which is deleted when Python exits.
    """)
    return


@app.cell
def _():
    import mosaica

    db = mosaica.create()
    return db, mosaica


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The next cell declares what the database holds, starting with a view for each layout.
    Their titles are what a map shows in its view menu.

    Then come the columns. `archive` and `primary_category` are categories: each holds one of
    a short list of values. That list is a **vocabulary**. arXiv publishes its own, with a name
    for every code, such as "Functional Analysis" for `math.FA`. Declaring the two vocabularies ourselves, with those names as titles, means maps
    and tables show the names. `closed=True` refuses a value the list doesn't hold, which
    catches a typo in the data.
    """)
    return


@app.cell
def _(db, papers):
    db.declare_view("topics", title="arXiv, UMAP with 15 neighbours")
    db.declare_view("topics-wide", title="arXiv, UMAP with 200 neighbours")
    db.declare_vocabulary("archive", closed=True, title="Archive")
    db.declare_vocabulary("primary_category", closed=True, title="Subject class")
    db.declare_columns(
        papers,
        skip=["x", "y", "categories"],
        category=["archive", "primary_category"],
        keyword=["arxiv_id"],
        unique=["entity_id"],
        render=["archive", "primary_category", "submitted_at"],
        index=["title"],
    )
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    `declare_columns` declares a column for each column of the frame from its type, except the
    ones `skip` names. The two categories read the vocabularies we just declared, because they
    have the same names.

    `keyword` is for structured values such as identifiers, kept whole where a plain string
    column would be text, split into words to search. An indexed keyword column can be filtered
    by exact value, by prefix or by a part of the value. We leave `arxiv_id` unindexed, so it is
    stored and shown on a paper's card. `unique` says no two papers
    share an `entity_id`, which makes it the papers' own identifier. Any table we insert later
    that carries an `entity_id` column names its papers by it, which is how the second layout
    gives a second position to the same papers rather than adding 50,000 more, and how every
    clustering below finds its papers.

    The last two arguments decide what each column costs and what it can do. A **rendered**
    column travels with every point the server sends to a map. The map can then colour points
    by it and filter on it, and the server can count it, at the price of a larger point. An
    **indexed** column gets an index instead, so it can be filtered and counted without
    travelling with the points, at the price of disc and build time. A column that is neither is a detail,
    stored and shown when someone opens a paper. Here we render the two categories and the
    submission date, which we'll colour, bin and filter by, and index the title to search it.
    Being rendered, the categories and the date need no index to be filtered or counted.

    The report lists every column and what it was declared as.

    ### Insert and commit

    The vocabularies take their values from two small files beside the papers, one for the
    archives and one for the subject classes.
    """)
    return


@app.cell
def _(arxiv, pd):
    pd.read_parquet(arxiv / "archive.parquet").head()
    return


@app.cell
def _(arxiv, pd):
    pd.read_parquet(arxiv / "primary_category.parquet").head()
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Both are in the shape Mosaica reads a vocabulary in. `key` is a value as the papers write
    it, such as `astro-ph`. `code` is the small integer Mosaica stores on each paper in place
    of the key, given here so that every database made from these files numbers the values the
    same way. `title` is the name maps and tables show. The file lists all 38 archives,
    including ones arXiv has since retired, which older papers still carry.

    A vocabulary and the category column reading it share a name, so `key="key"` says the
    table is for the vocabulary. The views take their positions from the frames, `x` and `y`
    naming the columns that hold them.
    """)
    return


@app.cell
def _(arxiv, db, papers, wide):
    db.insert("archive", arxiv / "archive.parquet", key="key")
    db.insert("primary_category", arxiv / "primary_category.parquet", key="key")
    db.insert("topics", papers, x="x", y="y")
    db.insert("topics-wide", wide, x="x", y="y")
    db.commit()
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Each insert read the columns it names and the ones with the names of declared columns. A
    path to a Parquet file is read where it lies, without a copy.

    The commit built the database and started a server for it on a free local port, and from
    here on the notebook asks that server, the same way a browser would. The papers' columns
    were read from the `topics` frame, because it is the first view declared. The
    `topics-wide` frame only gives each paper, named by its `entity_id`, a second position.

    ### Draw the two layouts

    Here are both views, coloured by archive. The map aims to draw up to 250,000 points, so
    every paper is drawn in each. `title_field="title"` titles each paper by its title when you
    hover over it or open it.
    """)
    return


@app.cell
def _(db):
    db.map(view="topics", colour_by="archive", title_field="title")
    return


@app.cell
def _(db):
    db.map(view="topics-wide", colour_by="archive", title_field="title")
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The first map scatters each archive over many islands, and the second gathers it into a
    few large regions. The panel beside each map counts the papers in view by archive, with
    arXiv's names for them. Click a point to open its paper's record.

    ## 2. The sample's named topics

    The sample comes with a clustering made by Toponymy, a library that clusters documents in
    several levels, from broad to fine, and asks a language model to name each cluster from the
    titles of a few of its papers. We use three of the sample's files, holding the clusters, the
    papers in each, and the names.

    In Mosaica a clustering is a **layer**, a set of **annotations** over the items. Each
    annotation has a key and holds some of the items. Mosaica's calls name an annotation an artifact, as in the
    `artifacts=` argument below. A layer whose annotations come in fixed levels, each finer
    level splitting the one above, is `tiered`. The names are a **label set** of that layer,
    which gives each annotation a name.

    Here are the three files, starting with the clusters.
    """)
    return


@app.cell
def _(arxiv, pd):
    pd.read_parquet(arxiv / "clusters-toponymy.parquet").head()
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    This file is in the shape Mosaica reads a layer's annotations in. `key` names each
    cluster, `level` is its level from 0, the broadest, to 3, and `parent` is the key of the
    cluster one level up that holds it, empty at level 0 and for clusters Toponymy left
    without one.

    Next, which papers each cluster holds.
    """)
    return


@app.cell
def _(arxiv, pd):
    pd.read_parquet(arxiv / "clusters-toponymy-members.parquet").head()
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    One row per paper in a cluster, so a paper appears once at each level that holds it.
    `level` and `key` name the cluster, and `entity` is the paper's identifier, the
    `entity_id` of the points file under another name.

    Last, the names.
    """)
    return


@app.cell
def _(arxiv, pd):
    pd.read_parquet(arxiv / "topics-toponymy.parquet").head()
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Each row names one cluster. `key` is the name's own key, and `level` is 0, since a label
    set is a layer of one level. `contents` holds the texts: first the name, then a few
    keyphrases. The three `attached_` columns say which cluster the name belongs to: its
    layer, here `clusters/toponymy`, its level and its key.

    We name the layer and its labels as the files name them, `clusters/toponymy` and
    `topics/toponymy`, because `attached_layer` says which layer each name belongs to.
    """)
    return


@app.cell
def _(arxiv, db):
    db.declare_layer(
        "clusters/toponymy",
        kind="tiered",
        levels=[
            (0, "16 topics", (0, 3)),
            (1, "46 topics", (4, 5)),
            (2, "161 topics", (6, 7)),
            (3, "574 topics", (8, 16)),
        ],
        title="Toponymy topics",
    )
    db.declare_labels("topics/toponymy", of="clusters/toponymy", title="Topic names")
    db.insert("clusters/toponymy", artifacts=arxiv / "clusters-toponymy.parquet")
    db.insert(
        "clusters/toponymy",
        members=arxiv / "clusters-toponymy-members.parquet",
        columns={"entity_id": "entity"},
    )
    db.insert("topics/toponymy", arxiv / "topics-toponymy.parquet")
    db.commit()
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The inserts read the files' columns by their names and need no other argument. The one
    exception is the paper's identifier, which the members file calls `entity`:
    `columns={"entity_id": "entity"}` says that column holds `entity_id`.

    The third value of each level is the range of map zooms the server serves the level at,
    from 0, the whole map, to 16, so the map draws the 16 broad topics when you see everything
    and finer ones as you zoom in. Without ranges every level is served at every zoom, and the
    map draws the broadest.

    The report counts 797 topics across the four levels, a name for each, and 124,108
    memberships, one for each paper in each topic that holds it. A layer declared without
    `views=` is drawn on every view, so the topics are on both maps. This commit didn't rebuild
    anything. It sent the layer to the running server, which added it while it went on
    answering.

    Let's draw the wide view coloured by topic, with the names.
    """)
    return


@app.cell
def _(db):
    db.map(
        view="topics-wide",
        colour_by="cluster:clusters/toponymy",
        layers=["topics/toponymy"],
        title_field="title",
    )
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    `colour_by="cluster:clusters/toponymy"` colours each paper by the topic it is in, and draws
    the papers in no topic in grey. `layers` draws the names over the map, along with the
    clusters they name. At the widest zoom you see the 16 broad topics, and zooming in brings in
    the finer levels. Click a name to open its topic's card, which counts its papers, lists the
    topics above and inside it, and has buttons to filter or highlight the map to it.

    ## 3. Your own clusters

    Now for an analysis of our own. The sample ships no embeddings, only the two layouts, so we
    cluster a layout, and we take the wide one, because Toponymy clustered the first and
    clustering it again would mostly find Toponymy's clusters. scikit-learn's HDBSCAN finds
    clusters as dense regions of points, and leaves points in sparse areas in none.

    `min_cluster_size` is the smallest group it will call a cluster, and `min_samples` how dense
    a neighbourhood has to be. `cluster_selection_method="leaf"` keeps the finest clusters
    HDBSCAN finds, which this layout needs, because its regions run into each other and the
    default merges nearly all of them into two. `copy=True` has it work on a copy of the
    positions, which newer versions of scikit-learn otherwise warn about. It takes about ten
    seconds.
    """)
    return


@app.cell
def _(wide):
    from sklearn.cluster import HDBSCAN

    found = HDBSCAN(
        min_cluster_size=300, min_samples=5, cluster_selection_method="leaf", copy=True
    ).fit_predict(wide[["x", "y"]])
    return (found,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    `found` holds a cluster number for each paper, or -1 for a paper in no cluster. We turn
    each number into a key, such as `"12"`, and leave the papers in no cluster without one.
    Then we name each cluster with its key and the subject class most of its papers have,
    using arXiv's names for the classes from the file the vocabulary read, so that a name both
    says what a cluster is about and tells two clusters about the same class apart.
    """)
    return


@app.cell
def _(arxiv, found, papers, pd, wide):
    mine = pd.DataFrame(
        {
            "entity_id": wide["entity_id"],
            "cluster": [str(number) if number >= 0 else None for number in found],
        }
    )
    mine["primary_category"] = mine["entity_id"].map(
        papers.set_index("entity_id")["primary_category"]
    )
    _classes = pd.read_parquet(arxiv / "primary_category.parquet").set_index("key")["title"]
    _commonest = mine.groupby("cluster")["primary_category"].agg(lambda one: one.mode()[0])
    cluster_names = {key: f"{key}: {_classes[code]}" for key, code in _commonest.items()}
    {"clusters": len(cluster_names), "papers in none": int(mine["cluster"].isna().sum())}
    return cluster_names, mine


@app.cell
def _(mine):
    mine.head()
    return


@app.cell
def _(cluster_names, pd):
    pd.Series(cluster_names, name="name").head()
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    HDBSCAN found 40 clusters and left 23,254 papers, nearly half, in none: the wide layout
    spreads papers out more evenly than the first, so fewer of them sit in a dense region.

    `mine` is a table of our own, one row per paper. `entity_id` names the paper, as in the
    points file, and `cluster` is the key of its cluster, or empty for a paper in none, as most
    of the first rows are. `primary_category` is there only to name the clusters.
    `cluster_names` maps each key to its name.

    The clusters become a flat layer, one level with no hierarchy, so the map shows the same
    clusters at every zoom. Its insert takes `mine`,
    whose `entity_id` names each paper and whose `cluster` column, named with `key=`, says which
    cluster it is in. A paper with no key is in none, and the insert ignores the
    `primary_category` column, which the layer doesn't read. The names go in as a dictionary
    from key to name.
    """)
    return


@app.cell
def _(cluster_names, db, mine):
    db.declare_layer("hdbscan", kind="flat", title="HDBSCAN clusters")
    db.declare_labels("hdbscan/names", of="hdbscan", title="HDBSCAN cluster names")
    db.insert("hdbscan", mine, key="cluster")
    db.insert("hdbscan/names", cluster_names)
    db.commit()
    return


@app.cell
def _(db):
    db.map(
        view="topics-wide",
        colour_by="cluster:hdbscan",
        layers=["hdbscan/names"],
        title_field="title",
    )
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Our clusters cover the denser parts of the wide layout, with their names at their centres,
    and the papers in no cluster are drawn in grey. The report's second line counts the rows of
    `mine`, every one of which named a paper the database already held and added it to a
    cluster at most.

    ### Compare a cluster with the topics

    Take the cluster holding most of the papers on Earth and planetary astrophysics, a subject
    small enough that one cluster should gather it. The next cell finds it by counting the
    subject class in `mine`.
    """)
    return


@app.cell
def _(cluster_names, mine):
    planetary = mine.loc[mine["primary_category"] == "astro-ph.EP", "cluster"].mode()[0]
    cluster_names[planetary]
    return (planetary,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    A filter can keep the papers in one annotation of a layer, with `member_of`, which takes
    the layer and the annotation's key, the one we inserted it with. On a layer with levels it
    takes the level too, as `"level": 1`, and without one it looks at level 0, the only level a
    flat layer has.
    """)
    return


@app.cell
def _(db, planetary):
    in_planetary = {"member_of": {"layer": "hdbscan", "key": planetary}}
    planetary_papers = db.view("topics").filter(in_planetary)
    planetary_papers.count()
    return (planetary_papers,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The cluster holds 466 papers. `aggregate` counts how a selection's items are distributed,
    and grouping by a layer counts them by that layer's annotations. Here we ask which of the
    16 broad topics our cluster's papers are in.
    """)
    return


@app.cell
def _(planetary_papers):
    (_by_topic,) = planetary_papers.aggregate(
        [{"by": {"layer": "clusters/toponymy", "level": 0, "top": 5}, "count": True}]
    )
    _by_topic.to_pandas()[["group", "title", "count"]]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    `aggregate` takes a list of groupings and returns a table for each, so we unpack a list of
    one. `by` says what to group by, `top` how many groups to list, the largest first, and
    `"count": True` asks for the number of papers in each. The `none` row counts the papers in
    no topic at this level.

    Here is a surprise. 349 of the 466 papers are in no broad topic, and 108 of the rest are in
    one called "Higgs Physics". Toponymy names a broad topic from the topics inside it, and the
    next level down shows what happened.
    """)
    return


@app.cell
def _(planetary_papers):
    (_by_topic,) = planetary_papers.aggregate(
        [{"by": {"layer": "clusters/toponymy", "level": 1, "top": 5}, "count": True}]
    )
    _by_topic.to_pandas()[["group", "title", "count"]]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Among the 46 second-level topics, 161 of the papers are in "Stellar Systems and ISM
    Chemistry" and the 108 are in "Quantum Optics and Exoplanets". That topic sits under "Higgs
    Physics" beside a topic on Higgs boson phenomenology, and the language model named the pair
    after the other one, as the topic's card on the map shows. So our cluster is a real subject
    that Toponymy split between a topic it named well and one it filed under a misleading name,
    which is worth knowing before you filter a map by "Higgs Physics". Try the same with
    another of your clusters, or with level 2.

    ## 4. Questions the server answers

    Each answer in this section is one call, which the server works out over the papers it
    holds and returns as a small table. How many papers does each broad topic hold?
    """)
    return


@app.cell
def _(db):
    (by_topic,) = db.aggregate(
        "topics", [{"by": {"layer": "clusters/toponymy", "level": 0, "top": 16}, "count": True}]
    )
    by_topic.to_pandas()[["group", "key", "title", "count"]]
    return (by_topic,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Condensed Matter is the largest broad topic, with 3,923 papers, and 22,696 papers are in
    none of the 16. Each row's `key` is the topic's `mosaica_id`, as a string, rather than the
    `tp0-…` key in the files, and a `member_of` filter takes it as it is.

    When were the papers submitted? A grouping can count a number or date column in bins.
    """)
    return


@app.cell
def _(db):
    (by_year,) = db.aggregate("topics", [{"by": {"field": "submitted_at", "bins": 40}, "count": True}])
    by_year.to_pandas()[["lower", "upper", "count"]]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    `"bins": 40` asks for at most 40 bins, and the server chose edges a reader can use, one bin
    per year, which gives 35. The sample starts with a single paper in 1990 and grows to 4,014
    in 2023. 2024 has 3,212, because the listings end in September.

    Which archives are over-represented among the papers about graphene? Here `filters` names
    the set being counted, the papers whose title has the word, and `reference={}` names the
    set it is compared with, every paper in the view.
    """)
    return


@app.cell
def _(db):
    (_by_archive,) = db.aggregate(
        "topics",
        [{"by": {"field": "archive", "top": 5}, "count": True}],
        filters={"title": {"match": "graphene"}},
        reference={},
    )
    _by_archive.to_pandas()[["group", "title", "count", "reference_count", "lift"]]
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    234 titles mention graphene, and 205 of those papers are in Condensed Matter.
    `reference_count` is each archive's papers in the whole corpus, and `lift` compares the two
    shares: the archive's share of the graphene papers divided by its share of all papers.
    Condensed Matter holds 5,887 of the 50,000 papers, and its lift of 7.44 says graphene
    papers come from it more than seven times as often as papers in general do. Physics, with a
    lift of 1.32, is close to its usual share, while Quantum Physics, at 0.40, and High Energy
    Physics - Theory, at 0.10, are under-represented. The `rest` row gathers every archive
    below the top five, which between them hold no graphene paper.

    Last, a `summary` gives the range and the mean of a column over every paper.
    """)
    return


@app.cell
def _(db):
    (_dates,) = db.aggregate("topics", [{"summary": ["submitted_at"]}])
    _dates.to_pandas()
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    The first paper in the sample was submitted on 16 November 1990 and the last on 19 September
    2024, and the mean submission date is in June 2015. `values_` counts the papers with a date
    and `missing_` those without.

    ## 5. Narrow it down and look at it twice

    Filters combine. Here we keep the papers submitted since 2015, in the broad topic "AI
    Systems", whose title mentions a neural network or neural networks. `match` finds words as
    they are written, without reducing a plural to its singular, so `any_of` asks for either
    form. This time `member_of` names the topic by its `mosaica_id`, its `key` in the table of
    papers per topic above, because the selection is drawn on a map, and the map holds a
    `member_of` filter by `mosaica_id`.
    """)
    return


@app.cell
def _(by_topic, db):
    import datetime

    _ai_systems = by_topic.to_pandas().set_index("title").loc["AI Systems", "key"]
    neural = (
        db.view("topics")
        .filter(
            {
                "any_of": [
                    {"title": {"match": "neural network"}},
                    {"title": {"match": "neural networks"}},
                ]
            }
        )
        .filter({"submitted_at": {"range": {"gte": datetime.date(2015, 1, 1)}}})
        .filter({"member_of": {"layer": "clusters/toponymy", "artifact": _ai_systems}})
    )
    neural.count()
    return (neural,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    138 papers match all three. `match` with two words finds titles holding both, and a date
    filter takes a `datetime.date`, read as midnight UTC. Filters added one after another must
    all match.

    The same filters apply to the other view, because both hold the same papers.
    `mosaica.side_by_side` shows several maps in one row, in Jupyter and in marimo alike.
    """)
    return


@app.cell
def _(db, mosaica, neural):
    mosaica.side_by_side(
        neural.map(
            colour_by="cluster:clusters/toponymy", layers=["topics/toponymy"], title_field="title"
        ),
        db.view("topics-wide")
        .filter(neural.filters)
        .map(colour_by="cluster:hdbscan", layers=["hdbscan/names"], title_field="title"),
    )
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    `neural.filters` is the three filters as one expression, so the second map shows the same
    138 papers in the wide layout. Each map opens framed on its papers, and you can pan and zoom
    them separately. The first draws Toponymy's topics over the first layout, and the second
    our HDBSCAN clusters over the wide layout they were computed from. Open Fields on either map
    to see the three filters as chips, the topic's under its name.

    To work on the papers themselves, read them into pandas.
    """)
    return


@app.cell
def _(neural):
    rows = neural.items(["arxiv_id", "title", "submitted_at", "primary_category"]).to_pandas()
    rows.sort_values("submitted_at").head()
    return (rows,)


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    `items` reads every paper in the selection with the columns you name, plus `mosaica_id`,
    the id Mosaica gives each paper, as a string. From here it's an ordinary data frame. The
    subject classes, for example, show machine learning (`cs.LG`) and computer vision
    (`cs.CV`) at the top.
    """)
    return


@app.cell
def _(rows):
    rows["primary_category"].value_counts().head()
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    ## What you built

    You loaded one set of papers into two views, one per layout, and gave its categories
    arXiv's own names. You added a published clustering and its names as a layer, ran HDBSCAN
    on the wide layout and added your clusters as a second layer, and found, by asking the
    server, that the planetary papers Toponymy split up sit partly under a misleading name. Then
    you counted papers by topic and by year, found the archive graphene papers come from with a
    lift, and drew one combined selection in both layouts side by side.

    To do the same with your own corpus, give each layout you have a view, and each clustering
    or classification a layer, with a label set for its names. The [Python client
    reference](https://github.com/mosaicadb/mosaica/tree/main/docs/reference/python) lists every
    call used here, with the options this notebook left at their defaults.
    """)
    return


if __name__ == "__main__":
    app.run()
