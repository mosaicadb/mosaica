"""A server started with `mosaica serve --open`, read and written with no token and no credential."""

from __future__ import annotations

import subprocess

import pytest

from mosaica import Control, _instance, connect

from conftest import binary, small

pytest.importorskip("pandas")


def test_an_open_server_is_read_and_written_with_no_credential(tmp_path):
    db, rows = small(tmp_path / "db")
    report = db.commit()
    assert report.ok, report.log
    db.close()

    child = subprocess.Popen(
        [binary(), "serve", "--open", "--exit-on-stdin-close", "--deployment",
         str(tmp_path / "db" / "mosaica.toml")],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        cwd=str(tmp_path / "db"),
    )
    try:
        listening = _instance.read_announce(child.stdout, 60.0, lambda: "")
        assert connect(f"http://{listening.viewer}").view("map").count() == len(rows)
        control = Control(f"http://{listening.control}")
        assert control.flush().status == 202
        assert control.status_answer().status == 200
    finally:
        _instance.stop(child)
