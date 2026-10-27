"""The example notebooks, run headless: every cell of `examples/notebook_marimo.py` at the sample
scale, of the first tutorial, `examples/01_getting_started.py`, over the GeoNames example data, and
of the second, `examples/02_analysing_a_map.py`, over the arXiv example data.

A marimo notebook is importable Python: an `App` whose cells are decorated functions. This reads
the file instead, takes each `@app.cell` function's body and executes the bodies in file order in
one namespace. That is the order the walk is written in, and the order marimo runs them in: a cell
that reads a variable runs after the cell that defines it, and cells with no dependency between
them run in file order.

The one cell that assigns `SCALE` has its value replaced with `"sample"` before it runs, so the
walk builds `data/notebook-sample/`, which the notebook finds above its own directory. The test
skips, naming what is missing, where that directory or the `mosaica` binary is absent.

`marimo` is stubbed while the cells run: `mo.md` returns its text, `mo.ui.anywidget` returns a
holder, and `mo.ui.slider` holds its starting value. The widget is built for real, against a stub
JavaScript bundle: there is no browser here, and nothing reads its text.

What is asserted is what each section prints, with the expected numbers read from the sample's
own files; that every map is served each layer it draws or colours by; and that a reader is
served each topic's title exactly when it may see every paper the title was written from.

Each Jupyter notebook beside a marimo file is generated from it by `marimo export ipynb`, and the
last test checks that it is what a fresh export makes.
"""

from __future__ import annotations

import ast
import json
import subprocess
import sys
import tempfile
import types
from pathlib import Path

import pytest

from conftest import binary
from mosaica import Viewer
from mosaica._reports import Declared

pytest.importorskip("pyarrow")
pd = pytest.importorskip("pandas")
pytest.importorskip("anywidget")
pytest.importorskip("sklearn")

NOTEBOOK = Path(__file__).resolve().parents[1] / "examples" / "notebook_marimo.py"
TWIN = NOTEBOOK.with_name("notebook.ipynb")
GETTING_STARTED = NOTEBOOK.with_name("01_getting_started.py")
ANALYSING = NOTEBOOK.with_name("02_analysing_a_map.py")

ASTRO_PH = {"astro-ph", "astro-ph.CO", "astro-ph.EP", "astro-ph.GA", "astro-ph.HE",
            "astro-ph.IM", "astro-ph.SR"}
LEARNING = {"cs.LG", "stat.ML"}


def cell_bodies(path: Path, scaled: bool = True) -> list[ast.Module]:
    """Each `@app.cell` function's body, with its `return` cut and `SCALE` set to the sample.

    `scaled=False` is a notebook with no `SCALE`."""
    bodies, scales = [], 0
    for node in ast.parse(path.read_text(encoding="utf-8"), filename=str(path)).body:
        if not isinstance(node, ast.FunctionDef):
            continue
        if not any(
            isinstance(d, ast.Attribute) and d.attr == "cell" for d in node.decorator_list
        ):
            continue
        body = [line for line in node.body if not isinstance(line, ast.Return)]
        for line in body:
            if isinstance(line, ast.Assign) and [getattr(t, "id", None) for t in line.targets] == [
                "SCALE"
            ]:
                line.value = ast.copy_location(ast.Constant("sample"), line.value)
                scales += 1
        bodies.append(ast.Module(body=body, type_ignores=[]))
    assert scales == int(scaled), f"{path.name} should assign SCALE in {int(scaled)} cells"
    return bodies


class Held:
    """What `mo.ui.anywidget` gives a marimo cell: the widget, and its synced traits as `.value`."""

    def __init__(self, widget):
        self.widget = widget
        self.value: dict = {}


@pytest.fixture
def stub_marimo(monkeypatch):
    marimo = types.ModuleType("marimo")
    marimo.md = lambda text: text
    # The cells run outside a marimo notebook, as they do in Jupyter.
    marimo.running_in_notebook = lambda: False
    marimo.ui = types.SimpleNamespace(
        anywidget=Held,
        slider=lambda *, value, **_: types.SimpleNamespace(value=value),
    )
    monkeypatch.setitem(sys.modules, "marimo", marimo)
    return marimo


@pytest.fixture
def maps(monkeypatch) -> list:
    """Every map the notebook draws, as `(reader, widget)`, in the order the cells ran.

    Every map, whether from a database, a reader or a selection, is made by `Viewer.map`.
    """
    drawn = []
    make = Viewer.map

    def recorded(reader, *args, **kwargs):
        widget = make(reader, *args, **kwargs)
        drawn.append((reader, widget))
        return widget

    monkeypatch.setattr(Viewer, "map", recorded)
    return drawn


@pytest.fixture
def walk(monkeypatch, tmp_path, stub_marimo, stub_bundle, maps):
    """The notebook's cells, executed in order, with every database it opened closed after."""
    binary()
    monkeypatch.delenv("MOSAICA_NOTEBOOK_DATA", raising=False)
    # Section 6 saves the database under a fresh temporary directory; this puts it in tmp_path.
    monkeypatch.setattr(tempfile, "tempdir", str(tmp_path))
    namespace: dict = {"__name__": "notebook_marimo", "__file__": str(NOTEBOOK)}
    try:
        for body in cell_bodies(NOTEBOOK):
            try:
                exec(compile(body, str(NOTEBOOK), "exec"), namespace)  # noqa: S102
            except FileNotFoundError as why:
                if "DATA" in namespace:
                    raise
                pytest.skip(str(why))
        yield namespace
    finally:
        from mosaica._database import Database

        for value in list(namespace.values()):
            if isinstance(value, Database):
                value.close()


def tokens(texts) -> list[list[str]]:
    """Each text's tokens, from `mosaica tokenise`, the analyser the server indexes text with.

    The count of a `match` or `phrase` filter is computed from these in pandas. A regular
    expression places word boundaries differently from the analyser, and its counts on the
    sample differ from the server's.
    """
    lines = "\n".join(" ".join(str(text).split()) if text is not None else "" for text in texts)
    out = subprocess.run(
        [binary(), "tokenise"], input=lines, capture_output=True, text=True, check=True
    ).stdout.split("\n")[: len(texts)]
    return [line.split("\t") if line else [] for line in out]


def expected(data: Path, years: list[str]) -> dict:
    """The counts the notebook should print, read from the sample's files as pandas reads them."""
    papers = pd.read_parquet(
        data / "points.parquet",
        columns=["entity_id", "submitted_at", "categories", "archive", "title", "abstract"],
    )
    in_week = papers["submitted_at"] > papers["submitted_at"].max() - pd.Timedelta(days=7)
    built = papers[~in_week]
    last_year = papers["submitted_at"].dt.year.astype(str) == years[-1]

    def holding(frame, terms):
        return int(frame["categories"].map(lambda held: bool(terms & set(held))).sum())

    cs_since_2020 = (built["archive"] == "cs") & (
        built["submitted_at"] >= pd.Timestamp("2020-01-01")
    )
    transformer = pd.Series(["transformer" in words for words in tokens(built["title"])],
                            index=built.index)
    black_hole = [
        any(pair == ("black", "hole") for pair in zip(words, words[1:]))
        for words in tokens(built["abstract"])
    ]

    return {
        "papers": len(papers),
        "built": len(built),
        "astro": holding(built, ASTRO_PH),
        "learning": holding(built, LEARNING),
        "learning after the week": holding(papers, LEARNING),
        "cs since 2020": int(cs_since_2020.sum()),
        "black hole": sum(black_hole),
        "transformer since 2020": int((cs_since_2020 & transformer).sum()),
        "last year before the week": int((last_year & ~in_week).sum()),
        "last year": int(last_year.sum()),
    }


def names_seen(data: Path, terms: set[str] | None = None) -> dict[str, list[str]]:
    """The topic titles a reader holding `terms` should be shown, by the name's key.

    The notebook inserts each name's title, rank 0 of its contents, with the papers the title was
    written from. A reader is shown the title when it may see every one of those papers.
    `terms=None` is a reader who sees every paper.
    """
    papers = pd.read_parquet(data / "points.parquet", columns=["entity_id", "categories"])
    if terms is not None:
        papers = papers[papers["categories"].map(lambda held: bool(terms & set(held)))]
    seen = set(papers["entity_id"])
    sources = pd.read_parquet(data / "topics-toponymy-members.parquet")
    sources = sources[sources["rank"] == 0]
    shown = sources.groupby("key")["entity"].agg(lambda ids: set(ids) <= seen)
    contents = pd.read_parquet(data / "topics-toponymy.parquet").set_index("key")["contents"]
    return {key: list(contents[key][0]) for key, visible in shown.items() if visible}


def served(reader, view: str, layers: list[str], filters=None):
    """The artifacts of `layers` a reader is served over the whole of `view`, as one tile at the
    largest quota the server takes."""
    per_tile = reader.meta()["selection"]["max_artifacts_per_tile"]
    return reader.viewport_artifacts(view, 0, per_tile, layers=layers, filters=filters)


def served_names(reader) -> dict[str, list[str]]:
    artifacts = served(reader, "papers", ["topic_names"])
    return dict(zip(artifacts.column("key").to_pylist(), artifacts.column("content").to_pylist()))


def drawn_layers(widget) -> set[str]:
    """The layers a map draws, and the layer it colours by when that is a clustering."""
    layers = set(widget.layers or [])
    if widget.colour_by and widget.colour_by.startswith("cluster:"):
        layers.add(widget.colour_by.removeprefix("cluster:"))
    return layers


def test_the_notebook_runs_and_serves_what_each_section_prints(walk, maps):
    """One run of the walk, the counts it prints, and the layers each of its maps is served.

    One test rather than six: the sections share a database and a server, and splitting them
    would build the sample six times.
    """

    want = expected(walk["DATA"], walk["years"])

    # Section 1: the frame, built and counted.
    assert walk["first_count"] == want["papers"]

    # Section 2: the database without the held-back week, as three readers count it.
    assert walk["reader_counts"] == {
        "database": want["built"],
        "astro-ph.*": want["astro"],
        "cs.LG + stat.ML": want["learning"],
    }

    # Section 3: the filters narrow the count, and combining two narrows it further.
    counts = walk["filter_counts"]
    assert counts == {
        "cs since 2020": want["cs since 2020"],
        "'black hole' in the abstract": want["black hole"],
        "cs since 2020, 'transformer' in the title": want["transformer since 2020"],
    }

    # Section 4 and section 5 both commit, and neither was refused.
    assert walk["yearly_report"].ok, walk["yearly_report"]
    assert walk["week_report"].ok, walk["week_report"]

    # Section 5: the week arrives in both views it was inserted into.
    last = f"years:{walk['years'][-1]}"
    assert walk["before_week"] == {"papers": want["built"], last: want["last year before the week"]}
    assert walk["after_week"] == {"papers": want["papers"], last: want["last year"]}
    # Every paper of the last year, the week's included, is in one of that year's clusters.
    clusters = served(walk["db"], last, ["yearly"])
    assert sum(clusters.column("masked_count").to_pylist()) == want["last year"]

    # Suppressing five machine-learning papers hides them from both readers, and lifting the
    # suppression shows them again.
    suppression = walk["suppression"]
    before = {"database": want["papers"], "cs.LG + stat.ML": want["learning after the week"]}
    assert suppression["before"].to_dict() == before
    assert suppression["suppressed"].to_dict() == {key: n - 5 for key, n in before.items()}
    assert suppression["unsuppressed"].to_dict() == before

    # Section 6: the saved copy serves what it was saved with.
    assert walk["saved_at"].joinpath("mosaica.toml").exists()
    assert walk["reopened"].view("papers").count() == want["papers"]

    # A topic's title reaches a reader only where it may see every paper the title was written
    # from. After section 5 the database holds every name in the sample, the week's included,
    # and its own reader is served all of them.
    database_names = served_names(walk["db"])
    assert database_names == names_seen(walk["DATA"])
    for reader, terms in (("astro", ASTRO_PH), ("learning", LEARNING)):
        names = served_names(walk[reader])
        assert names == names_seen(walk["DATA"], terms), reader
        assert 0 < len(names) < len(database_names), reader

    # Every map, as the reader it was drawn for, over its view and filter, is served each layer
    # it draws or colours by.
    assert len(maps) == 7
    # The notebook gives every map that draws a layer its quota.
    assert all(widget.artifacts_per_tile for _, widget in maps if drawn_layers(widget))
    every_drawn_layer_is_served(maps)


def every_drawn_layer_is_served(maps) -> None:
    """Every map, as the reader it was drawn for, over its view and filter, is served each layer
    it draws or colours by.

    A map given no view opens on the first one, and asks for its annotations with its quota, or
    the widget's own, 50 or the server's maximum where that is smaller.
    """
    for reader, widget in maps:
        meta = reader.meta()
        view = widget.view or meta["views"][0]["id"]
        per_tile = widget.artifacts_per_tile or min(50, meta["selection"]["max_artifacts_per_tile"])
        for layer in drawn_layers(widget):
            got = reader.viewport_artifacts(view, 0, per_tile, layers=[layer], filters=widget.filters)
            assert layer in got.column("layer").to_pylist(), (
                f"the map of {view} is served nothing in {layer}"
            )


def run_cells(path: Path, namespace: dict) -> list:
    """Each cell of a tutorial run in order in `namespace`, as the reader runs it, and the value
    each cell shows, which is the value of its last line where that is an expression."""
    shown = []
    for body in cell_bodies(path, scaled=False):
        last = body.body.pop() if body.body and isinstance(body.body[-1], ast.Expr) else None
        exec(compile(body, str(path), "exec"), namespace)  # noqa: S102
        if last is not None:
            shown.append(eval(compile(ast.Expression(last.value), str(path), "eval"), namespace))  # noqa: S307
    return shown


def test_getting_started_shows_the_counts_its_data_holds(stub_marimo, stub_bundle, maps):
    """The first tutorial, each cell run as the reader runs it, against counts read from its
    Parquet file with pyarrow.

    What a cell shows is the value of its last line, so that value is what is checked.
    """
    import pyarrow.compute as pc
    import pyarrow.parquet as pq

    binary()
    namespace: dict = {"__name__": "getting_started", "__file__": str(GETTING_STARTED)}
    try:
        shown = run_cells(GETTING_STARTED, namespace)
        places = pq.read_table(namespace["geonames"] / "points.parquet")
        # Every column of the frame is declared, the coordinates as details.
        (declared,) = [value for value in shown if isinstance(value, Declared)]
        assert [row.name for row in declared.columns] == places.column_names
        counts = [value for value in shown if isinstance(value, int)]
        tables = [value for value in shown if isinstance(value, pd.DataFrame)]

        population = pc.fill_null(places["population"], 0)
        big = pc.sum(pc.greater_equal(population, 1_000_000)).as_py()
        san = sum("san" in words for words in tokens(places["name"].to_pylist()))
        in_region = pc.equal(places["division"], "East Nusa Tenggara, Indonesia")
        assert counts == [big, san, pc.sum(in_region).as_py()]
        # The figures the prose quotes.
        assert counts == [613, 1310, 314]
        assert namespace["db"].view("world").count() == places.num_rows

        # Every class and the places with none, largest first, as pyarrow counts them.
        by_class = tables[1].set_index("key")["count"]
        classes = places["feature_class"].value_counts().to_pylist()
        want = {entry["values"]: entry["counts"] for entry in classes}
        assert {(None if pd.isna(key) else key): n for key, n in by_class.items()} == want
        assert list(by_class) == sorted(by_class, reverse=True)
        assert want[None] == 47

        # The westernmost place filed under the region, which the outlier step finds.
        region = places.filter(in_region)
        west = region.take([pc.index(region["lon"], pc.min(region["lon"])).as_py()])
        found = tables[2].iloc[0]
        assert (found["geonameid"], found["name"], found["lon"]) == (
            west["geonameid"][0].as_py(),
            west["name"][0].as_py(),
            west["lon"][0].as_py(),
        )
        assert found["name"] == "Rokot"

        # The three maps: everything, then each of the two selections.
        assert [widget.filters for _, widget in maps] == [
            None,
            {"population": {"range": {"gte": 1_000_000}}},
            {"division": {"eq": "East Nusa Tenggara, Indonesia"}},
        ]
    finally:
        if "db" in namespace:
            namespace["db"].close()


def test_analysing_a_map_shows_what_its_data_and_its_clusters_hold(stub_marimo, stub_bundle, maps):
    """The second tutorial, each cell run as the reader runs it. What it shows is checked against
    the same counts made in pandas from the sample's files and from the notebook's own HDBSCAN
    clusters, and against the figures its prose quotes.

    The clusters are the reader's scikit-learn run, so what is checked of them is a property:
    the cluster the notebook compares is the one holding most planetary papers, whatever its
    number."""
    binary()
    namespace: dict = {"__name__": "analysing_a_map", "__file__": str(ANALYSING)}
    try:
        shown = run_cells(ANALYSING, namespace)
        arxiv = namespace["arxiv"]
        papers = pd.read_parquet(arxiv / "points.parquet")
        reports = [value for value in shown if hasattr(value, "ok")]
        assert len(reports) == 3 and all(report.ok for report in reports)
        frames = [value for value in shown if isinstance(value, pd.DataFrame)]
        counts = [value for value in shown if isinstance(value, int)]

        # The reader's clusters, each named by its key and its commonest subject class.
        mine = namespace["mine"]
        names = namespace["cluster_names"]
        classes = pd.read_parquet(arxiv / "primary_category.parquet").set_index("key")["title"]
        clustered = mine.dropna(subset=["cluster"])
        assert names == {
            key: f"{key}: {classes[group['primary_category'].mode()[0]]}"
            for key, group in clustered.groupby("cluster")
        }
        summary = next(value for value in shown if isinstance(value, dict) and "clusters" in value)
        assert summary == {"clusters": len(names), "papers in none": len(mine) - len(clustered)}
        # HDBSCAN's result varies with the platform's numerics, so the figures the prose quotes are
        # checked only when this run is the one the prose was written from.
        as_written = summary == {"clusters": 40, "papers in none": 23254}

        # The cluster compared is the one holding most planetary papers, and the server counts
        # its papers by topic as pandas does.
        planetary = namespace["planetary"]
        on_planets = clustered.loc[clustered["primary_category"] == "astro-ph.EP", "cluster"]
        assert planetary == on_planets.value_counts().index[0]
        in_planetary = set(clustered.loc[clustered["cluster"] == planetary, "entity_id"])
        assert counts[0] == len(in_planetary)
        assert not as_written or len(in_planetary) == 466

        members = pd.read_parquet(arxiv / "clusters-toponymy-members.parquet")
        topics = pd.read_parquet(arxiv / "topics-toponymy.parquet")
        title = {row.attached_key: row.contents[0][0] for row in topics.itertuples()}

        def by_topic(entities: set, level: int) -> list:
            held = members[(members["level"] == level) & members["entity"].isin(entities)]
            sizes = held.groupby("key")["entity"].nunique().sort_values(ascending=False)
            return [(title[key], int(n)) for key, n in sizes.items()]

        def listed(frame) -> list:
            rows = frame[frame["group"] == "listed"]
            return list(zip(rows["title"], rows["count"].astype(int)))

        def none_of(frame) -> int:
            return int(frame.loc[frame["group"] == "none", "count"].sum())

        # Two heads of the papers' files, two of the vocabularies', three of the layer's and
        # one of `mine` come before the tables the prose quotes.
        level_0, level_1, per_topic, per_year, graphene, dates = frames[8:14]
        assert listed(level_0) == by_topic(in_planetary, 0)[:5]
        in_a_broad_topic = members.loc[
            (members["level"] == 0) & members["entity"].isin(in_planetary), "entity"
        ].nunique()
        assert none_of(level_0) == len(in_planetary) - in_a_broad_topic
        assert listed(level_1) == by_topic(in_planetary, 1)[:5]
        if as_written:
            assert none_of(level_0) == 349
            assert listed(level_0)[0] == ("Higgs Physics", 108)
            assert listed(level_1)[:2] == [
                ("Stellar Systems and ISM Chemistry", 161), ("Quantum Optics and Exoplanets", 108)
            ]
        # The 108 under "Higgs Physics" are the 108 in "Quantum Optics and Exoplanets", which is
        # one of its two children.
        artifacts = pd.read_parquet(arxiv / "clusters-toponymy.parquet")
        key_of = {name: key for key, name in title.items()}
        higgs, optics = key_of["Higgs Physics"], key_of["Quantum Optics and Exoplanets"]

        def held_in(key: str) -> set:
            return set(members.loc[members["key"] == key, "entity"]) & in_planetary

        assert not as_written or held_in(higgs) == held_in(optics)
        children = {title[key] for key in artifacts.loc[artifacts["parent"] == higgs, "key"]}
        assert len(children) == 2 and "Quantum Optics and Exoplanets" in children

        # Papers per broad topic, largest first, and the papers in none.
        everyone = set(papers["entity_id"])
        assert listed(per_topic) == by_topic(everyone, 0)
        assert listed(per_topic)[0] == ("Condensed Matter", 3923)
        in_any = members.loc[members["level"] == 0, "entity"].nunique()
        assert none_of(per_topic) == len(papers) - in_any == 22696
        assert all(str(key).isdigit() for key in per_topic.loc[per_topic["group"] == "listed", "key"])

        # One bin a year, and the papers of each year.
        years = papers["submitted_at"].dt.year.value_counts().sort_index()
        assert [moment.year for moment in per_year["lower"]] == list(years.index)
        assert list(per_year["count"]) == list(years)
        assert (len(per_year), years.iloc[0], years[2023], years[2024]) == (35, 1, 4014, 3212)

        # The graphene titles by archive against the corpus, each archive's lift as pandas
        # works it out, in the order of their counts.
        about = pd.Series(["graphene" in words for words in tokens(papers["title"].tolist())])
        archives = pd.read_parquet(arxiv / "archive.parquet").set_index("key")["title"]
        on_graphene = papers.loc[about, "archive"].value_counts()
        overall = papers["archive"].value_counts()
        lifts = (on_graphene / about.sum()) / (overall[on_graphene.index] / len(papers))
        rows = graphene[graphene["group"] == "listed"]
        assert list(rows["title"]) == [archives[key] for key in on_graphene.index[:5]]
        assert list(rows["count"]) == list(on_graphene.iloc[:5])
        assert list(rows["reference_count"]) == list(overall[on_graphene.index[:5]])
        assert [round(one, 6) for one in rows["lift"]] == [round(one, 6) for one in lifts.iloc[:5]]
        assert int(about.sum()) == graphene["count"].sum() == 234
        assert [round(one, 2) for one in rows["lift"]] == [7.44, 1.32, 0.40, 1.05, 0.10]
        assert (rows.iloc[0]["count"], rows.iloc[0]["reference_count"]) == (205, 5887)
        assert graphene.loc[graphene["group"] == "rest", "count"].sum() == 0

        # The range and mean of the submission dates.
        submitted = papers["submitted_at"]
        assert dates.loc[0, "min_submitted_at"].date() == submitted.min().date()
        assert dates.loc[0, "max_submitted_at"].date() == submitted.max().date()
        assert dates.loc[0, "mean_submitted_at"].strftime("%Y-%m") == submitted.mean().strftime("%Y-%m")
        assert [dates.loc[0, column].strftime("%Y-%m-%d") for column in ("min_submitted_at", "max_submitted_at")] == [
            "1990-11-16", "2024-09-19"
        ]
        assert dates.loc[0, "mean_submitted_at"].strftime("%Y-%m") == "2015-06"

        # The combined selection, counted, read and drawn in both views.
        ai = members.loc[(members["level"] == 0) & (members["key"] == key_of["AI Systems"]), "entity"]
        words = tokens(papers["title"].tolist())
        neural = pd.Series([("neural" in one) and bool({"network", "networks"} & set(one)) for one in words])
        exact = pd.Series([
            {"neural", "network"} <= set(one) or {"neural", "networks"} <= set(one) for one in words
        ])
        assert (neural == exact).all()
        since = papers["submitted_at"] >= pd.Timestamp("2015-01-01")
        chosen = papers[exact & since & papers["entity_id"].isin(set(ai))]
        assert counts[1] == len(chosen) == 138
        rows = namespace["rows"]
        assert sorted(rows["arxiv_id"]) == sorted(chosen["arxiv_id"])
        head = frames[-1]
        assert list(head["arxiv_id"]) == list(chosen.sort_values("submitted_at")["arxiv_id"].head())
        assert all(isinstance(one, str) for one in rows["mosaica_id"])
        assert list(shown[-1].head(2).index) == list(chosen["primary_category"].value_counts().index[:2])
        assert list(shown[-1].head(2).items()) == [("cs.LG", 44), ("cs.CV", 23)]

        assert [(widget.view, widget.colour_by) for _, widget in maps] == [
            ("topics", "archive"),
            ("topics-wide", "archive"),
            ("topics-wide", "cluster:clusters/toponymy"),
            ("topics-wide", "cluster:hdbscan"),
            ("topics", "cluster:clusters/toponymy"),
            ("topics-wide", "cluster:hdbscan"),
        ]
        assert all(widget.title_field == "title" for _, widget in maps)
        # Both maps are given the selection's filters, its date as microseconds.
        assert maps[-1][1].filters == maps[-2][1].filters
        assert [list(leaf) for leaf in maps[-1][1].filters["all_of"]] == [
            list(leaf) for leaf in namespace["neural"].filters["all_of"]
        ]
        every_drawn_layer_is_served(maps)
    finally:
        if "db" in namespace:
            namespace["db"].close()


@pytest.mark.parametrize(
    "marimo_file", [NOTEBOOK, GETTING_STARTED, ANALYSING], ids=lambda p: p.stem
)
def test_the_jupyter_notebook_is_the_export_of_the_marimo_file(tmp_path, marimo_file):
    """Each `.ipynb` is what `marimo export ipynb` makes of its marimo file today."""
    pytest.importorskip("marimo")
    fresh = tmp_path / "notebook.ipynb"
    subprocess.run(
        [sys.executable, "-m", "marimo", "export", "ipynb", str(marimo_file), "-o", str(fresh)],
        check=True,
        capture_output=True,
    )

    def read(path: Path) -> dict:
        notebook = json.loads(path.read_text(encoding="utf-8"))
        notebook["metadata"].get("marimo", {}).pop("marimo_version", None)
        return notebook

    twin = TWIN if marimo_file == NOTEBOOK else marimo_file.with_suffix(".ipynb")
    assert read(twin) == read(fresh)
