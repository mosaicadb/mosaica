"""The control-plane client on its own: the retry rule and the batch id.

The `429` is the one status the SDK retries, and a page of rows no server answered the one request
it sends once more, and each must resend identical bytes: a batch id maps to
the SHA-256 of the raw request body, so a body re-serialised before the retry would be a new batch
rather than a replay. That is what this file fakes a server for. Everything else about the plane is
tested against a real one.
"""

from __future__ import annotations

import http.server
import json
import threading

import pytest

from mosaica import _control
from mosaica._control import (
    RETRY_BUDGET_S,
    UNANSWERED,
    Control,
    batch_id,
)
from mosaica._database import create
from mosaica._refusal import Refusal


class _Backpressure(http.server.BaseHTTPRequestHandler):
    """Answers `429` until `refusals` is spent, then `200`. Each `429` carries `retry_after` as
    its `Retry-After` header and `retry_after_s` in its body, where each is not `None`."""

    refusals = 2
    #: Requests to close without an answer before any is answered.
    silences = 0
    retry_after: str | None = "0.01"
    retry_after_s: float | None = 0.01
    bodies: list = []
    batches: list = []

    def do_POST(self) -> None:  # noqa: N802, the base class's spelling
        length = int(self.headers.get("content-length", "0"))
        _Backpressure.bodies.append(self.rfile.read(length))
        _Backpressure.batches.append(self.headers.get("x-mosaica-batch-id"))
        if _Backpressure.silences > 0:
            _Backpressure.silences -= 1
            self.close_connection = True
            return
        if _Backpressure.refusals > 0:
            _Backpressure.refusals -= 1
            said = {"error": "backpressure", "detail": "busy"}
            if _Backpressure.retry_after_s is not None:
                said["retry_after_s"] = _Backpressure.retry_after_s
            answer = json.dumps(said).encode()
            self.send_response(429)
            if _Backpressure.retry_after is not None:
                self.send_header("retry-after", _Backpressure.retry_after)
            self.send_header("content-length", str(len(answer)))
            self.end_headers()
            self.wfile.write(answer)
            return
        answer = json.dumps({"created": 1, "minted": 0, "mosaica_ids": ["7"]}).encode()
        self.send_response(200)
        self.send_header("content-length", str(len(answer)))
        self.end_headers()
        self.wfile.write(answer)

    def log_message(self, *args) -> None:
        return


@pytest.fixture
def waits(monkeypatch):
    """The waits `Control` sleeps between attempts, recorded and not slept, and the fake plane
    set to its defaults."""
    slept: list = []
    monkeypatch.setattr(_control, "sleep", slept.append)
    _Backpressure.refusals = 2
    _Backpressure.silences = 0
    _Backpressure.retry_after = "0.01"
    _Backpressure.retry_after_s = 0.01
    _Backpressure.bodies = []
    _Backpressure.batches = []
    return slept


def test_a_429_is_retried_after_its_retry_after_with_identical_bytes(waits):
    server = http.server.HTTPServer(("127.0.0.1", 0), _Backpressure)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        control = Control(f"http://127.0.0.1:{server.server_port}", "credential")
        body = b"one page of rows"
        answer = control.ingest(body, batch_id("points", 0), view="s0")
    finally:
        server.shutdown()
        server.server_close()
    assert answer.ok and answer.attempts == 3
    # Identical bytes under one batch id: a retry is a replay, never a second batch.
    assert _Backpressure.bodies == [body] * 3
    assert len(set(_Backpressure.batches)) == 1


def test_a_batch_id_is_fresh_per_request_and_never_derived_from_the_body():
    """A request is identified by an id the client chose, and identity is never inferred from
    what the request contains. Two pages carrying the same bytes are two requests, so the same
    frame committed twice is loaded twice."""
    assert batch_id("points", 0) != batch_id("points", 0)
    assert batch_id("points", 0).startswith("points-0-")


@pytest.fixture
def declared():
    """A database declaring three unique attributes, which is all `addresses` reads."""
    db = create()
    db.declare_attribute("paper", type="keyword", unique=True)
    db.declare_attribute("n", type="i64", unique=True)
    db.declare_attribute("ts", type="timestamp_us", unique=True)
    db.declare_attribute("title", type="keyword")
    yield db
    db.close()


def test_a_list_names_items_by_mosaica_id_and_a_table_by_its_columns_as_text(declared):
    """A list holds `mosaica_id`s; a table's rows name items by its `mosaica_id` and unique
    columns. Every value is text, and a null stays null."""
    assert declared.addresses([7, "8", None]) == [
        {"mosaica_id": "7"}, {"mosaica_id": "8"}, {"mosaica_id": None}
    ]
    assert declared.addresses({"paper": ["p3", None], "n": [None, 5]}) == [
        {"paper": "p3", "n": None},
        {"paper": None, "n": "5"},
    ]
    # A column that is neither `mosaica_id` nor a unique attribute names nothing and is left out.
    assert declared.addresses({"paper": ["p3"], "title": ["A title"]}) == [{"paper": "p3"}]
    # A frame's own ids arrive as numpy scalars, which are integers and are not `int`.
    numpy = pytest.importorskip("numpy")
    assert declared.addresses([numpy.uint64(2**63)]) == [{"mosaica_id": str(2**63)}]
    # A pandas frame's index is not one of its columns.
    pd = pytest.importorskip("pandas")
    frame = pd.DataFrame({"paper": ["a", "b", "c"]}).iloc[[2, 0]]
    assert declared.addresses(frame) == [{"paper": "c"}, {"paper": "a"}]


def test_a_timestamp_value_is_sent_as_its_microseconds_since_the_epoch(declared):
    """A timestamp field's values travel as decimal microseconds, whichever library holds them;
    one with no time zone is UTC."""
    import datetime

    pd = pytest.importorskip("pandas")
    numpy = pytest.importorskip("numpy")
    micros = str(1_577_836_800_000_005)
    naive = datetime.datetime(2020, 1, 1, 0, 0, 0, 5)
    held = [
        naive,
        naive.replace(tzinfo=datetime.timezone.utc),
        pd.Timestamp("2020-01-01 00:00:00.000005"),
        numpy.datetime64("2020-01-01T00:00:00.000005"),
    ]
    assert [declared.addresses({"ts": [one]}) for one in held] == [[{"ts": micros}]] * 4
    assert declared.addresses(pd.DataFrame({"ts": [held[2]]})) == [{"ts": micros}]


def serving():
    """The fake plane, started, with its address."""
    server = http.server.HTTPServer(("127.0.0.1", 0), _Backpressure)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, f"http://127.0.0.1:{server.server_port}"


def test_a_429_that_never_lets_up_is_answered_before_the_waits_pass_the_budget(waits):
    """The request is sent again while the waits total at most `RETRY_BUDGET_S`; the `429` whose
    wait would take them past it is the answer, which a commit's report lists."""
    _Backpressure.refusals = 1_000
    _Backpressure.retry_after = "7"
    server, base = serving()
    try:
        answer = Control(base, "credential").ingest(b"rows", batch_id("points", 0))
    finally:
        server.shutdown()
        server.server_close()
    assert not answer.ok and answer.status == 429
    assert waits == [7.0] * int(RETRY_BUDGET_S // 7)
    assert answer.attempts == len(waits) + 1 == len(_Backpressure.bodies)
    assert len(set(_Backpressure.batches)) == 1


@pytest.mark.parametrize(
    ("header", "body", "waited"),
    [("2", 5, 2.0), (None, 3, 3.0), (None, None, 1.0), ("0.01", 0.01, 1.0), ("0", None, 1.0)],
    ids=["header-first", "then-body", "then-one-second", "floor", "zero"],
)
def test_a_429_waits_its_header_then_its_body_then_one_second_and_never_less(
    waits, header, body, waited
):
    _Backpressure.refusals = 1
    _Backpressure.retry_after = header
    _Backpressure.retry_after_s = body
    server, base = serving()
    try:
        answer = Control(base, "credential").ingest(b"rows", batch_id("points", 0))
    finally:
        server.shutdown()
        server.server_close()
    assert answer.ok and waits == [waited]


def test_a_page_no_server_answered_is_sent_once_more_with_identical_bytes(waits):
    """The server answers a body it already accepted under the batch id as a replay, so the page
    goes once more, as it was, after the one-second wait."""
    _Backpressure.refusals = 0
    _Backpressure.silences = 1
    server, base = serving()
    try:
        answer = Control(base, "credential").ingest(b"rows", batch_id("points", 0))
    finally:
        server.shutdown()
        server.server_close()
    assert answer.ok and answer.attempts == 2 and waits == [1.0]
    assert _Backpressure.bodies == [b"rows"] * 2
    assert len(set(_Backpressure.batches)) == 1


def test_a_page_unanswered_twice_is_an_answer_and_not_an_exception(waits):
    """A page sent once more and still unanswered is a refusal the report carries: whether it
    landed is the database's to say."""
    _Backpressure.silences = 3
    server, base = serving()
    try:
        answer = Control(base, "credential").ingest(b"rows", batch_id("points", 0))
    finally:
        server.shutdown()
        server.server_close()
    assert not answer.ok and answer.status == UNANSWERED
    assert answer.attempts == 2 and _Backpressure.bodies == [b"rows"] * 2
    assert "did not answer" in answer.detail


def test_the_resend_takes_nothing_from_the_429_budget(waits):
    """After a resend, a `429` is still waited out for the whole budget."""
    _Backpressure.silences = 1
    _Backpressure.refusals = 1_000
    _Backpressure.retry_after = "7"
    server, base = serving()
    try:
        answer = Control(base, "credential").ingest(b"rows", batch_id("points", 0))
    finally:
        server.shutdown()
        server.server_close()
    assert answer.status == 429
    assert waits == [1.0] + [7.0] * int(RETRY_BUDGET_S // 7)


def test_a_request_that_reaches_no_server_is_an_answer_and_not_an_exception(waits):
    """A request that never reached a server is a refusal the report carries."""
    server, base = serving()
    server.shutdown()
    server.server_close()
    answer = Control(base, "credential").changes([{"op": "delete", "match": {"mosaica_id": "1"}}])
    assert not answer.ok and answer.status == UNANSWERED and answer.attempts == 1
    assert "did not answer" in answer.detail


def test_a_commit_whose_pages_reach_no_server_reports_it_and_does_not_raise(
    tmp_path, monkeypatch, waits
):
    """A page that was never answered is a refusal the report carries, not an exception, and the
    pages after it are not sent."""
    from conftest import small
    from mosaica._database import Database

    db, rows = small(tmp_path / "db")
    try:
        assert db.commit().ok
        db.insert("map", rows, x="x", y="y", access="access")
        # The operator plane this commit's pages go to is an address nothing is listening on.
        server, base = serving()
        server.shutdown()
        server.server_close()
        monkeypatch.setattr(Database, "control", property(lambda self: Control(base, "c")))
        with pytest.raises(Refusal) as raised:
            db.commit()
        assert [one["status"] for one in raised.value.report.refusals] == [UNANSWERED]
    finally:
        db.close()
