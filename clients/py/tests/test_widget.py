"""The kernel half of the widget protocol, against a fake comm.

ipywidgets opens a dummy comm when no kernel is running, so a widget can be built here; what is
faked is the send side (captured) and the page's messages (delivered through the same dispatch
the comm would use), which is the whole surface the protocol has.
"""

import json
import pathlib
import warnings

import pytest
import traitlets

anywidget = pytest.importorskip("anywidget")

from mosaica import Map, Token  # noqa: E402


@pytest.fixture
def make(monkeypatch, tmp_path):
    """A Map whose bundle is a stub and whose `send` is captured."""
    import mosaica.widget as widget

    stub = tmp_path / "mosaica-components.js"
    stub.write_text("export function render() {}")
    monkeypatch.setattr(widget, "bundle_path", lambda: stub)

    def build(**kw):
        m = Map(kw.pop("url", "http://viewer.test"), **kw)
        m.sent, m.buffers = [], []
        monkeypatch.setattr(m, "send", lambda content, buffers=None: (m.sent.append(content), m.buffers.append(buffers)))
        return m

    return build


def page_says(m, content):
    m._handle_custom_msg(content, [])


def test_the_token_is_never_model_state(make):
    m = make(token="tok-secret")
    state = m.get_state()
    assert "tok-secret" not in json.dumps({k: v for k, v in state.items() if k != "_esm"})
    assert not any(name.startswith("token") for name in m.trait_names())


def test_the_page_gets_the_bundle_as_synced_esm_and_empty_css(make):
    m = make(token="t")
    state = m.get_state()
    assert state["_esm"].startswith("export function render")
    assert state["_css"] == ""


def test_the_up_traits_start_as_none_not_empty(make):
    m = make(token="t")
    assert m.region is None and m.filters is None and m.selected is None and m.bbox is None


def test_ready_is_answered_with_the_token_as_a_custom_message(make):
    m = make(token="tok-1")
    assert m.sent == []
    page_says(m, {"type": "ready"})
    assert m.sent == [{"type": "token", "token": "tok-1", "expires_at": None}]
    assert m.tokens_sent == 1


def test_reauthorise_renews_a_token_that_can_renew(make):
    calls = []

    def mint(n=[0]):
        n[0] += 1
        calls.append(n[0])
        return Token(f"tok-{n[0]}", 1800000000.0 + n[0], renew=mint)

    m = make(token=mint())
    page_says(m, {"type": "ready"})
    page_says(m, {"type": "reauthorise"})
    assert [s["token"] for s in m.sent] == ["tok-1", "tok-2"]
    assert m.sent[1]["expires_at"] == 1800000002.0


def test_reauthorise_calls_a_callable_source_and_resends_a_fixed_string(make):
    tokens = iter(["a", "b"])
    m = make(token=lambda: next(tokens))
    page_says(m, {"type": "ready"})
    page_says(m, {"type": "reauthorise"})
    assert [s["token"] for s in m.sent] == ["a", "b"]
    fixed = make(token="fixed")
    page_says(fixed, {"type": "ready"})
    page_says(fixed, {"type": "reauthorise"})
    assert [s["token"] for s in fixed.sent] == ["fixed", "fixed"]


def test_a_failing_source_refuses_rather_than_raising_into_the_comm(make):
    def broken():
        raise RuntimeError("idp down")

    m = make(token=broken)
    page_says(m, {"type": "ready"})
    assert m.sent == [{"type": "refused", "detail": "idp down"}]


def test_map_needs_a_token(make):
    with pytest.raises(TypeError):
        make()


def test_ids_are_decimal_strings(make):
    m = make(token="t")
    m.selected = 2**63 + 5
    assert m.selected == "9223372036854775813"
    m.selected_artifact = "7"
    assert m.selected_artifact == "7"
    for bad in ("0x10", -1, 2**64, 1.5, "seven", True):
        with pytest.raises(traitlets.TraitError):
            m.selected = bad
    m.selected = None
    assert m.selected is None


def test_layers_refuse_the_all_keyword_and_tell_none_from_default(make):
    m = make(token="t")
    assert m.layers is None
    with pytest.raises(traitlets.TraitError):
        m.layers = ["all"]
    m.layers = ["clusters/a"]
    assert m.layers == ["clusters/a"]
    m.layers = []
    assert m.layers == []


def test_bbox_is_four_floats_or_none(make):
    m = make(token="t", bbox=(0, 1, 2, 3))
    assert m.bbox == [0.0, 1.0, 2.0, 3.0]
    with pytest.raises(traitlets.TraitError):
        m.bbox = [1, 2]


def test_a_page_error_lands_in_last_error_and_a_warning(make):
    m = make(token="t")
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        page_says(m, {"type": "error", "what": "filters", "detail": "none_of"})
    assert m.last_error == "filters: none_of"
    assert any("filters: none_of" in str(w.message) for w in caught)
    assert "last_error" not in m.get_state()


def test_the_synced_surface_is_exactly_the_design_s(make):
    synced = set(Map.class_traits(sync=True)) - set(anywidget.AnyWidget.class_traits(sync=True))
    synced = {k for k in synced if not k.startswith("_")}  # `_esm` and `_css` are anywidget's
    assert synced == {
        "url", "view", "explorer_layout", "height", "title_field", "artifacts_per_tile",
        "budget", "budget_min", "budget_max", "cluster_budget", "cluster_budget_min", "cluster_budget_max", "bbox", "layers", "colour_by", "palette", "value_colours", "cluster_colours", "size_by", "size_min", "size_max", "size_scale", "filters",
        "selected", "selected_artifact", "region",
    }


def test_size_settings_are_synced_and_leave_the_map_s_choice_by_default(make):
    state = make(token="t").get_state()
    assert [state[k] for k in ("size_by", "size_min", "size_max", "size_scale")] == [None, None, None, None]
    m = make(token="t", size_by="citations", size_min=1.5, size_max=12, size_scale="log")
    state = m.get_state()
    assert (state["size_by"], state["size_min"], state["size_max"], state["size_scale"]) == ("citations", 1.5, 12.0, "log")
    m.size_scale = "rank"
    assert m.get_state()["size_scale"] == "rank"
    with pytest.raises(traitlets.TraitError):
        m.size_scale = "square"


def test_annotations_per_tile_leave_the_page_s_default_unless_given_a_number(make):
    # `None` is the page's own default: 50, or the server's maximum where that is smaller.
    assert make(token="t").get_state()["artifacts_per_tile"] is None
    m = make(token="t", artifacts_per_tile=7)
    assert m.get_state()["artifacts_per_tile"] == 7


def test_the_palette_is_synced_and_leaves_the_map_s_choice_by_default(make):
    assert make(token="t").get_state()["palette"] is None
    m = make(token="t", palette="kelly")
    assert m.get_state()["palette"] == "kelly"
    m.palette = "okabe-ito"
    assert m.get_state()["palette"] == "okabe-ito"
    with pytest.raises(traitlets.TraitError):
        m.palette = "positional"


def test_value_and_cluster_colours_go_down_and_leave_the_map_s_choices_by_default(make):
    state = make(token="t").get_state()
    assert state["value_colours"] is None and state["cluster_colours"] is None
    m = make(token="t", value_colours={"venue": {"nips": "#A0B0C0"}}, cluster_colours={"topics": {7: "#112233"}})
    state = m.get_state()
    assert state["value_colours"] == {"venue": {"nips": "#a0b0c0"}}
    assert state["cluster_colours"] == {"topics": {"7": "#112233"}}
    m.cluster_colours = {}
    assert m.get_state()["cluster_colours"] == {}


def test_colours_chosen_in_the_map_come_up_and_are_read_in_a_later_cell(make):
    m = make(token="t")
    seen = []
    m.observe(lambda change: seen.append(change["name"]), names=["value_colours", "cluster_colours"])
    m.set_state({"value_colours": {"venue": {"icml": "#abcdef"}}, "cluster_colours": {"topics": {"18446744073709551615": "#000000"}}})
    assert m.value_colours == {"venue": {"icml": "#abcdef"}}
    assert m.cluster_colours == {"topics": {"18446744073709551615": "#000000"}}
    assert sorted(seen) == ["cluster_colours", "value_colours"]


def test_colours_must_be_hex_and_clusters_named_by_mosaica_id(make):
    m = make(token="t")
    for bad in ({"venue": {"nips": "red"}}, {"venue": "#112233"}, {"venue": {"nips": "#12345"}}):
        with pytest.raises(traitlets.TraitError):
            m.value_colours = bad
    for bad in ({"7": "#112233"}, {"t": {"seven": "#112233"}}, {"t": {"7": "#11223g"}}, {"t": {-1: "#112233"}}, {"t": {2**64: "#112233"}}):
        with pytest.raises(traitlets.TraitError):
            m.cluster_colours = bad


def test_the_point_budget_and_its_range_are_synced_down_with_their_defaults(make):
    state = make(token="t").get_state()
    assert [state[k] for k in ("budget", "budget_min", "budget_max")] == [0, 1_000, 2_000_000]
    m = make(token="t", budget=40_000, budget_min=500, budget_max=90_000)
    assert [m.get_state()[k] for k in ("budget", "budget_min", "budget_max")] == [40_000, 500, 90_000]
    m.budget = 60_000
    assert m.get_state()["budget"] == 60_000


def test_title_field_is_synced_down_and_defaults_to_none(make):
    assert make(token="t").get_state()["title_field"] is None
    assert make(token="t", title_field="title").get_state()["title_field"] == "title"


def test_map_without_a_bundle_says_how_to_get_one(monkeypatch):
    import mosaica.widget as widget

    monkeypatch.setattr(widget, "bundle_path", lambda: None)
    with pytest.raises(RuntimeError):
        Map("http://viewer.test", token="t")


def test_the_cluster_budget_and_its_range_are_synced_down_with_their_defaults(make):
    keys = ("cluster_budget", "cluster_budget_min", "cluster_budget_max")
    assert [make(token="t").get_state()[k] for k in keys] == [1_000, 10, 10_000]
    m = make(token="t", cluster_budget=300, cluster_budget_min=5, cluster_budget_max=5_000)
    assert [m.get_state()[k] for k in keys] == [300, 5, 5_000]
    m.cluster_budget = 40
    assert m.get_state()["cluster_budget"] == 40


def synced_basemap(m):
    """What the page reads: the value and the local images it names, in one trait."""
    return m.get_state()["_basemap"]


def test_the_basemap_goes_down_as_given_and_defaults_to_the_map_s_own(make):
    assert synced_basemap(make(token="t")) == {"value": None, "images": {}}
    for value in (
        "world",
        "none",
        {"tiles": "https://tiles.example.org/{z}/{x}/{y}.png", "attribution": "Example"},
        {"image": "https://example.org/plan.png", "bounds": [0.0, 0.0, 10.0, 5.0]},
        {"image": "data:image/png;base64,iVBORw0KGgo="},
        {"geo": "world", "xy": "none"},
        {},
    ):
        m = make(token="t", basemap=value)
        assert m.basemap == value and synced_basemap(m) == {"value": value, "images": {}}
    m = make(token="t")
    np = pytest.importorskip("numpy")
    m.basemap = {"xy": {"image": "http://example.org/a.png", "bounds": np.array([0, 0, 1, 2.5])}, "geo": {"tiles": "https://t.example.org/{z}/{x}/{y}.png", "attribution": None}}
    want = {"xy": {"image": "http://example.org/a.png", "bounds": [0.0, 0.0, 1.0, 2.5]}, "geo": {"tiles": "https://t.example.org/{z}/{x}/{y}.png"}}
    assert m.basemap == want and synced_basemap(m)["value"] == want
    m.basemap = {"image": "http://example.org/a.png", "bounds": (np.float32(1), np.int64(2), 3, 4.5)}
    assert m.basemap["bounds"] == [1.0, 2.0, 3.0, 4.5]
    m.basemap = None
    assert synced_basemap(m) == {"value": None, "images": {}}
    assert "basemap" not in m.get_state()


def test_a_basemap_of_the_wrong_shape_is_refused_at_the_call(make):
    m = make(token="t", basemap="world")
    for bad in (
        "osm",
        3,
        ["world"],
        {"tiles": "https://tiles.example.org/tile.png"},
        {"tiles": "https://tiles.example.org/{z}/{x}/{y}.png", "attribution": 7},
        {"tiles": "https://tiles.example.org/{z}/{x}/{y}.png", "credit": "Example"},
        {"image": ""},
        {"image": 4},
        {"image": "file:///home/me/plan.png"},
        {"image": "http://example.org/a.png", "bounds": [0, 0, 1]},
        {"image": "http://example.org/a.png", "bounds": [0, 0, 1, float("nan")]},
        {"image": "http://example.org/a.png", "bounds": [0, 0, 1, 10**400]},
        {"image": "http://example.org/a.png", "bounds": [0, 0, 1, True]},
        {"image": "http://example.org/a.png", "bounds": "0,0,1,1"},
        {"image": "http://example.org/a.png", "bounds": 4},
        {"image": "http://example.org/a.png", "bounds": {0, 1, 2, 3}},
        {"image": "http://example.org/a.png", "tiles": "https://t.example.org/{z}/{x}/{y}.png"},
        {"image": {"image": "https://example.org/a.png"}},
        {"tiles": {"tiles": "https://t.example.org/{z}/{x}/{y}.png"}},
        {"image": "world", "tiles": {"image": "https://example.org/a.png"}},
        {"geo": "osm"},
        {"geo": {"imgae": "a.png"}},
        {1: "world"},
    ):
        with pytest.raises(traitlets.TraitError):
            m.basemap = bad
    assert m.basemap == "world"


def test_a_key_holding_none_is_left_out_at_either_level(make):
    m = make(token="t")
    m.basemap = {"image": "https://example.org/a.png", "tiles": None}
    assert m.basemap == {"image": "https://example.org/a.png"}
    m.basemap = {"tiles": None, "geo": "world"}
    assert m.basemap == {"geo": "world"}
    m.basemap = {"geo": None, "xy": {"image": "https://example.org/a.png", "bounds": None}}
    assert m.basemap == {"xy": {"image": "https://example.org/a.png"}}
    assert synced_basemap(m)["value"] == {"xy": {"image": "https://example.org/a.png"}}


def test_a_dict_with_tiles_or_image_is_one_value_and_world_is_a_path_there(make, tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    m = make(token="t")
    with pytest.raises(traitlets.TraitError):
        m.basemap = {"image": "world"}
    (tmp_path / "world").write_bytes(b"png")
    m.basemap = {"image": "world"}
    assert synced_basemap(m) == {"value": {"image": {"local": "world"}}, "images": {"world": 1}}


def asks(m, reads):
    page_says(m, {"type": "basemap-images", "reads": reads})


def test_a_local_image_goes_to_the_page_when_it_asks_and_never_into_saved_state(make, tmp_path, monkeypatch):
    png = b"\x89PNG\r\n\x1a\n" + b"pixels" * 50
    (tmp_path / "plan.png").write_bytes(png)
    monkeypatch.chdir(tmp_path)
    plan = "plan.png"
    given = {"geo": "world", "xy": {"image": pathlib.Path("plan.png"), "bounds": [0, 0, 4, 3]}}
    m = make(token="t", basemap=given)
    assert m.basemap == {"geo": "world", "xy": {"image": pathlib.Path("plan.png"), "bounds": [0.0, 0.0, 4.0, 3.0]}}
    assert synced_basemap(m) == {"value": {"geo": "world", "xy": {"image": {"local": plan}, "bounds": [0.0, 0.0, 4.0, 3.0]}}, "images": {plan: 1}}
    saved = json.dumps({k: v for k, v in m.get_state().items() if k != "_esm"})
    assert "pixels" not in saved and str(tmp_path) not in saved
    # Nothing is pushed: the page asks for the reads it lacks, and gets only those it names.
    assert m.sent == []
    asks(m, {plan: 1})
    asks(m, {plan: 0, "elsewhere.png": 1})
    assert m.sent == [{"type": "basemap-image", "path": plan, "read": 1, "mime": "image/png"}]
    assert m.buffers == [[png]]
    # A change elsewhere, or the same bytes read again, keeps the read; edited bytes move it on,
    # even under the same value.
    m.basemap = {"geo": "none", "xy": {"image": "plan.png"}}
    m.basemap = {"xy": {"image": "plan.png"}}
    assert synced_basemap(m)["images"] == {plan: 1}
    (tmp_path / "plan.png").write_bytes(png + b"more")
    m.basemap = {"xy": {"image": "plan.png"}}
    assert synced_basemap(m) == {"value": {"xy": {"image": {"local": plan}}}, "images": {plan: 2}}
    m.sent.clear()
    m.buffers.clear()
    asks(m, {plan: 1})
    asks(m, {plan: 2})
    assert m.sent == [{"type": "basemap-image", "path": plan, "read": 2, "mime": "image/png"}]
    assert m.buffers == [[png + b"more"]]
    m.basemap = "world"
    assert synced_basemap(m) == {"value": "world", "images": {}}
    m.sent.clear()
    asks(m, {plan: 2})
    assert m.sent == []


def test_a_path_stays_a_path_even_where_it_reads_as_a_url(make, tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    (tmp_path / "https:").mkdir()
    (tmp_path / "https:" / "plan.png").write_bytes(b"png")
    m = make(token="t", basemap={"image": pathlib.Path("https:/plan.png")})
    plan = "https:/plan.png"
    assert synced_basemap(m) == {"value": {"image": {"local": plan}}, "images": {plan: 1}}
    # Read back and set again, it is still the file.
    m.basemap = m.basemap
    assert m.basemap == {"image": pathlib.Path("https:/plan.png")} and synced_basemap(m)["images"] == {plan: 1}
    with pytest.raises(traitlets.TraitError):
        m.basemap = {"image": pathlib.Path("~/plan.png")}


def test_a_local_image_that_cannot_be_read_is_refused_and_the_value_kept(make, tmp_path):
    with pytest.raises(traitlets.TraitError):
        make(token="t", basemap={"image": tmp_path / "missing.png"})
    m = make(token="t", basemap="world")
    for unreadable in ({"geo": {"image": str(tmp_path)}}, {"image": tmp_path / "missing.png"}):
        with pytest.raises(traitlets.TraitError):
            m.basemap = unreadable
    assert m.basemap == "world" and synced_basemap(m) == {"value": "world", "images": {}}
    # Inside a hold the refusal rolls back too.
    with pytest.raises(traitlets.TraitError):
        with m.hold_trait_notifications():
            m.basemap = {"image": tmp_path / "missing.png"}
    assert m.basemap == "world" and synced_basemap(m) == {"value": "world", "images": {}}


def test_a_held_change_that_fails_keeps_the_images_the_page_may_still_ask_for(make, tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    (tmp_path / "plan.png").write_bytes(b"old")
    (tmp_path / "other.png").write_bytes(b"new")
    m = make(token="t", basemap={"image": "plan.png"})
    with pytest.raises(traitlets.TraitError):
        with m.hold_trait_notifications():
            m.basemap = {"image": "other.png"}
            m.layers = ["all"]
    assert m.basemap == {"image": "plan.png"} and synced_basemap(m)["images"] == {"plan.png": 1}
    asks(m, {"plan.png": 1})
    assert m.sent == [{"type": "basemap-image", "path": "plan.png", "read": 1, "mime": "image/png"}]
    assert m.buffers == [[b"old"]]


def test_an_image_url_is_not_read_here(make):
    m = make(token="t", basemap={"image": "HTTPS://example.org/plan.png"})
    asks(m, {"HTTPS://example.org/plan.png": 1})
    assert m.sent == [] and synced_basemap(m)["images"] == {}
