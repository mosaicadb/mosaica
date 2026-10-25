"""The supervisor: the deployment file, the announce line, and stopping the child by its pid (§7).

The announce line is read from a fake child here, a Python script printing what `mosaica serve`
prints, so the parser is covered without a bundle and without the binary.
"""

import json
import os
import signal
import subprocess
import sys
import textwrap
import time

import pytest

from mosaica import _instance

ANNOUNCE = (
    '{"event": "listening", "viewer": "127.0.0.1:38001", "session": "127.0.0.1:38002", '
    '"control": "127.0.0.1:38003"}'
)


def child(script: str) -> subprocess.Popen:
    return subprocess.Popen(
        [sys.executable, "-c", textwrap.dedent(script)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
    )


def test_the_announce_line_is_read_past_the_childs_own_logging():
    process = child(
        f"""
        import sys
        print("INFO the allocator's arena count is capped")
        print("INFO the engine adopted the prefix's derived artifact structures")
        print({ANNOUNCE!r})
        sys.stdout.flush()
        input()
        """
    )
    try:
        listening = _instance.read_announce(process.stdout, 10, lambda: "")
        assert listening.viewer == "127.0.0.1:38001"
        assert listening.session == "127.0.0.1:38002"
        assert listening.control == "127.0.0.1:38003"
    finally:
        _instance.stop(process)


def test_a_unix_control_plane_is_carried_through_as_written():
    line = (
        '{"event":"listening","viewer":"127.0.0.1:1","session":"127.0.0.1:2",'
        '"control":"unix:/run/mosaica/control.sock"}'
    )
    process = child(f"""
        import sys
        print({line!r}); sys.stdout.flush(); input()
        """)
    try:
        assert _instance.read_announce(process.stdout, 10, lambda: "").control == (
            "unix:/run/mosaica/control.sock"
        )
    finally:
        _instance.stop(process)


def test_a_child_that_announces_nothing_times_out_carrying_its_stderr():
    process = child(
        """
        import sys
        print("INFO listening on something the SDK cannot read")
        print("refused to start: config io error", file=sys.stderr)
        sys.stdout.flush(); sys.stderr.flush()
        input()
        """
    )
    try:
        errors = _instance._drain(process.stderr)
        with pytest.raises(_instance.ServeRefused) as refusal:
            _instance.read_announce(process.stdout, 1.0, lambda: "".join(errors))
        assert '"event": "listening"' in str(refusal.value)
        assert "config io error" in str(refusal.value)
    finally:
        _instance.stop(process)


def test_a_listening_line_missing_an_address_is_refused_rather_than_guessed():
    process = child("""
        import sys
        print('{"event": "listening", "viewer": "127.0.0.1:1"}'); sys.stdout.flush(); input()
        """)
    try:
        with pytest.raises(_instance.ServeRefused):
            _instance.read_announce(process.stdout, 5, lambda: "")
    finally:
        _instance.stop(process)


def test_a_child_that_exits_is_not_waited_out():
    process = child("print('INFO nothing to say')")
    with pytest.raises(_instance.ServeRefused):
        _instance.read_announce(process.stdout, 30, lambda: "")


def test_stop_kills_by_pid_and_leaves_nothing_running():
    process = child("input()")
    pid = process.pid
    _instance.stop(process)
    assert process.poll() is not None
    with pytest.raises(OSError):
        os.kill(pid, 0)
    assert pid not in _instance._running


def test_the_deployment_file_names_three_loopback_planes_at_port_zero(tmp_path):
    path = _instance.write_deployment(tmp_path)
    text = path.read_text()
    assert 'viewer = "127.0.0.1:0"' in text
    assert 'session = "127.0.0.1:0"' in text
    assert 'control = "127.0.0.1:0"' in text
    assert "[disclosure]\ntoken_max_lifetime = 3600" in text
    # The widget's page is served from a loopback address, and its origin cannot be enumerated.
    assert "cors_loopback = true" in text
    assert "cors_origins" not in text
    assert (tmp_path / ".mosaica" / "cache").is_dir()


def test_the_secrets_are_generated_once_and_owner_only(tmp_path):
    _instance.secrets_for(tmp_path)
    written = {
        name: (tmp_path / ".mosaica" / name).read_text() for name in ("operator.cred",)
    }
    for name in written:
        assert oct((tmp_path / ".mosaica" / name).stat().st_mode)[-3:] == "600"
    # The directory too, and each file is created owner-only rather than narrowed afterwards.
    assert oct((tmp_path / ".mosaica").stat().st_mode)[-3:] == "700"
    _instance.secrets_for(tmp_path)
    assert written == {
        name: (tmp_path / ".mosaica" / name).read_text() for name in written
    }


def test_the_binary_is_found_at_mosaica_bin(tmp_path, monkeypatch):
    binary = tmp_path / "mosaica"
    binary.write_text("")
    monkeypatch.setenv("MOSAICA_BIN", str(binary))
    assert _instance.find_binary() == (str(binary), "MOSAICA_BIN")
    monkeypatch.setenv("MOSAICA_BIN", str(tmp_path / "absent"))
    with pytest.raises(Exception):
        _instance.find_binary()


def test_no_binary_anywhere_names_the_companion_wheel_at_this_packages_version(
    tmp_path, monkeypatch
):
    import mosaica

    monkeypatch.delenv("MOSAICA_BIN", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))
    monkeypatch.setitem(sys.modules, "mosaica_native", None)
    monkeypatch.setattr(_instance, "__file__", str(tmp_path / "site" / "mosaica" / "_instance.py"))
    with pytest.raises(Exception) as refused:
        _instance.find_binary()
    # The unpinned name installs a placeholder, so the pin is what the refusal must carry.
    assert f"mosaica-native=={mosaica.__version__}" in str(refused.value)


#: A committed database, left open at interpreter exit. `where` is `None` for a temporary one.
LEFT_OPEN = """
    import json, sys
    import pyarrow as pa
    import mosaica as td

    where = json.loads(sys.argv[1])
    db = td.create(where)
    db.declare_view("map", extent={"x": [0, 10], "y": [0, 10]})
    rows = pa.table({"x": [1.0, 2.0], "y": [1.0, 2.0], "access": [["public"], ["public"]]})
    db.insert("map", rows, x="x", y="y", access="access")
    db.commit()
    saved = json.loads(sys.argv[2])
    if saved is not None:
        db.save(saved)
        td.open(saved).serve()
    print(json.dumps({"path": str(db.path), "pid": db._child.pid}))
"""


def left_open(where, saved=None) -> dict:
    from conftest import binary

    binary()
    done = subprocess.run(
        [sys.executable, "-c", textwrap.dedent(LEFT_OPEN), json.dumps(where), json.dumps(saved)],
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert done.returncode == 0, done.stderr
    left = json.loads(done.stdout.strip().splitlines()[-1])
    with pytest.raises(OSError):
        os.kill(left["pid"], 0)
    return left


def test_a_temporary_database_is_removed_at_interpreter_exit(tmp_path):
    saved = tmp_path / "saved"
    left = left_open(None, str(saved))
    assert not os.path.exists(left["path"])
    # The copy it was saved to, which a second database opened and served, is the user's.
    assert (saved / "mosaica.toml").exists()


def test_a_database_at_a_path_the_user_named_is_kept_at_interpreter_exit(tmp_path):
    left = left_open(str(tmp_path / "db"))
    assert (tmp_path / "db" / "mosaica.toml").exists()
    assert left["path"] == str(tmp_path / "db")


#: A temporary database in `where`, committed unless `how` is "uncommitted", in a process that then
#: ends as `how` says, after printing its directory, its server's pid and any child it forked.
ENDED = """
    import json, os, pathlib, signal, sys, time
    import pyarrow as pa
    import mosaica
    from mosaica import _database

    how, where = sys.argv[1], sys.argv[2]
    _database.RAM_BACKED = pathlib.Path(where)
    db = mosaica.create()
    db.declare_view("map", extent={"x": [0, 10], "y": [0, 10]})
    db.insert("map", pa.table({"x": [1.0, 2.0], "y": [1.0, 2.0]}), x="x", y="y")
    if how != "uncommitted":
        db.commit()
    forked = None
    if how == "fork":
        forked = os.fork()
        if forked == 0:
            time.sleep(120)
            os._exit(0)
    pid = None if db._child is None else db._child.pid
    print(json.dumps({"path": str(db.path), "pid": pid, "forked": forked}), flush=True)
    if how in ("os._exit", "uncommitted", "fork"):
        # How a marimo kernel leaves: its SIGTERM handler exits without running exit handlers.
        os._exit(0)
    time.sleep(120)
"""


def stopped(pid: int | None, within: float = 15.0) -> bool:
    deadline = time.monotonic() + within
    while pid is not None and time.monotonic() < deadline:
        try:
            os.kill(pid, 0)
        except OSError:
            return True
        time.sleep(0.1)
    return pid is None


def swept(where, left: dict, monkeypatch) -> bool:
    """Whether the next `create()` in `where` removes the directory `left` names."""
    import mosaica
    from mosaica import _database

    monkeypatch.setattr(_database, "RAM_BACKED", where)
    fresh = mosaica.create()
    try:
        return not os.path.exists(left["path"])
    finally:
        fresh.close()


def ended(how: str, where) -> tuple[subprocess.Popen, dict]:
    process = subprocess.Popen(
        [sys.executable, "-c", textwrap.dedent(ENDED), how, str(where)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return process, json.loads(process.stdout.readline())


@pytest.mark.parametrize("how", ["os._exit", "SIGTERM", "SIGKILL", "uncommitted"])
def test_a_temporary_database_is_removed_however_its_process_ends(how, tmp_path, monkeypatch):
    """The server stops as the process ends, and the next `create()` removes the directory the
    process could not, an uncommitted database's among them."""
    from conftest import binary

    binary()
    where = tmp_path / "shm"
    where.mkdir()
    process, left = ended(how, where)
    try:
        if how in ("SIGTERM", "SIGKILL"):
            os.kill(process.pid, getattr(signal, how))
        process.wait(timeout=30)
        assert stopped(left["pid"]), process.stderr.read()
        assert os.path.exists(left["path"]), "nothing ran in the process to remove it"
        assert swept(where, left, monkeypatch)
    finally:
        if process.poll() is None:
            process.kill()


def test_a_forked_child_keeps_neither_the_server_nor_the_directory(tmp_path, monkeypatch):
    """A child forked after the server started inherits the pipe to its standard input and the
    directory's lock, and lets both go as it starts."""
    from conftest import binary

    binary()
    where = tmp_path / "shm"
    where.mkdir()
    process, left = ended("fork", where)
    try:
        process.wait(timeout=30)
        assert left["forked"] and os.kill(left["forked"], 0) is None, "the child is alive"
        assert stopped(left["pid"]), process.stderr.read()
        assert swept(where, left, monkeypatch)
    finally:
        if left.get("forked"):
            try:
                os.kill(left["forked"], signal.SIGKILL)
            except OSError:
                pass


def test_the_sweep_removes_only_directories_whose_owner_has_gone(tmp_path, monkeypatch):
    import mosaica
    from mosaica import _database, _instance

    where = tmp_path / "shm"
    where.mkdir()
    monkeypatch.setattr(_database, "RAM_BACKED", where)
    live = mosaica.create()
    # Not candidates: a directory without the lock file, one not named for the package, and a
    # link to a directory holding an unlocked lock file.
    (where / "mosaica-without-a-lock").mkdir()
    (where / "elsewhere").mkdir()
    (where / "elsewhere" / _instance.LOCK).write_text("")
    outside = tmp_path / "outside"
    outside.mkdir()
    (outside / _instance.LOCK).write_text("")
    (where / "mosaica-link").symlink_to(outside)
    try:
        second = mosaica.create()
        second.close()
        assert live.path.exists(), "a directory its live owner holds is kept"
        assert (where / "mosaica-without-a-lock").exists()
        assert (where / "elsewhere" / _instance.LOCK).exists()
        assert (outside / _instance.LOCK).exists() and (where / "mosaica-link").is_symlink()
    finally:
        live.close()
    assert not live.path.exists()


def test_a_refused_start_leaves_the_temporary_database_to_retry_or_save(tmp_path, monkeypatch):
    from conftest import binary

    binary()
    import pyarrow as pa

    import mosaica

    db = mosaica.create()
    try:
        db.declare_view("map", extent={"x": [0, 10], "y": [0, 10]})
        db.insert("map", pa.table({"x": [1.0, 2.0], "y": [1.0, 2.0]}), x="x", y="y")
        monkeypatch.setenv("MOSAICA_SERVE_TIMEOUT", "0")
        with pytest.raises(_instance.ServeRefused):
            db.commit()
        assert (db.path / "mosaica.toml").exists() and (db.path / "sources").is_dir()
        assert db.save(tmp_path / "kept") == tmp_path / "kept"
        monkeypatch.delenv("MOSAICA_SERVE_TIMEOUT")
        assert db.view("map").count() == 2
    finally:
        db.close()


def test_a_database_nothing_refers_to_stops_its_server_and_removes_its_directory():
    from conftest import binary

    binary()
    import gc

    import pyarrow as pa

    import mosaica

    db = mosaica.create()
    db.declare_view("map", extent={"x": [0, 10], "y": [0, 10]})
    db.insert("map", pa.table({"x": [1.0, 2.0], "y": [1.0, 2.0]}), x="x", y="y")
    db.commit()
    selection = db.view("map")
    path, pid = str(db.path), db._child.pid
    collecting = gc.isenabled()
    gc.disable()
    try:
        db = None
        # A selection reads through its database, so the server runs while one is held.
        assert selection.count() == 2
        selection = None
        assert stopped(pid, within=10) and not os.path.exists(path)
    finally:
        if collecting:
            gc.enable()


def test_a_temporary_database_is_removed_when_marimo_exports_its_notebook(tmp_path, monkeypatch):
    """marimo runs the notebook in a kernel that leaves through `os._exit` and kills its process
    group, so no exit handler of this package runs: the server stops as the pipe closes, and the
    next `create()` removes the directory."""
    from conftest import binary

    binary()
    where = tmp_path / "shm"
    where.mkdir()
    written = tmp_path / "left.json"
    cell = textwrap.indent(
        textwrap.dedent(ENDED)
        .replace("how, where = sys.argv[1], sys.argv[2]", f"how, where = 'marimo', {str(where)!r}")
        .replace("time.sleep(120)\n", "")
        .replace("print(json.dumps(", f"open({str(written)!r}, 'w').write(json.dumps(")
        .replace("}), flush=True)", "}))"),
        "    ",
    )
    notebook = tmp_path / "notebook.py"
    notebook.write_text(
        "import marimo\napp = marimo.App()\n\n@app.cell\ndef _():\n" + cell
        + "    return\n\nif __name__ == '__main__':\n    app.run()\n"
    )
    done = subprocess.run(
        [sys.executable, "-m", "marimo", "export", "html", str(notebook), "-o",
         str(tmp_path / "out.html")],
        capture_output=True,
        text=True,
        timeout=300,
    )
    assert written.exists(), done.stderr
    left = json.loads(written.read_text())
    assert stopped(left["pid"]), done.stderr
    assert swept(where, left, monkeypatch)


def test_a_temporary_database_is_removed_when_a_jupyter_kernel_shuts_down(tmp_path):
    """A kernel shut down runs its exit handlers, which stop the server and remove the directory
    without waiting for a sweep."""
    from conftest import binary

    binary()
    jupyter_client = pytest.importorskip("jupyter_client")
    pytest.importorskip("ipykernel")
    manager = jupyter_client.KernelManager(kernel_name="python3")
    manager.start_kernel()
    client = manager.client()
    client.start_channels()
    try:
        client.wait_for_ready(timeout=60)
        where = tmp_path / "shm"
        where.mkdir()
        cell = textwrap.dedent(ENDED).replace("time.sleep(120)", "").replace(
            "how, where = sys.argv[1], sys.argv[2]", f"how, where = 'kernel', {str(where)!r}"
        )
        printed = ""
        client.execute(cell)
        while True:
            message = client.get_iopub_msg(timeout=120)
            content = message["content"]
            if message["msg_type"] == "stream":
                printed += content["text"]
            if message["msg_type"] == "error":
                pytest.fail("\n".join(content["traceback"]))
            if message["msg_type"] == "status" and content["execution_state"] == "idle":
                break
        left = json.loads(printed.strip().splitlines()[-1])
    finally:
        client.stop_channels()
        manager.shutdown_kernel()
    assert stopped(left["pid"]) and not os.path.exists(left["path"])
