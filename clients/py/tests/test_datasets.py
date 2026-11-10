"""`mosaica.datasets` and `mosaica demo`: what a dataset builds into, what is made again and what
is not, the fetch's lock, the map page's server, and the catalogue page against the
declarations."""

from __future__ import annotations

import contextlib
import http.client
import http.server
import json
import re
import threading
import tomllib
from pathlib import Path

import pytest

import mosaica
from conftest import binary
from mosaica import datasets, demo
from mosaica.datasets import _steps
from mosaica._refusal import Refusal

REPOSITORY = Path(__file__).resolve().parents[3]
PAGE = REPOSITORY / "docs" / "start" / "datasets.md"


def declaration(name: str) -> dict:
    """A dataset's declaration: a checkout's copy for one with nothing to prepare, and the recipe's
    otherwise, without fetching anything."""
    import sys

    dataset = datasets.describe(name)
    if dataset.prepare is None:
        path = datasets.prepared(name)
    else:
        path = sys.modules[dataset.prepare.__module__].DECLARATION
    return tomllib.loads(path.read_text(encoding="utf-8"))


def documented(name: str) -> dict[str, set[str]]:
    """The names in the first column of the Views, Fields and Layers tables of a dataset's section."""
    text = PAGE.read_text(encoding="utf-8")
    start = re.search(rf"^## `{re.escape(name)}`$", text, re.MULTILINE)
    assert start is not None, f"{PAGE} has no section for {name}"
    end = re.search(r"^## ", text[start.end():], re.MULTILINE)
    section = text[start.end(): start.end() + end.start() if end else len(text)]
    tables: dict[str, set[str]] = {}
    for heading in ("Views", "Fields", "Layers"):
        found = re.search(rf"^\*\*{heading}\*\*\n\n((?:\|.*\n)+)", section, re.MULTILINE)
        assert found is not None, f"{name}'s section has no {heading} table"
        rows = found.group(1).splitlines()[2:]
        tables[heading] = {re.match(r"\| `([^`]+)`", row).group(1) for row in rows}
    return tables


@pytest.mark.parametrize("name", datasets.names())
def test_the_page_lists_what_each_declaration_declares(name):
    declared = declaration(name)
    tables = documented(name)
    assert tables["Views"] == {view["name"] for view in declared.get("view", [])}
    assert tables["Fields"] == {a["name"] for a in declared.get("attribute", [])}
    layers = set()
    for layer in declared.get("layer", []):
        layers.add(layer["name"])
        if "labels" in layer:
            layers.add(layer["labels"]["name"])
    assert tables["Layers"] == layers


def test_every_dataset_is_on_the_page():
    text = PAGE.read_text(encoding="utf-8")
    assert set(re.findall(r"^## `([^`]+)`$", text, re.MULTILINE)) == set(datasets.names())


def test_a_sample_example_builds_once_and_serves_every_item(tmp_path):
    binary()
    db = datasets.load("places", dir=tmp_path, port=0, progress=False)
    try:
        assert db.view("world").count() == 150_000
        assert tomllib.loads(db.declaration)["view"][0]["name"] == "world"
        current = (db.path / "bundle" / "CURRENT").stat().st_mtime_ns
    finally:
        db.close()
    again = datasets.load("places", dir=tmp_path, port=0, progress=False)
    try:
        assert (again.path / "bundle" / "CURRENT").stat().st_mtime_ns == current
        assert again.view("world").count() == 150_000
        with page_server({"places": (datasets.describe("places"), None, again)}) as port:
            site = json.loads(ask(port, "GET", "/site.json", {"Host": f"127.0.0.1:{port}"}).read())
            assert site["datasets"][0]["viewer_url"] == again.viewer_url
            seen = {}
            for persona in datasets.describe("places").personas:
                token = json.loads(persona_token(port, "places", persona.id).read())["token"]
                seen[persona.id] = mosaica.connect(again.viewer_url, token).view("world").count()
        assert seen["everything"] == 150_000
        assert all(0 < seen[p] < 150_000 for p in ("eu", "us", "gb", "jp"))
        assert seen["gb"] == again.viewer(["GB"]).view("world").count()
    finally:
        again.close()


def test_an_unknown_dataset_or_an_unwanted_sample_is_refused(tmp_path):
    with pytest.raises(Refusal):
        datasets.load("nowhere", dir=tmp_path)
    with pytest.raises(Refusal):
        datasets.load("places", dir=tmp_path, sample=10)


@pytest.fixture
def publisher(tmp_path):
    """A web server on this machine handing out the files in a directory."""
    served = tmp_path / "published"
    served.mkdir()

    class Quiet(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(served), **kwargs)

        def log_message(self, *_args):
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Quiet)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    yield served, f"http://127.0.0.1:{server.server_address[1]}/"
    server.shutdown()


def test_a_fetch_is_recorded_and_not_repeated(tmp_path, publisher):
    served, base = publisher
    (served / "a.txt").write_text("first")
    raw = tmp_path / "raw"
    _steps.fetch(raw, {"a.txt": base + "a.txt"}, progress=False)
    lock = json.loads((raw / "LOCK.json").read_text())
    assert lock["a.txt"]["bytes"] == 5
    (served / "a.txt").write_text("second")
    _steps.fetch(raw, {"a.txt": base + "a.txt"}, progress=False)
    assert (raw / "a.txt").read_text() == "first"


def test_a_download_that_stops_short_is_refused_and_not_recorded(tmp_path):
    import socket

    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen(1)

    def answer_short():
        connection, _ = listener.accept()
        connection.recv(65536)
        connection.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\n\r\n" + b"x" * 1000)
        connection.close()

    threading.Thread(target=answer_short, daemon=True).start()
    url = f"http://127.0.0.1:{listener.getsockname()[1]}/a.zip"
    raw = tmp_path / "raw"
    with pytest.raises(Refusal):
        _steps.fetch(raw, {"a.zip": url}, progress=False)
    assert not (raw / "a.zip").exists()
    assert "a.zip" not in _steps.locked(raw)


def test_a_directory_of_someone_elses_files_is_left_alone(tmp_path):
    binary()
    theirs = tmp_path / "places" / "db" / "notes.txt"
    theirs.parent.mkdir(parents=True)
    theirs.write_text("mine")
    with pytest.raises(Refusal):
        datasets.load("places", dir=tmp_path, port=0, progress=False)
    assert theirs.read_text() == "mine"


class _Database:
    viewer_url = "http://127.0.0.1:1"

    class _Token:
        token = "secret"
        expires_at = 1e12

    def token(self, terms=None):
        return self._Token()


@contextlib.contextmanager
def page_server(served: dict):
    """The demo's page server over `served`, `{id: (dataset, sample, database)}`, on a free port."""
    site = json.dumps(demo._site("Maps", served)).encode()
    handler = demo._handler(served, b"<p>map</p>", site, b"// bundle")
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        yield server.server_address[1]
    finally:
        server.shutdown()
        server.server_close()


@pytest.fixture
def page():
    with page_server({"places": (datasets.describe("places"), None, _Database())}) as port:
        yield port


def ask(port: int, method: str, path: str, headers: dict) -> http.client.HTTPResponse:
    connection = http.client.HTTPConnection("127.0.0.1", port)
    connection.request(method, path, body=b"{}" if method == "POST" else None, headers=headers)
    return connection.getresponse()


def persona_token(port: int, dataset: str, persona: str) -> http.client.HTTPResponse:
    return ask(port, "POST", f"/token?dataset={dataset}&persona={persona}",
               {"Host": f"127.0.0.1:{port}", "Content-Type": "application/json"})


def test_the_page_gives_a_token_to_its_own_page(page):
    answer = persona_token(page, "places", "gb")
    assert answer.status == 200
    assert json.loads(answer.read()) == {"token": "secret", "expiresAt": 1e12}
    assert ask(page, "GET", "/?dataset=places&as=gb", {"Host": f"localhost:{page}"}).status == 200
    assert persona_token(page, "places", "nobody").status == 404
    assert persona_token(page, "elsewhere", "gb").status == 404


def test_the_page_refuses_another_host_and_a_simple_post(page):
    rebound = {"Host": f"attacker.example:{page}", "Content-Type": "application/json"}
    assert ask(page, "POST", "/token?dataset=places&persona=gb", rebound).status == 403
    assert ask(page, "GET", "/site.json", rebound).status == 403
    simple = {"Host": f"127.0.0.1:{page}", "Content-Type": "text/plain"}
    assert ask(page, "POST", "/token?dataset=places&persona=gb", simple).status == 415


def test_the_list_names_every_dataset(capsys):
    assert demo.main(["--list"]) == 0
    listed = capsys.readouterr().out
    for name in datasets.names():
        assert name in listed


def test_the_command_hands_demo_to_the_package(monkeypatch, capsys):
    native = pytest.importorskip("mosaica_native")
    monkeypatch.setattr("sys.argv", ["mosaica", "demo", "--list"])
    with pytest.raises(SystemExit) as stopped:
        native.main()
    assert stopped.value.code == 0
    assert "places" in capsys.readouterr().out


def gbif_snapshot(raw: Path, parts: list[dict]) -> None:
    """A GBIF snapshot already fetched into `raw`: its index, its parts and the lock that records
    them, so `load` reads nothing from the network."""
    import pyarrow as pa
    import pyarrow.parquet as pq

    from mosaica.datasets import gbif

    (raw / "parts").mkdir(parents=True)
    names = [f"{i:06d}" for i in range(len(parts))]
    (raw / "parts.json").write_text(json.dumps({"snapshot": "2026-11-01", "parts": names}))
    (raw / "citation.txt").write_text("GBIF.org (1 November 2026) GBIF Occurrence Download\n")
    lock = {"citation.txt": {"url": gbif.LISTING + "occurrence/2026-11-01/citation.txt",
                             "bytes": (raw / "citation.txt").stat().st_size}}
    types = {"year": pa.int32(), "decimallatitude": pa.float64(), "decimallongitude": pa.float64()}
    for name, rows in zip(names, parts):
        table = pa.table({c: pa.array([r.get(c) for r in rows], types.get(c, pa.string()))
                          for c in gbif.COLUMNS})
        path = raw / "parts" / f"{name}.parquet"
        pq.write_table(table, path)
        source = f"s3://{gbif.BUCKET}/occurrence/2026-11-01/occurrence.parquet/{name}"
        lock[f"parts/{name}.parquet"] = {"url": source, "bytes": path.stat().st_size}
    (raw / "LOCK.json").write_text(json.dumps(lock))


def occurrence(gbifid: int, **values) -> dict:
    row = {"kingdom": "Animalia", "family": "Anatidae", "genus": "Anas", "species": "Anas crecca",
           "scientificname": "Anas crecca L.", "specieskey": "2498036", "year": 1900,
           "countrycode": "GB", "decimallatitude": 51.5, "decimallongitude": -0.1,
           "gbifid": str(gbifid)}
    return row | values


def test_gbif_places_each_occurrence_with_a_coordinate_under_its_terms(tmp_path):
    binary()
    gbif_snapshot(tmp_path / "gbif" / "raw", [
        [occurrence(1), occurrence(2, decimallatitude=None),
         occurrence(3, countrycode=None, year=2001)],
        [occurrence(4, family=None, countrycode="FR", decimallatitude=89.0, specieskey=None)],
    ])
    db = datasets.load("gbif", dir=tmp_path, port=0, progress=False)
    try:
        assert db.view("geo").count() == 3
        assert db.viewer(["GB"]).view("geo").count() == 1
        assert db.viewer(["UNRECORDED"]).view("geo").count() == 1
        assert db.viewer(["y:1900"]).view("geo").count() == 2
        assert db.viewer(["s:2498036"]).view("geo").count() == 2
        assert [layer["name"] for layer in db.layers()] == ["taxonomy/tree"]
    finally:
        db.close()


def geonames_dump(raw: Path, features: list[tuple]) -> None:
    """A GeoNames dump already fetched into `raw`: `features` as `allCountries.txt` rows, small
    code tables, and the lock that records them, so `load` reads nothing from the network."""
    import zipfile

    from mosaica.datasets import geonames

    raw.mkdir(parents=True)
    with zipfile.ZipFile(raw / "allCountries.zip", "w") as dump:
        dump.writestr("allCountries.txt", "".join(
            "\t".join("" if v is None else str(v) for v in row) + "\n" for row in features))
    tables = {
        "countryInfo.txt": "#ISO\tISO3\n"
        "GB\tGBR\t826\tUK\tUnited Kingdom\tLondon\t244820\t66488991\tEU\t.uk\tGBP\tPound\t44"
        "\t\t\ten-GB\t2635167\tIE\t\n"
        "FR\tFRA\t250\tFR\tFrance\tParis\t547030\t66987244\tEU\t.fr\tEUR\tEuro\t33"
        "\t\t\tfr-FR\t3017382\tBE\t\n",
        "admin1CodesASCII.txt": "GB.ENG\tEngland\tEngland\t6269131\nFR.11\tÎle-de-France"
                                "\tIle-de-France\t3012874\n",
        "admin2Codes.txt": "GB.ENG.GLA\tGreater London\tGreater London\t2648110\n",
        "featureCodes_en.txt": "P.PPLC\tcapital\tcapital of a country\n"
                               "P.PPL\tpopulated place\ta city or village\n"
                               "T.HLL\thill\ta rounded elevation\n"
                               "H.SEA\tsea\ta large body of salt water\n",
    }
    for name, text in tables.items():
        (raw / name).write_text(text, encoding="utf-8")
    lock = {name: {"url": url, "bytes": (raw / name).stat().st_size}
            for name, url in geonames.FILES.items()}
    (raw / "LOCK.json").write_text(json.dumps(lock))


def feature(geonameid, name, lat, lon, kind, country, admin1, admin2, date) -> tuple:
    feature_class, feature_code = kind.split(".")
    return (geonameid, name, name, "", lat, lon, feature_class, feature_code, country, "",
            admin1, admin2, "", "", 0, "", 10, "Europe/London", date)


def test_geonames_puts_each_feature_under_its_country_and_publishes_its_own_id(tmp_path):
    binary()
    geonames_dump(tmp_path / "geonames" / "raw", [
        feature(2643743, "London", 51.5, -0.13, "P.PPLC", "GB", "ENG", "GLA", "2024-01-03"),
        feature(2640000, "Box Hill", 51.25, -0.31, "T.HLL", "GB", "ENG", None, "2024-01-01"),
        feature(2988507, "Paris", 48.85, 2.35, "P.PPL", "FR", "11", None, "2024-01-02"),
        feature(2960848, "North Sea", 56.0, 3.0, "H.SEA", None, None, None, "2024-01-04"),
    ])
    db = datasets.load("geonames", dir=tmp_path, port=0, progress=False)
    try:
        assert db.view("world").count() == 4
        assert db.viewer(["FR"]).view("world").count() < db.viewer(["GB"]).view("world").count()
        attributes = {a["name"] for a in tomllib.loads(db.declaration)["attribute"]}
        assert "geonameid" in attributes and "id" not in attributes
    finally:
        db.close()
    import pyarrow.parquet as pq

    points = pq.read_table(tmp_path / "geonames" / "prepared" / "points.parquet")
    assert sorted(points["geonameid"].to_pylist()) == [2640000, 2643743, 2960848, 2988507]
    first = datasets.load("geonames", dir=tmp_path, sample=2, port=0, progress=False)
    try:
        assert first.view("world").count() == 2
    finally:
        first.close()
    kept = pq.read_table(tmp_path / "geonames" / "prepared-2" / "points.parquet")
    assert sorted(kept["geonameid"].to_pylist()) == [2640000, 2988507]


def test_a_step_runs_again_when_its_inputs_change_or_it_stopped(tmp_path):
    made = []

    def make():
        made.append(1)
        (tmp_path / "out" / "file").write_text("made")

    _steps.made(tmp_path / "out", {"recipe": 1}, make, "making")
    _steps.made(tmp_path / "out", {"recipe": 1}, make, "making")
    assert len(made) == 1
    _steps.made(tmp_path / "out", {"recipe": 2}, make, "making")
    assert len(made) == 2

    def stops():
        raise KeyboardInterrupt

    with pytest.raises(KeyboardInterrupt):
        _steps.made(tmp_path / "out", {"recipe": 3}, stops, "making")
    _steps.made(tmp_path / "out", {"recipe": 3}, make, "making")
    assert len(made) == 3


def test_the_demo_checks_every_dataset_before_it_makes_any(tmp_path, capsys):
    assert demo.main(["places", "nowhere", "--dir", str(tmp_path), "--no-open"]) == 1
    assert demo.main(["places", "places:3", "--dir", str(tmp_path), "--no-open"]) == 1
    assert not (tmp_path / "places").exists()
    with pytest.raises(SystemExit):
        demo.main(["gbif:some"])


def test_a_step_stopped_while_clearing_its_directory_runs_again(tmp_path, monkeypatch):
    out = tmp_path / "out"

    def make():
        (out / "made").mkdir()
        (out / "made" / "file").write_text("made")

    _steps.made(out, {"recipe": 1}, make, "making")

    def stopped(_path):
        raise KeyboardInterrupt

    with monkeypatch.context() as patch:
        patch.setattr(_steps.shutil, "rmtree", stopped)
        with pytest.raises(KeyboardInterrupt):
            _steps.made(out, {"recipe": 2}, make, "making")
    _steps.made(out, {"recipe": 2}, make, "making")
    assert (out / "made" / "file").read_text() == "made"
