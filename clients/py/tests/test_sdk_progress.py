"""The progress a commit and a waited compaction show, and the outcome the wait returns.

A first commit's records are the build's, a later commit's are the ingest record it makes, and a
waited compaction's are read from `GET /control/compact/{request}`. Each is tested through
`progress=` with a function that keeps the records, against a real server. The outcomes a real
server cannot be made to give on demand (a refused or failed compaction, a restart that lost the
request, a refused reading) come from a scripted route.
"""

from __future__ import annotations

import http.server
import json
import os
from pathlib import Path

import pytest

from conftest import serving, small
from mosaica import _control
from mosaica._control import Control
from mosaica._database import Database
from mosaica._refusal import Refusal


def phases_never_go_back(records: list[dict]) -> bool:
    indices = [one["phase"]["index"] for one in records]
    return indices == sorted(indices)


def holds_no_record(text: str) -> bool:
    """Whether no line of `text` begins as a serialised progress record does, with `{"job"`."""
    return not any(line.lstrip().startswith('{"job"') for line in text.splitlines())


# ---------------------------------------------------------------------------- the commits


def test_a_first_commit_gives_the_build_records_in_order_and_keeps_its_log(tmp_path):
    db, rows = small(tmp_path / "db")
    records: list[dict] = []
    try:
        report = db.commit(progress=records.append)
        assert report.ok and report.items == len(rows)
        assert records and {one["job"] for one in records} == {"build"}
        assert phases_never_go_back(records)
        assert records[-1]["state"] == "finished"
        assert [one for one in records[:-1] if one["state"] != "running"] == []
        assert holds_no_record(report.log)
    finally:
        db.close()


def test_a_first_commit_without_progress_writes_nothing_to_stderr(tmp_path, capfd):
    db, rows = small(tmp_path / "db")
    try:
        capfd.readouterr()
        report = db.commit(progress=False)
        assert report.ok and report.items == len(rows)
        assert capfd.readouterr().err == ""
    finally:
        db.close()


def test_commits_and_a_waited_compaction_draw_their_progress_by_default(tmp_path):
    db, rows = small(tmp_path / "db")
    try:
        assert db.commit().ok
        db.insert("map", rows.assign(id=[3, 4]), x="x", y="y", access="access")
        assert db.commit().ok
        assert db.remove({"id": [1]}).accepted == 1
        assert db.compact(wait=True)["ok"]
    finally:
        db.close()


def test_a_later_commit_counts_every_row_of_its_plan_then_waits_for_publication(tmp_path):
    db, rows = small(tmp_path / "db")
    records: list[dict] = []
    try:
        assert db.commit(progress=False).ok
        db.insert("map", rows.assign(id=[3, 4]), x="x", y="y", access="access")
        report = db.commit(progress=records.append)
        assert report.ok and report.rows == len(rows)
        assert {one["job"] for one in records} == {"ingest"}
        assert phases_never_go_back(records)
        pages = [one for one in records if one["step"] == "pages"]
        assert pages[-1]["done"] == pages[-1]["total"] == len(rows)
        assert [one["step"] for one in records if one["phase"]["index"] == 2][0] == "publication"
        assert records[-1]["state"] == "finished"
    finally:
        db.close()


def test_a_later_commit_whose_pages_reach_no_server_ends_failed_without_waiting(
    tmp_path, monkeypatch
):
    db, rows = small(tmp_path / "db")
    records: list[dict] = []
    try:
        assert db.commit(progress=False).ok
        db.insert("map", rows.assign(id=[3, 4]), x="x", y="y", access="access")
        # The operator plane this commit's pages go to is an address nothing is listening on, and
        # the wait before a page is sent once more is not slept.
        server, base = serving(http.server.BaseHTTPRequestHandler)
        server.shutdown()
        server.server_close()
        monkeypatch.setattr(_control, "sleep", lambda seconds: None)
        monkeypatch.setattr(Database, "control", property(lambda self: Control(base, "c")))
        with pytest.raises(Refusal):
            db.commit(progress=records.append)
        assert records[-1]["state"] == "failed"
        assert "publication" not in {one["step"] for one in records}
    finally:
        db.close()


class _Stopped(Exception):
    pass


def builds_running(path: Path) -> list[str]:
    """The processes whose command line is a `mosaica build` of the database at `path`."""
    found = []
    for entry in Path("/proc").iterdir():
        try:
            line = (entry / "cmdline").read_bytes().split(b"\0")
        except OSError:
            continue
        words = [word.decode(errors="replace") for word in line]
        if "build" in words and any(str(path) in word for word in words):
            found.append(entry.name)
    return found


@pytest.mark.skipif(not os.path.isdir("/proc"), reason="the build's process is looked for in /proc")
def test_a_sink_that_raises_stops_the_first_commit_and_a_later_one_builds(tmp_path):
    db, rows = small(tmp_path / "db")

    def stop(one: dict) -> None:
        raise _Stopped(one["step"])

    try:
        with pytest.raises(_Stopped):
            db.commit(progress=stop)
        assert not db.built
        assert builds_running(db.path) == []
        report = db.commit(progress=False)
        assert report.ok and report.items == len(rows)
    finally:
        db.close()


def test_a_progress_that_is_neither_a_flag_nor_a_function_is_refused_before_anything_runs(
    tmp_path,
):
    db, _ = small(tmp_path / "db")
    try:
        with pytest.raises(Refusal):
            db.commit(progress="bar")
        assert not db.built
    finally:
        db.close()


# ---------------------------------------------------------------------------- a real compaction


def test_a_waited_compaction_of_a_deletion_ends_compacted_with_its_record(tmp_path):
    db, _ = small(tmp_path / "db")
    records: list[dict] = []
    try:
        assert db.commit(progress=False).ok
        assert db.remove({"id": [1]}).accepted == 1
        answer = db.control.compact(wait=True, progress=records.append)
        waited = answer.waited
        assert answer.status == 202 and set(answer.body) == {"request"}
        assert (waited["ok"], waited["ended"]) == (True, "answered")
        assert waited["answer"]["outcome"] == "compacted"
        assert waited["answer"]["first"] <= answer.body["request"] <= waited["answer"]["last"]
        assert {one["job"] for one in records} == {"compaction"}
        assert waited["job"] == records[-1] and records[-1]["state"] == "finished"
        served = db.control.compaction_request(answer.body["request"]).body
        assert (served["answered"], served["answer"], served["job"]) == (
            True, waited["answer"], waited["job"])
    finally:
        db.close()


def test_a_request_the_server_never_numbered_is_not_found(tmp_path):
    db, _ = small(tmp_path / "db")
    try:
        assert db.commit(progress=False).ok
        accepted = db.status()["compaction"]["requests"]["accepted"]
        assert db.control.compaction_request(accepted + 1).status == 404
        # A number an earlier run gave is below every number this run gives.
        assert db.control.compaction_request(1).status == 404
    finally:
        db.close()


def test_a_request_number_is_awaited_on_its_own(tmp_path):
    db, _ = small(tmp_path / "db")
    try:
        assert db.commit(progress=False).ok
        assert db.remove({"id": [1]}).accepted == 1
        request = db.control.compact().body["request"]
        waited = db.control.await_compaction(request, progress=False)
        assert (waited["ok"], waited["ended"], waited["answer"]["outcome"]) == (
            True, "answered", "compacted")
    finally:
        db.close()


# ---------------------------------------------------------------------------- scripted outcomes

#: A request number as a server numbers them, from the wall clock in milliseconds when it started.
REQUEST = 1_786_543_210_123


def job(state: str = "running", step: str = "row_space", shard: int = 0) -> dict:
    return {"job": "compaction", "state": state,
            "phase": {"index": 1, "of": 4, "name": "Rewriting rows"}, "step": step,
            "context": f"shard {shard}", "elapsed_s": 1.0 + shard, "step_elapsed_s": 1.0,
            "rss_bytes": 1}


def pending(running: dict | None = None) -> tuple:
    return 200, {"request": REQUEST, "answered": False, "answer": None, "job": running}


def answered(outcome: str, ran: dict | None = None, **refusal) -> tuple:
    answer = {"first": REQUEST - 1, "last": REQUEST, "outcome": outcome,
              "ok": outcome in ("compacted", "nothing_to_do"), "gate": None, "detail": None,
              "shard": None, "need_bytes": None, "had_bytes": None, **refusal}
    return 200, {"request": REQUEST, "answered": True, "answer": answer, "job": ran}


#: A shard refused for want of disc, as the server answers one.
REFUSED = {"gate": "disk", "detail": "the shard needs more free disc than the volume has",
           "shard": 2, "need_bytes": 6_442_450_944, "had_bytes": 1_073_741_824}

#: A reading that closes the connection without an answer.
SILENT = (None, None)


class _Scripted(http.server.BaseHTTPRequestHandler):
    """Numbers a compaction request `REQUEST`, and answers each reading of its route with the next
    of `readings`, a status and a body, the last one again once they run out. `SILENT` closes the
    connection without an answer."""

    readings: list = []

    def do_POST(self) -> None:  # noqa: N802, the base class's spelling
        self.answer(202, {"request": REQUEST})

    def do_GET(self) -> None:  # noqa: N802
        if self.path != f"/control/compact/{REQUEST}":
            self.answer(404, {"error": "not_found", "detail": self.path})
            return
        many = _Scripted.readings
        code, body = many.pop(0) if len(many) > 1 else many[0]
        if code is None:
            self.close_connection = True
            return
        self.answer(code, body)

    def answer(self, code: int, body: dict) -> None:
        text = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("content-length", str(len(text)))
        self.end_headers()
        self.wfile.write(text)

    def log_message(self, *args) -> None:
        return


@pytest.fixture
def scripted(monkeypatch):
    """A control plane that answers from a script, read with no interval between readings, and
    its address."""
    monkeypatch.setattr(_control, "COMPACT_POLL_S", 0.0)
    _Scripted.readings = []
    server, base = serving(_Scripted)
    yield base
    server.shutdown()
    server.server_close()


def waited(base: str) -> tuple:
    records: list[dict] = []
    accepted = Control(base, "credential").compact(wait=True, progress=records.append)
    assert accepted.ok and accepted.body == {"request": REQUEST}
    return accepted.waited, records


@pytest.mark.parametrize("outcome", ["compacted", "nothing_to_do", "failed"])
def test_an_answered_request_ends_on_the_record_of_the_compaction_that_answered_it(
    scripted, outcome
):
    ended = job("failed" if outcome == "failed" else "finished", "postings", shard=1)
    ran = None if outcome == "nothing_to_do" else ended
    _Scripted.readings = [pending(), pending(job()), pending(job(shard=1)), answered(outcome, ran)]
    waited_, records = waited(scripted)
    served = answered(outcome)[1]["answer"]
    assert waited_ == {"ok": served["ok"], "ended": "answered", "answer": served, "job": ran}
    assert records == [job(), job(shard=1)] + ([ran] if ran else [])


def test_a_refused_request_returns_the_refusal_as_served(scripted):
    _Scripted.readings = [answered("refused", **REFUSED)]
    waited_, records = waited(scripted)
    assert (waited_["ok"], waited_["ended"]) == (False, "answered")
    assert waited_["answer"] == answered("refused", **REFUSED)[1]["answer"]
    assert records == []


def test_a_reading_no_server_answered_is_read_again(scripted):
    _Scripted.readings = [pending(job()), SILENT, answered("compacted", job("finished"))]
    waited_, records = waited(scripted)
    assert (waited_["ok"], waited_["ended"]) == (True, "answered")
    assert records == [job(), job("finished")]


@pytest.mark.parametrize(("code", "ended"), [(404, "lost"), (410, "gone"), (403, "unread")])
def test_a_reading_the_server_refused_ends_the_wait_with_it(scripted, code, ended):
    _Scripted.readings = [pending(job()), (code, {"error": "no", "detail": "no"})]
    waited_, records = waited(scripted)
    assert (waited_["ok"], waited_["ended"], waited_["read"].status) == (False, ended, code)
    assert records == [job()]


def test_the_wait_takes_a_number_on_its_own(scripted):
    _Scripted.readings = [pending(job()), answered("nothing_to_do")]
    records: list[dict] = []
    waited_ = Control(scripted, "c").await_compaction(REQUEST, progress=records.append)
    assert (waited_["ok"], waited_["ended"], waited_["job"]) == (True, "answered", None)
    assert records == [job()]


def test_a_database_raises_unless_the_request_compacted_or_had_nothing_to_do(
    tmp_path, monkeypatch, scripted
):
    db, _ = small(tmp_path / "db")
    try:
        assert db.commit(progress=False).ok
        monkeypatch.setattr(Database, "control", property(lambda self: Control(scripted, "c")))
        _Scripted.readings = [answered("nothing_to_do")]
        assert db.compact(wait=True)["answer"]["outcome"] == "nothing_to_do"
        for reading, status in (
            (answered("refused", **REFUSED), None),
            (answered("failed", job("failed")), None),
            ((404, {"error": "not_found", "detail": "lost"}), 404),
            ((410, {"error": "gone", "detail": "gone"}), 410),
            ((403, {"error": "forbidden", "detail": "no"}), 403),
        ):
            _Scripted.readings = [reading]
            with pytest.raises(Refusal) as raised:
                db.compact(wait=True)
            assert raised.value.report["request"] == REQUEST and not raised.value.report["ok"]
            assert raised.value.status == status
    finally:
        db.close()
