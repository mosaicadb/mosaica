"""How far a long-running job has gone, as the package shows it and hands it to a caller.

A job's record is the `Progress` object that `docs/openapi/mosaica.yaml` describes and that
`mosaica build --progress json` and `/control/status` serve, as a `dict`. Three sources give
records here: the lines of a first commit's build, the ingest record a later commit makes as it
sends its pages, and the compaction record `Control.compact(wait=True)` reads from
`/control/status`. Each passes every record to the sink that `progress=` names, through `shown`.

`Tracker` makes the ingest record. Its `rate` is measured over the last 20 seconds of the step,
and a step younger than two seconds has none, as the server's and the build's records do.
"""

from __future__ import annotations

import collections
import json
import sys
import time
from typing import Any, Callable

from ._refusal import Refusal

#: The span a `Tracker`'s rate is measured over, in seconds.
RATE_WINDOW_S = 20.0

#: A step younger than this has no rate, because its first pages measure start-up rather than
#: throughput.
RATE_SETTLE_S = 2.0

#: How long the line renderer goes without a line while a step runs, in seconds.
LINE_EVERY_S = 30.0


def record(line: str) -> dict | None:
    """The progress record a line of `mosaica build --progress json` carries, or `None` for a
    line that is not one: a record is a JSON object with a `job` key."""
    text = line.strip()
    if not text.startswith("{"):
        return None
    try:
        value = json.loads(text)
    except ValueError:
        return None
    return value if isinstance(value, dict) and "job" in value else None


# ---------------------------------------------------------------------------- the sinks


class Sink:
    """Where records go: called with each record, and closed when the job's records end."""

    def __call__(self, one: dict) -> None:
        return None

    def close(self) -> None:
        return None

    def __enter__(self) -> "Sink":
        return self

    def __exit__(self, *exception) -> None:
        self.close()


class _Callable(Sink):
    def __init__(self, receive: Callable[[dict], Any]) -> None:
        self.receive = receive

    def __call__(self, one: dict) -> None:
        self.receive(one)


def shown(progress: Any) -> Sink:
    """The sink `progress=` names: `True` draws each record with `tqdm` where it can be
    imported and as lines on stderr where it cannot, `False` shows nothing, and a callable
    receives every record."""
    if progress is True:
        try:
            from tqdm.auto import tqdm
        except ImportError:
            return _Lines()
        return _Bar(tqdm)
    if progress is False:
        return Sink()
    if callable(progress):
        return _Callable(progress)
    raise Refusal(
        f"progress={progress!r} is not one of the values it takes. Write progress=True to show "
        "progress, progress=False to show none, or a function that takes each record as a dict"
    )


def _step(one: dict) -> tuple:
    """The phase, name and context that tell one step from another."""
    return (one.get("phase", {}).get("index"), one.get("step"), one.get("context"))


class _Lines(Sink):
    """One line on stderr when a step begins, the last record of a step when the next begins, a
    line at most every 30 seconds between, and the job's last record, as `mosaica build
    --progress plain` prints them."""

    def __init__(self) -> None:
        self.shown: tuple | None = None
        self.printed = 0.0
        #: The last record given and not printed.
        self.held: dict | None = None

    def __call__(self, one: dict) -> None:
        if one.get("state", "running") != "running":
            self.held = None
            self._print(one)
            return
        if _step(one) != self.shown:
            if self.held is not None:
                self._print(self.held)
            self.held = None
            self._print(one)
        elif time.monotonic() - self.printed >= LINE_EVERY_S:
            self.held = None
            self._print(one)
        else:
            self.held = one

    def _print(self, one: dict) -> None:
        phase = one.get("phase", {})
        print(
            f"{_clock(time.time())} [{phase.get('index')}/{phase.get('of')} "
            f"{phase.get('name')}] {line(one)}",
            file=sys.stderr,
            flush=True,
        )
        self.shown = _step(one)
        self.printed = time.monotonic()


class _Bar(Sink):
    """One `tqdm` bar for the job, reset to each step as it begins, which `tqdm.auto` draws as a
    widget in a notebook and as a line redrawn in place in a terminal."""

    def __init__(self, tqdm) -> None:
        self.tqdm = tqdm
        self.bar = None
        self.shown: tuple | None = None

    def __call__(self, one: dict) -> None:
        counted = one.get("total") is not None
        if self.bar is None:
            self.bar = self.tqdm(dynamic_ncols=True)
        if _step(one) != self.shown:
            self.shown = _step(one)
            phase = one.get("phase", {})
            context = f" {one['context']}" if one.get("context") else ""
            bytes_ = one.get("unit") == "bytes"
            self.bar.unit = "B" if bytes_ else f" {one.get('unit')}" if counted else ""
            self.bar.unit_scale = counted
            self.bar.unit_divisor = 1024 if bytes_ else 1000
            # A step that counts nothing shows its name and how long it has run.
            self.bar.bar_format = None if counted else "{desc} [{elapsed}{postfix}]"
            self.bar.set_description_str(
                f"[{phase.get('index')}/{phase.get('of')} {phase.get('name')}] "
                f"{one.get('step')}{context}",
                refresh=False,
            )
            self.bar.reset(total=one.get("total"))
        ended = one.get("state", "running") != "running"
        postfix = f"rss {_bytes(int(one.get('rss_bytes') or 0))}"
        self.bar.set_postfix_str(postfix + (f", {one['state']}" if ended else ""), refresh=False)
        if counted:
            self.bar.update(max(int(one.get("done") or 0) - self.bar.n, 0))
        if ended or not counted:
            self.bar.refresh()

    def close(self) -> None:
        if self.bar is not None:
            self.bar.close()
            self.bar = None


# ---------------------------------------------------------------------------- the ingest record


class Tracker:
    """The record of a job this process runs: its phases, the step open and the step's count.

    `snapshot()` returns the record as the server serves one, with `rss_bytes` this process's
    resident memory.
    """

    def __init__(self, job: str, phases: list[str]) -> None:
        self.job = job
        self.phases = phases
        self.started = time.monotonic()
        self.state = "running"
        self.begin(0, "")

    def begin(self, at: int, step: str, total: int | None = None, unit: str | None = None) -> None:
        """Begin `step` in the phase at `at`, counted from 0, counting `total` of `unit` where it
        counts anything."""
        self.phase = at
        self.step = step
        self.context: str | None = None
        self.counted = None if total is None else (int(total), unit)
        self.done = 0
        self.step_started = time.monotonic()
        self.samples = collections.deque([(self.step_started, 0)])

    def advance(self, n: int) -> None:
        self.done += int(n)

    def end(self, state: str) -> None:
        """Mark the job `finished` or `failed`, leaving its step as it was."""
        self.state = state

    def snapshot(self) -> dict:
        now = time.monotonic()
        phase = {"index": self.phase + 1, "of": len(self.phases), "name": self.phases[self.phase]}
        out: dict = {"job": self.job, "state": self.state, "phase": phase, "step": self.step}
        if self.context is not None:
            out["context"] = self.context
        if self.counted is not None:
            total, unit = self.counted
            out.update(done=self.done, total=total, unit=unit)
            rate = self._rate(now)
            if rate is not None:
                out["rate"] = rate
                if rate > 0:
                    out["eta_s"] = max(total - self.done, 0) / rate
        out.update(
            elapsed_s=now - self.started,
            step_elapsed_s=now - self.step_started,
            rss_bytes=resident_bytes(),
        )
        return out

    def _rate(self, now: float) -> float | None:
        """The rate from the oldest sample within the window to now, recording now as a sample.
        The step's start is the first sample, so a reader that asks rarely gets the step's
        average."""
        while len(self.samples) > 1 and now - self.samples[1][0] >= RATE_WINDOW_S:
            self.samples.popleft()
        since, start = self.samples[0]
        self.samples.append((now, self.done))
        if now - self.step_started < RATE_SETTLE_S or now <= since:
            return None
        return max(self.done - start, 0) / (now - since)


def page_context(index: int, of: int) -> str | None:
    """The context of the page at `index`, counted from 0, of `of`, as `page (37 of 120)`, and
    none where there is one page, as the build's records name their parts."""
    return f"page ({index + 1} of {of})" if of > 1 else None


def resident_bytes() -> int:
    """This process's resident memory, read from `/proc/self/status`, or its peak resident memory
    where the platform has no `/proc`, or 0 where it reports neither."""
    try:
        with open("/proc/self/status", encoding="ascii", errors="replace") as status:
            for text in status:
                if text.startswith("VmRSS:"):
                    return int(text.split()[1]) * 1024
    except (OSError, ValueError, IndexError):
        pass
    try:
        import resource
    except ImportError:
        return 0
    peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    # macOS reports bytes and the other platforms with `getrusage` report KiB.
    return int(peak) if sys.platform == "darwin" else int(peak) * 1024


# ---------------------------------------------------------------------------- one line


def line(one: dict) -> str:
    """The step, its count, rate, time remaining, the job's elapsed time and resident memory, and
    the job's state once it has ended, as `mosaica build --progress plain` writes them."""
    out = str(one.get("step", ""))
    if one.get("context"):
        out += f" {one['context']}"
    done, total, unit = one.get("done"), one.get("total"), one.get("unit")
    if done is not None and total is not None and unit is not None:
        name = "" if unit == "bytes" else f" {unit}"
        out += f"  {_amount(done, unit)}/{_amount(total, unit)}{name}"
        if one.get("rate") is not None:
            out += f"  {_amount(one['rate'], unit)}/s"
        if one.get("eta_s") is not None:
            out += f"  ~{_duration(one['eta_s'])}"
    out += f"  {_duration(one.get('elapsed_s', 0.0))}  rss {_bytes(int(one.get('rss_bytes') or 0))}"
    if one.get("state", "running") != "running":
        out += f"  {one['state']}"
    return out


def _amount(value: float, unit: str) -> str:
    return _bytes(int(value)) if unit == "bytes" else _count(float(value))


def _count(value: float) -> str:
    """A count to three significant figures with a decimal suffix, such as `41.2M`."""
    suffixes = ["", "k", "M", "G", "T", "P"]
    at = 0
    while value >= 999.5 and at + 1 < len(suffixes):
        value /= 1000.0
        at += 1
    if at == 0 and value == int(value):
        decimals = 0
    elif value >= 99.95:
        decimals = 0
    elif value >= 9.995:
        decimals = 1
    else:
        decimals = 2
    return f"{value:.{decimals}f}{suffixes[at]}"


def _bytes(value: int) -> str:
    """Bytes in binary units, such as `18.2 GiB`."""
    units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"]
    amount = float(value)
    at = 0
    while amount >= 1023.95 and at + 1 < len(units):
        amount /= 1024.0
        at += 1
    decimals = 0 if at == 0 or amount >= 99.95 else 1 if amount >= 9.995 else 2
    return f"{amount:.{decimals}f} {units[at]}"


def _duration(seconds: float) -> str:
    """Seconds as `18s`, `7m12s` or `3h04m`."""
    s = int(round(max(float(seconds), 0.0)))
    if s < 60:
        return f"{s}s"
    if s < 3600:
        return f"{s // 60}m{s % 60:02d}s"
    return f"{s // 3600}h{s // 60 % 60:02d}m"


def _clock(now: float) -> str:
    """The time of day in UTC, as `12:04:31Z`."""
    s = int(now) % 86_400
    return f"{s // 3600:02d}:{s // 60 % 60:02d}:{s % 60:02d}Z"
