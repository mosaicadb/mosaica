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
import sys
import threading

import pytest

from conftest import small
from mosaica import _control, _progress
from mosaica._control import Control
from mosaica._database import Database
from mosaica._refusal import Refusal


def phases_never_go_back(records: list[dict]) -> bool:
    indices = [one["phase"]["index"] for one in records]
    return indices == sorted(indices)


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
        assert not any(_progress.record(line) for line in report.log.splitlines())
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


def test_a_first_commit_without_tqdm_writes_plain_lines_to_stderr(tmp_path, capfd, monkeypatch):
    monkeypatch.setitem(sys.modules, "tqdm", None)
    monkeypatch.setitem(sys.modules, "tqdm.auto", None)
    db, _ = small(tmp_path / "db")
    try:
        capfd.readouterr()
        assert db.commit().ok
        lines = capfd.readouterr().err.splitlines()
        assert lines and not any(_progress.record(line) for line in lines)
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
        body = db.compact(wait=True, progress=records.append)
        served = db.status()["compaction"]
        assert body["outcome"] in ("compacted", "nothing_to_do")
        assert body["last_answer"]["first"] <= body["request"] <= body["last_answer"]["last"]
        assert {one["job"] for one in records} <= {"compaction"}
        if body["outcome"] == "compacted":
            assert body["last_job"] == served["last_job"] == records[-1]
            assert records[-1]["state"] == "finished"
        else:
            assert body["last_job"] is None
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
    """A control plane that answers from a script, with the reads' interval not slept, and its
    address."""
    monkeypatch.setattr(_control, "sleep", lambda seconds: None)
    _Scripted.request = 3
    _Scripted.statuses = []
    _Scripted.refusal = None
    server = http.server.HTTPServer(("127.0.0.1", 0), _Scripted)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    yield f"http://127.0.0.1:{server.server_port}"
    server.shutdown()
    server.server_close()


def waited(base: str) -> tuple:
    records: list[dict] = []
    answered = Control(base, "credential").compact(wait=True, progress=records.append)
    return answered, records


def test_a_refused_request_returns_the_served_gate_and_ends_on_no_job(scripted):
    refused = answer(2, 3, "refused", "disk", "the disc is full; free some space")
    _Scripted.statuses = [
        status(3, 1, jobs=[job()]),
        status(3, 3, refused, last_job=job("finished")),
    ]
    answered, records = waited(scripted)
    assert answered.ok and answered.status == 202
    assert answered.body == {"request": 3, "outcome": "refused", "last_answer": refused,
                             "last_job": None}
    assert records == [job()]


def test_a_failed_compaction_ends_on_the_last_job_as_served(scripted):
    failed = job("failed", "postings")
    _Scripted.statuses = [
        status(3, 2, jobs=[job()]),
        status(3, 2, jobs=[job(step="postings")]),
        status(3, 3, answer(3, 3, "failed"), last_job=failed),
    ]
    answered, records = waited(scripted)
    assert answered.body["outcome"] == "failed"
    assert answered.body["last_job"] == failed
    assert records == [job(), job(step="postings"), failed]


def test_a_request_with_nothing_to_compact_shows_no_last_job(scripted):
    _Scripted.statuses = [status(3, 3, answer(3, 3, "nothing_to_do"), last_job=job("finished"))]
    answered, records = waited(scripted)
    assert (answered.body["outcome"], answered.body["last_job"]) == ("nothing_to_do", None)
    assert records == []


def test_a_server_that_restarted_after_the_request_has_lost_it(scripted):
    _Scripted.statuses = [status(1, 1, answer(1, 1, "compacted"))]
    answered, _ = waited(scripted)
    assert answered.body["outcome"] == "lost"
    assert answered.body["last_answer"] is None


def test_an_answer_replaced_before_it_was_read_leaves_the_outcome_unknown(scripted):
    _Scripted.statuses = [status(5, 5, answer(4, 5, "compacted"), last_job=job("finished"))]
    answered, records = waited(scripted)
    assert answered.body["outcome"] == "unknown"
    assert answered.body["last_answer"] is None and records == []


def test_a_refused_read_of_the_status_is_the_answer_with_the_request_number(scripted):
    _Scripted.refusal = 403
    answered, _ = waited(scripted)
    assert answered.status == 403 and answered.body["request"] == 3


def test_a_database_raises_for_an_outcome_other_than_compacted_or_nothing_to_do(
    tmp_path, monkeypatch, scripted
):
    db, _ = small(tmp_path / "db")
    try:
        assert db.commit(progress=False).ok
        monkeypatch.setattr(Database, "control", property(lambda self: Control(scripted, "c")))
        _Scripted.statuses = [status(3, 3, answer(3, 3, "nothing_to_do"))]
        assert db.compact(wait=True)["outcome"] == "nothing_to_do"
        for outcome in ("refused", "failed"):
            _Scripted.statuses = [status(3, 3, answer(3, 3, outcome))]
            with pytest.raises(Refusal):
                db.compact(wait=True)
    finally:
        db.close()
