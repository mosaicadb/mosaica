"""A spatial layer, built and then published through the control plane (§4.6, contracts §3.4).

A spatial artifact's shape is its whole membership, resolved per request against each generation's
own segments, so nothing here stages a member set: what is read back is the masked count the shape
produced over the viewer's own visible rows.
"""

from __future__ import annotations

import pyarrow as pa
import pytest

from conftest import browse

from test_sdk_corpus import declare_notebook

pytest.importorskip("pyarrow")

#: The lower-left quarter of the notebook corpus's own coordinates, in the view's space.
REGION = "POLYGON ((0 0, 30000 0, 30000 30000, 0 30000, 0 0))"


def test_a_layer_of_inline_polygons_is_built_and_served_with_a_masked_count(served, corpus):
    def declare(db):
        declare_notebook(db, corpus)
        db.declare_layer(
            "regions",
            kind="flat",
            membership="spatial",
            shape={"kind": "polygon"},
            artifacts=[{"key": "lower_left", "wkt": REGION, "space": "view"}],
            title="Regions",
        )

    db = served(declare)
    assert "[[layer.artifacts]]" in db.declaration
    rows = browse(db, "s0", "regions")["artifacts"]
    assert [row["key"] for row in rows] == ["lower_left"]
    # The corpus holds 50,000 papers and the polygon is a quarter of the frame, so the shape
    # selected some of them and not all of them.
    assert 0 < rows[0]["masked_count"] < 50_000


def test_a_spatial_layer_published_after_the_first_commit_carries_its_wkt_and_its_space(
    served, corpus
):
    db = served(lambda db: declare_notebook(db, corpus))
    db.declare_layer(
        "regions",
        kind="flat",
        membership="spatial",
        shape={"kind": "polygon"},
        title="Regions",
    )
    # The shape is named by its kind, and its column is read under the name the build reads it
    # under (§3). A polygon published at a running service is WKT text, which is what the
    # publication route takes; the build reads the same column as WKB.
    db.insert(
        "regions",
        artifacts=pa.table(
            {
                "key": pa.array(["lower_left"], pa.string()),
                "geometry": pa.array([REGION], pa.string()),
                "space": pa.array(["view"], pa.string()),
            }
        ),
        key="key",
        shape="polygon",
        space="space",
    )
    plan = db.check()
    assert plan.ok, plan
    assert any("declare layer 'regions'" in line for line in plan.plan), plan.plan
    # A spatial layer's members are never paged: the plan carries the publication and no growth.
    assert not any(line.startswith("grow") for line in plan.plan), plan.plan

    report = db.commit()
    assert report.ok, report
    assert report.artifacts_minted == 1

    rows = browse(db, "s0", "regions")["artifacts"]
    assert [row["key"] for row in rows] == ["lower_left"]
    assert 0 < rows[0]["masked_count"] < 50_000


def test_a_polygon_in_wkb_at_the_publication_route_is_a_finding(served, corpus):
    """The two doors read one column two ways: the build takes WKB, the route takes WKT."""
    db = served(lambda db: declare_notebook(db, corpus))
    db.declare_layer(
        "regions/wkb",
        kind="flat",
        membership="spatial",
        shape={"kind": "polygon"},
        title="Regions",
    )
    db.insert(
        "regions/wkb",
        artifacts=pa.table(
            {
                "key": pa.array(["lower_left"], pa.string()),
                "geometry": pa.array([b"\x01\x03"], pa.binary()),
            }
        ),
        key="key",
        shape="polygon",
    )
    plan = db.check()
    assert not plan.ok
    assert any("WKB at the publication route" in str(f) for f in plan.findings), plan


def test_an_inline_roster_built_at_the_first_commit_is_not_offered_again(served, corpus):
    """An `excluding` row is the case that shows it (ingest §2.3).

    The build compiles the inline roster into the bundle. Offered again at the next commit it
    would be a `409`: the complement is taken over the entities that exist then, so the same list
    is a different set. The first commit records the keys as published, and the plan that follows
    carries no publication for the layer.
    """

    def declare(db):
        declare_notebook(db, corpus)
        db.declare_layer(
            "cohorts",
            kind="flat",
            artifacts=[{"key": "all_but_three", "excluding": {"id": [1, 2, 3]}}],
            require_member_visibility={"count": 1},
            title="Cohorts",
        )

    db = served(declare)
    rows = browse(db, "s0", "cohorts")["artifacts"]
    assert [row["key"] for row in rows] == ["all_but_three"]
    assert rows[0]["masked_count"] == 49_997

    plan = db.check()
    assert plan.ok, plan
    assert not any("cohorts" in line for line in plan.plan), plan.plan
    report = db.commit()
    assert report.ok, report

    rows = browse(db, "s0", "cohorts")["artifacts"]
    assert rows[0]["masked_count"] == 49_997


# ------------------------------------------------------------- a box on a projected view

#: A box in degrees whose edges fall between the grid's points.
EUROPE = (-25.0, 5.0, 45.0, 65.0)


def graticule():
    """A point every ten degrees, as the rows a projected view is inserted with."""
    import pandas as pd

    places = [(lon, lat) for lon in range(-170, 171, 10) for lat in range(-70, 71, 10)]
    return pd.DataFrame({
        "id": range(len(places)),
        "lon": [float(lon) for lon, _ in places],
        "lat": [float(lat) for _, lat in places],
    })


def held(rows, box):
    """The ids of the rows inside `box`, in degrees, from the rows themselves."""
    inside = rows[rows.lon.between(box[0], box[2]) & rows.lat.between(box[1], box[3])]
    return sorted(inside.id)


def mercator(box):
    """`box` in a whole-world Web Mercator view's own coordinates, where y runs south."""
    import math

    def y(lat):
        return 0.5 - math.asinh(math.tan(math.radians(lat))) / (2 * math.pi)

    return [(box[0] + 180) / 360, y(box[3]), (box[2] + 180) / 360, y(box[1])]


@pytest.fixture
def world(tmp_path):
    """A database with one Web Mercator view, `world`, and the graticule in it; and the rows."""
    from conftest import binary
    from mosaica._database import create

    binary()
    db = create(tmp_path / "world")
    rows = graticule()
    db.declare_view("world", projection="web_mercator",
                    extent={"lon": [-180.0, 180.0], "lat": [-80.0, 80.0]})
    db.declare_attribute("id", type="i64", unique=True)
    db.insert("world", rows, lon="lon", lat="lat")
    assert db.commit().ok
    yield db, rows
    db.close()


def test_a_box_on_a_projected_view_is_in_the_degrees_its_rows_were_inserted_with(world):
    db, rows = world
    expected = held(rows, EUROPE)
    assert 0 < len(expected) < len(rows)
    selection = db.view("world").within(EUROPE)
    assert selection.count() == len(expected)
    assert sorted(selection.items(["id"]).to_pandas().id) == expected
    # The same box, stated in each space through the filter.
    stated = {"region": {"bbox": list(EUROPE), "space": "wgs84"}}
    assert db.view("world").filter(stated).count() == len(expected)
    in_view = {"region": {"bbox": mercator(EUROPE), "space": "view"}}
    assert db.view("world").filter(in_view).count() == len(expected)


def test_two_boxes_on_a_projected_view_keep_their_overlap(world):
    db, rows = world
    north = (-180.0, 35.0, 180.0, 75.0)
    overlap = (EUROPE[0], 35.0, EUROPE[2], EUROPE[3])
    selection = db.view("world").within(EUROPE).within(north)
    assert selection.box == overlap
    assert selection.count() == len(held(rows, overlap)) > 0


def test_a_sample_of_a_box_on_a_projected_view_is_drawn_from_inside_it(world):
    import json

    db, rows = world
    expected = held(rows, EUROPE)
    sample = db.view("world").within(EUROPE).sample(zoom=3, k=1024)
    counts = json.loads(sample.schema.metadata[b"mosaica.counts"])
    assert counts["matched"] == sample.num_rows == len(expected)
    # The whole view, by comparison, is every row.
    assert db.view("world").sample(zoom=0, k=1024).num_rows == len(rows)


def test_a_map_of_a_box_on_a_projected_view_is_framed_in_the_views_coordinates(world, stub_bundle):
    db, _ = world
    m = db.view("world").within(EUROPE).map()
    assert m.view == "world" and m.filters is None
    assert m.bbox == pytest.approx(mercator(EUROPE))


def test_a_box_on_a_projected_view_selects_the_same_after_a_restart(world, tmp_path):
    from mosaica._database import open as open_database

    db, rows = world
    db.close()
    again = open_database(tmp_path / "world")
    try:
        assert again.view("world").within(EUROPE).count() == len(held(rows, EUROPE))
    finally:
        again.close()
