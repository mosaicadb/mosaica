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


#: A committed temporary database in a process that then ends as `how` says, after printing its
#: directory and its server's pid.
ENDED = """
    import json, os, signal, sys, time
    import pyarrow as pa
    import mosaica

    how = sys.argv[1]
    db = mosaica.create()
    db.declare_view("map", extent={"x": [0, 10], "y": [0, 10]})
    db.insert("map", pa.table({"x": [1.0, 2.0], "y": [1.0, 2.0]}), x="x", y="y")
    db.commit()
    print(json.dumps({"path": str(db.path), "pid": db._child.pid}), flush=True)
    if how == "os._exit":
        # How a marimo kernel leaves: its SIGTERM handler exits without running exit handlers.
        os._exit(0)
    time.sleep(120)
"""


def gone(path: str, pid: int, within: float = 15.0) -> bool:
    deadline = time.monotonic() + within
    while time.monotonic() < deadline:
        try:
            os.kill(pid, 0)
        except OSError:
            if not os.path.exists(path):
                return True
        time.sleep(0.1)
    return False


@pytest.mark.parametrize("how", ["os._exit", "SIGTERM", "SIGKILL"])
def test_a_temporary_database_is_removed_however_its_process_ends(how):
    from conftest import binary

    binary()
    process = subprocess.Popen(
        [sys.executable, "-c", textwrap.dedent(ENDED), how],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        left = json.loads(process.stdout.readline())
        if how != "os._exit":
            os.kill(process.pid, getattr(signal, how))
        process.wait(timeout=30)
        assert gone(left["path"], left["pid"]), process.stderr.read()
    finally:
        if process.poll() is None:
            process.kill()


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
        assert gone(path, pid, within=10)
    finally:
        if collecting:
            gc.enable()


def test_a_temporary_database_is_removed_when_marimo_exports_its_notebook(tmp_path):
    """marimo runs the notebook in a kernel that leaves through `os._exit` and kills its process
    group, so no exit handler of this package runs."""
    from conftest import binary

    binary()
    where = tmp_path / "left.json"
    cell = textwrap.indent(
        textwrap.dedent(ENDED)
        .replace("how = sys.argv[1]", "how = 'marimo'")
        .replace("time.sleep(120)", "")
        .replace("print(json.dumps(", f"open({str(where)!r}, 'w').write(json.dumps(")
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
    assert where.exists(), done.stderr
    left = json.loads(where.read_text())
    assert gone(left["path"], left["pid"]), done.stderr


def test_a_temporary_database_is_removed_when_a_jupyter_kernel_shuts_down():
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
        cell = textwrap.dedent(ENDED).replace("time.sleep(120)", "").replace(
            "how = sys.argv[1]", "how = 'kernel'"
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
    assert gone(left["path"], left["pid"])
