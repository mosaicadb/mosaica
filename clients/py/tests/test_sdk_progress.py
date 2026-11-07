"""The progress a commit and a waited compaction show, and the outcome the wait returns.

A first commit's records are the build's, a later commit's are the ingest record it makes, and a
waited compaction's are read from `/control/status`. Each is tested through `progress=` with a
function that keeps the records, against a real server. The outcomes a real server cannot be made
to give on demand (a refused or failed compaction, a restart that lost the request, an answer
replaced before it was read) come from a scripted `/control/status`.
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


def test_both_commits_draw_their_progress_by_default(tmp_path):
    db, rows = small(tmp_path / "db")
    try:
        assert db.commit().ok
        db.insert("map", rows.assign(id=[3, 4]), x="x", y="y", access="access")
        assert db.commit().ok
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


def test_a_waited_compaction_ends_with_the_outcome_the_server_served(tmp_path):
    db, _ = small(tmp_path / "db")
    records: list[dict] = []
    try:
        assert db.commit(progress=False).ok
        assert db.remove({"id": [1]}).accepted == 1
        answer = db.control.compact(wait=True, progress=records.append)
        served = db.status()["compaction"]
        waited = answer.waited
        assert answer.status == 202 and set(answer.body) == {"request"}
        assert (waited["ok"], waited["ended"]) == (True, "answered")
        assert waited["last_answer"] == served["requests"]["last_answer"]
        assert waited["last_answer"]["outcome"] in ("compacted", "nothing_to_do")
        assert {one["job"] for one in records} <= {"compaction"}
        if waited["last_answer"]["outcome"] == "compacted":
            assert waited["last_job"] == served["last_job"] == records[-1]
            assert records[-1]["state"] == "finished"
        else:
            assert "last_job" not in waited

        assert db.remove({"id": [2]}).accepted == 1
        waited = db.compact(wait=True, progress=False)
        assert waited["ok"] and waited["last_answer"]["first"] <= waited["request"]
    finally:
        db.close()


# ---------------------------------------------------------------------------- scripted outcomes


def job(state: str = "running", step: str = "row_space") -> dict:
    return {"job": "compaction", "state": state,
            "phase": {"index": 1, "of": 4, "name": "Rewriting rows"}, "step": step,
            "context": "shard 0", "elapsed_s": 1.0, "step_elapsed_s": 1.0, "rss_bytes": 1}


def status(accepted: int, answered: int, last_answer=None, jobs=(), last_job=None) -> dict:
    return {"jobs": list(jobs),
            "compaction": {"requests": {"accepted": accepted, "answered": answered,
                                        "last_answer": last_answer, "last_refusal": None},
                           "last_job": last_job}}


def answer(first: int, last: int, outcome: str, gate=None, detail=None) -> dict:
    return {"first": first, "last": last, "outcome": outcome, "gate": gate, "detail": detail}


class _Scripted(http.server.BaseHTTPRequestHandler):
    """Numbers a compaction request `request`, and answers each read of `/control/status` with the
    next of `statuses`, the last one again once they run out, or with `refusal` where it is set."""

    request = 3
    statuses: list = []
    refusal: int | None = None

    def do_POST(self) -> None:  # noqa: N802, the base class's spelling
        self.answer(202, {"request": _Scripted.request})

    def do_GET(self) -> None:  # noqa: N802
        if _Scripted.refusal is not None:
            self.answer(_Scripted.refusal, {"error": "forbidden", "detail": "no"})
            return
        body = _Scripted.statuses.pop(0) if len(_Scripted.statuses) > 1 else _Scripted.statuses[0]
        self.answer(200, body)

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
    _Scripted.request = 3
    _Scripted.statuses = []
    _Scripted.refusal = None
    server, base = serving(_Scripted)
    yield base
    server.shutdown()
    server.server_close()


def waited(base: str) -> tuple:
    records: list[dict] = []
    answered = Control(base, "credential").compact(wait=True, progress=records.append)
    assert answered.ok and answered.body == {"request": _Scripted.request}
    return answered.waited, records


def test_a_refused_request_returns_the_served_answer_and_ends_on_no_job(scripted):
    refused = answer(2, 3, "refused", "disk", "the disc is full; free some space")
    _Scripted.statuses = [
        status(3, 1, jobs=[job()]),
        status(3, 3, refused, last_job=job("finished")),
    ]
    waited_, records = waited(scripted)
    assert waited_ == {"ok": False, "ended": "answered", "last_answer": refused}
    assert records == [job()]


def test_a_failed_compaction_ends_on_the_last_job_as_served(scripted):
    failed = job("failed", "postings")
    _Scripted.statuses = [
        status(3, 2, jobs=[job()]),
        status(3, 2, jobs=[job(step="postings")]),
        status(3, 3, answer(3, 3, "failed"), last_job=failed),
    ]
    waited_, records = waited(scripted)
    assert waited_ == {"ok": False, "ended": "answered", "last_answer": answer(3, 3, "failed"),
                       "last_job": failed}
    assert records == [job(), job(step="postings"), failed]


def test_a_request_with_nothing_to_compact_shows_no_last_job(scripted):
    _Scripted.statuses = [status(3, 3, answer(3, 3, "nothing_to_do"), last_job=job("finished"))]
    waited_, records = waited(scripted)
    assert waited_ == {"ok": True, "ended": "answered", "last_answer": answer(3, 3, "nothing_to_do")}
    assert records == []


def test_a_server_that_restarted_after_the_request_has_lost_it(scripted):
    _Scripted.statuses = [status(1, 1, answer(1, 1, "compacted"))]
    waited_, _ = waited(scripted)
    assert waited_ == {"ok": False, "ended": "lost", "accepted": 1}


def test_an_answer_replaced_before_it_was_read_leaves_the_outcome_unknown(scripted):
    later = answer(4, 5, "compacted")
    _Scripted.statuses = [status(5, 5, later, last_job=job("finished"))]
    waited_, records = waited(scripted)
    assert waited_ == {"ok": False, "ended": "unknown", "last_answer": later}
    assert records == []


def test_a_refused_reading_of_the_status_ends_the_wait_with_its_answer(scripted):
    _Scripted.refusal = 403
    waited_, _ = waited(scripted)
    assert (waited_["ok"], waited_["ended"], waited_["status"].status) == (False, "unread", 403)


def test_a_database_raises_unless_the_request_compacted_or_had_nothing_to_do(
    tmp_path, monkeypatch, scripted
):
    db, _ = small(tmp_path / "db")
    try:
        assert db.commit(progress=False).ok
        monkeypatch.setattr(Database, "control", property(lambda self: Control(scripted, "c")))
        _Scripted.statuses = [status(3, 3, answer(3, 3, "nothing_to_do"))]
        assert db.compact(wait=True)["last_answer"]["outcome"] == "nothing_to_do"
        for script in (
            [status(3, 3, answer(3, 3, "refused", "disk", "the disc is full"))],
            [status(3, 3, answer(3, 3, "failed"), last_job=job("failed"))],
            [status(1, 1)],
            [status(5, 5, answer(4, 5, "compacted"))],
        ):
            _Scripted.statuses = script
            with pytest.raises(Refusal):
                db.compact(wait=True)
        _Scripted.refusal = 403
        with pytest.raises(Refusal):
            db.compact(wait=True)
    finally:
        db.close()
