"""The local server: the deployment file, its secret, and the `mosaica serve` child process.

`commit()` starts `mosaica serve` as a child of the kernel over the directory's `mosaica.toml`.
The three planes are on loopback at port 0, so the kernel picks no ports and the child says which
ones it bound by printing one JSON line on stdout. The line is written once all three planes are
listening, so the announced viewer address is already answering when it arrives.

There is no fallback that guesses a port. A guess is a second process's port as readily as this
one's, and the notebook would then hold a URL answering for somebody else's server. A child that
announces nothing is a refusal carrying what the child wrote.

The child is killed by its pid, never by process name: a kill by name reaches every other server
on the machine. It is killed at `close()`, when the `Database` is no longer referenced, and at
interpreter exit. A temporary database's directory is removed at the same points, after its child
has stopped.

A thread waits on each child, so a child stopped from outside is reaped at once. A Jupyter kernel
stops its children at shutdown and waits until none is left, and an unreaped child would hold it
there until the kernel was killed, before its exit handlers ran.

Some processes end without running exit handlers: a marimo kernel leaves through `os._exit`, and
a kernel can be killed. So the first database a process makes or serves also starts a watchdog: a
small Python process in its own session, which this process tells of each child and temporary
directory through a pipe. When the pipe closes, which it does however this process ends, the
watchdog stops each child still running and removes each temporary directory.
"""

from __future__ import annotations

import atexit
import importlib
import json
import os
import queue
import secrets
import shutil
import signal
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from importlib.machinery import ExtensionFileLoader
from importlib.util import module_from_spec, spec_from_loader
from pathlib import Path
from types import ModuleType

from ._refusal import Refusal
from ._toml import dumps

#: How long `start` waits for the announce line before it gives up, in seconds, unless
#: `MOSAICA_SERVE_TIMEOUT` names another.
SERVE_TIMEOUT = 30.0

#: How many of the child's lines are kept for a refusal message.
KEPT_LINES = 200


def serve_timeout() -> float:
    return float(os.environ.get("MOSAICA_SERVE_TIMEOUT", SERVE_TIMEOUT))


#: The deployment's token lifetime, which `[disclosure]` requires and has no backstop default.
TOKEN_MAX_LIFETIME = 3600

_running: dict[int, subprocess.Popen] = {}

#: The directories of the temporary databases this process made and has not closed.
temporary: set[Path] = set()


class Held:
    """What one database holds outside this process: its server, and its directory where it is a
    temporary one. `release` gives both up; the `Database` holds no reference back, so a finaliser
    can release them once the `Database` is gone."""

    def __init__(self, path: Path, is_temporary: bool) -> None:
        self.path = path
        self.temporary = is_temporary
        self.child: subprocess.Popen | None = None
        if is_temporary:
            temporary.add(path)
            _tell({"temporary": str(path)})


def release(held: Held) -> None:
    """Stop the database's server, and remove its directory if it is a temporary one."""
    if held.child is not None:
        stop(held.child)
        held.child = None
    if held.temporary and held.path in temporary:
        remove(held.path)


def remove(directory: Path) -> None:
    shutil.rmtree(directory, ignore_errors=True)
    temporary.discard(directory)
    _tell({"removed": str(directory)})


#: The watchdog's program. It reads one JSON line per change to what the process holds, and acts
#: when its input ends. A pid is signalled only while its command line still names the deployment
#: file it was started with, so a pid the system has handed to another process is left alone.
_WATCHDOG = r"""
import json, os, shutil, signal, subprocess, sys, time

signal.signal(signal.SIGINT, signal.SIG_IGN)
signal.signal(signal.SIGHUP, signal.SIG_IGN)
servers, directories = {}, set()
for line in sys.stdin:
    message = json.loads(line)
    if "serve" in message:
        servers[message["serve"]] = message["deployment"]
    elif "stopped" in message:
        servers.pop(message["stopped"], None)
    elif "temporary" in message:
        directories.add(message["temporary"])
    elif "removed" in message:
        directories.discard(message["removed"])


def ours(pid, deployment):
    try:
        with open(f"/proc/{pid}/cmdline", "rb") as file:
            command = file.read().decode(errors="replace")
    except FileNotFoundError:
        return False
    except OSError:
        done = subprocess.run(["ps", "-o", "command=", "-p", str(pid)], capture_output=True, text=True)
        command = done.stdout
    return deployment in command


for number in (signal.SIGTERM, signal.SIGKILL):
    left = [pid for pid, deployment in servers.items() if ours(pid, deployment)]
    for pid in left:
        try:
            os.kill(pid, number)
        except OSError:
            pass
    deadline = time.monotonic() + 5
    while left and time.monotonic() < deadline:
        time.sleep(0.05)
        left = [pid for pid in left if ours(pid, servers[pid])]
for directory in directories:
    shutil.rmtree(directory, ignore_errors=True)
"""

_watchdog: subprocess.Popen | None = None
_told = threading.RLock()
#: Set once the exit handler has cleaned up, after which no watchdog is started.
_exiting = False


def _tell(message: dict) -> None:
    """Tell the watchdog of a change, starting it on the first. A watchdog that cannot be started
    leaves the cleanup to `close()`, the finaliser and the exit handler."""
    global _watchdog
    with _told:
        try:
            if _exiting:
                return
            if _watchdog is None or _watchdog.poll() is not None:
                _watchdog = subprocess.Popen(
                    [sys.executable, "-I", "-S", "-c", _WATCHDOG],
                    stdin=subprocess.PIPE,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    text=True,
                    start_new_session=True,
                )
            _watchdog.stdin.write(json.dumps(message) + "\n")
            _watchdog.stdin.flush()
        except OSError:
            pass


class ServeRefused(Refusal):
    """The `mosaica serve` process a database started did not report its three addresses.

    `Database.serve()` raises it, and so does every call that starts the server, such as the
    first `commit()`, when the process prints no line naming its viewer, session and control
    addresses within `MOSAICA_SERVE_TIMEOUT` seconds (30 by default), or prints one that leaves
    an address out. The process is stopped. On a timeout the message holds the last lines it
    wrote to its output and its error stream; otherwise it holds the line that left an address
    out. It is a kind of `Refusal`.
    """


@dataclass
class Listening:
    """The addresses a database's server listens on, each as `host:port`.

    - `viewer`: the viewer plane, where readers read.
    - `session`: the session plane, where the operator credential, or an API key holding
      `authorise-as`, makes and revokes tokens.
    - `control`: the control plane, where the operator writes.
    """

    viewer: str
    session: str
    control: str


def write_deployment(directory: Path) -> Path:
    """Write `mosaica.toml`, which `mosaica build` and `mosaica serve` both read.

    Every path in it resolves against this file's own directory, so the database directory serves
    from wherever it is copied to.

    `serve.cors_loopback` lets a page served from this machine read from the server, which is
    what a notebook's map needs, since its page's address is not known in advance. The server
    listens on this machine only. The catalogue of principals, credentials and grants is kept
    under `.mosaica/catalogue`.
    """
    serve = {
        "viewer": "127.0.0.1:0",
        "session": "127.0.0.1:0",
        "control": "127.0.0.1:0",
        "operator_credential_file": ".mosaica/operator.cred",
        "cors_loopback": True,
    }
    document = {
        "bundle": {
            "path": "bundle",
            "cache": ".mosaica/cache",
            "wal": ".mosaica/wal.log",
        },
        "build": {"schema": "schema.toml"},
        "disclosure": {"token_max_lifetime": TOKEN_MAX_LIFETIME},
        "serve": serve,
        "catalogue": {"dir": ".mosaica/catalogue"},
    }
    # The engine's cache directory is opened rather than created, so a deployment file naming one
    # that does not exist refuses to start with an IO error.
    (directory / ".mosaica" / "cache").mkdir(parents=True, exist_ok=True)
    path = directory / "mosaica.toml"
    path.write_text(dumps(document), encoding="utf-8")
    return path


def secrets_for(directory: Path) -> None:
    """Write the operator credential, once per database.

    It is written under `.mosaica/`, which is owner-only, and the file is created owner-only
    rather than created and then narrowed: between a write and a `chmod` the secret is readable by
    anyone on the machine.
    """
    private = directory / ".mosaica"
    private.mkdir(parents=True, exist_ok=True)
    private.chmod(0o700)
    _secret(private / "operator.cred", lambda: secrets.token_urlsafe(32))


def _secret(path: Path, mint) -> None:
    if path.exists():
        return
    value = mint()
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w") as file:
        file.write(value if value.endswith("\n") else value + "\n")


def start(
    binary: str,
    deployment: Path,
    timeout: float | None = None,
) -> tuple[subprocess.Popen, Listening]:
    """Start `mosaica serve` and read the addresses it bound."""
    child = subprocess.Popen(
        [binary, "serve", "--deployment", str(deployment)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
        cwd=str(Path(deployment).parent),
    )
    _running[child.pid] = child
    _tell({"serve": child.pid, "deployment": str(deployment)})
    threading.Thread(target=child.wait, daemon=True).start()
    errors = _drain(child.stderr)
    try:
        return child, read_announce(
            child.stdout, serve_timeout() if timeout is None else timeout, lambda: "".join(errors)
        )
    except ServeRefused:
        stop(child)
        raise


def read_announce(stdout, timeout: float, stderr_text) -> Listening:
    """Read the child's output until the line announcing its addresses.

    The line is JSON carrying `event: "listening"` and the three planes' bound addresses. Lines
    that are not that are skipped, the child's own logging having shared the stream before now.

    One reader holds the stream for the life of the child: it stops queueing lines once the
    announce line is found and keeps reading, so a child that logs after it has started never
    fills the pipe and blocks.
    """
    reader = _Reader(stdout)
    reader.start()
    deadline = time.monotonic() + timeout
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        try:
            line = reader.lines.get(timeout=min(remaining, 0.25))
        except queue.Empty:
            continue
        if line is None:
            break
        announce = _announce(line)
        if announce is not None:
            reader.queueing = False
            return announce
    raise ServeRefused(
        "mosaica serve announced no listening line within "
        f"{timeout:g}s. The SDK waits for one line of JSON on stdout carrying "
        '\'"event": "listening"\' and the viewer, session and control addresses. '
        "What the child wrote:\n"
        + "".join(f"  stdout: {line}" for line in reader.kept[-20:])
        + _tail(stderr_text())
    )


class _Reader(threading.Thread):
    """One thread per stream: it queues lines while `queueing`, and keeps the last of them."""

    def __init__(self, stream) -> None:
        super().__init__(daemon=True)
        self.stream = stream
        self.lines: queue.Queue = queue.Queue()
        self.kept: list[str] = []
        self.queueing = True

    def run(self) -> None:
        for line in self.stream:
            self.kept.append(line)
            del self.kept[:-KEPT_LINES]
            if self.queueing:
                self.lines.put(line)
        self.lines.put(None)


def _announce(line: str) -> Listening | None:
    line = line.strip()
    if not line.startswith("{"):
        return None
    try:
        document = json.loads(line)
    except ValueError:
        return None
    if not isinstance(document, dict) or document.get("event") != "listening":
        return None
    missing = [k for k in ("viewer", "session", "control") if not document.get(k)]
    if missing:
        raise ServeRefused(
            f"mosaica serve's listening line names no {', '.join(missing)} address: {line}"
        )
    return Listening(
        viewer=str(document["viewer"]),
        session=str(document["session"]),
        control=str(document["control"]),
    )


def _drain(stream) -> list[str]:
    """Keep reading a pipe the SDK does not parse, so the child never blocks writing to it."""
    reader = _Reader(stream)
    reader.queueing = False
    reader.start()
    return reader.kept


def _tail(text: str) -> str:
    if not text:
        return ""
    return "\n" + "".join(f"  stderr: {line}\n" for line in text.strip().splitlines()[-20:])


def stop(child: subprocess.Popen, timeout: float = 5.0) -> None:
    """Stop the child by its pid: `SIGTERM`, then `SIGKILL` if it is still there."""
    _running.pop(child.pid, None)
    _tell({"stopped": child.pid})
    # `Popen` signals only a child it has not reaped, so a pid handed on is never signalled.
    child.send_signal(signal.SIGTERM)
    try:
        child.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        child.kill()
        child.wait(timeout=timeout)


@atexit.register
def _stop_everything() -> None:
    global _exiting
    for child in list(_running.values()):
        stop(child)
    for directory in list(temporary):
        remove(directory)
    with _told:
        _exiting = True
        if _watchdog is not None:
            _watchdog.stdin.close()
            _watchdog.wait()


def find_binary() -> tuple[str, str]:
    """The `mosaica` binary and where it was found.

    `MOSAICA_BIN` when set, else the first `mosaica` on `PATH`, else the `mosaica-native`
    platform wheel, else a checkout's target directory, release before debug. An explicit
    override and a developer's own build both win over the wheel; the wheel is what makes a
    fresh `pip install mosaica` work.
    """
    named = os.environ.get("MOSAICA_BIN")
    if named:
        if not Path(named).exists():
            raise Refusal(f"MOSAICA_BIN names {named}, which does not exist")
        return named, "MOSAICA_BIN"
    from shutil import which

    found = which("mosaica")
    if found:
        return found, "PATH"
    try:
        from mosaica_native import binary_path
    except ImportError:
        pass
    else:
        return binary_path(), "the mosaica-native wheel"
    for parent in Path(__file__).resolve().parents:
        for profile in ("release", "debug"):
            candidate = parent / "target" / profile / "mosaica"
            if candidate.exists():
                return str(candidate), f"this checkout's target/{profile}"
    from . import __version__

    raise Refusal(
        "no `mosaica` binary at MOSAICA_BIN, on PATH, in the mosaica-native wheel, or in a "
        f"checkout's target directory; install the wheel this package was made with, "
        f"`pip install \"mosaica-native=={__version__}\"`, or build the binary with "
        "`cargo build --release -p mosaica` and name it in MOSAICA_BIN"
    )


def find_extension() -> ModuleType | None:
    """The `_mosaica` extension module, or `None` where nothing carries it.

    The same order as `find_binary`: whatever is already importable as `_mosaica`, then the
    `mosaica-native` wheel, then a checkout's target directory, where cargo names it
    `lib_mosaica.so`, which Python will not import by name, so it is loaded by path.
    """
    for name in ("_mosaica", "mosaica_native._mosaica"):
        try:
            return importlib.import_module(name)
        except ImportError:
            pass
    for parent in Path(__file__).resolve().parents:
        for profile in ("release", "debug"):
            for built in ("lib_mosaica.so", "lib_mosaica.dylib", "_mosaica.dll"):
                candidate = parent / "target" / profile / built
                if not candidate.exists():
                    continue
                loader = ExtensionFileLoader("_mosaica", str(candidate))
                spec = spec_from_loader("_mosaica", loader)
                module = module_from_spec(spec)
                sys.modules["_mosaica"] = module
                loader.exec_module(module)
                return module
    return None
