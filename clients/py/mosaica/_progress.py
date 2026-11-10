"""How far a long-running job has gone, as the package draws it and hands it to a caller.

A job's record is the `Progress` object that `docs/openapi/mosaica.yaml` describes and that
`mosaica build --progress json` and `/control/status` serve, as a `dict`. Three sources give
records here: the lines of a first commit's build, the ingest record a later commit makes as it
sends its pages, and the compaction record `Control.compact(wait=True)` reads from the server.
Each passes every record to the sink that `progress=` names, which `shown` makes.
"""

from __future__ import annotations

import json
from typing import Any, Callable

from tqdm.auto import tqdm

from ._refusal import Refusal

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
    """The sink `progress=` names: `True` draws each record with `tqdm.auto`, `False` draws
    nothing, and a callable receives every record."""
    if progress is True:
        return _Bar()
    if progress is False:
        return Sink()
    if callable(progress):
        return _Callable(progress)
    raise Refusal(
        f"progress={progress!r} is not one of the values it takes. Write progress=True to show "
        "progress, progress=False to show none, or a function that takes each record as a dict"
    )


def _begins(last: dict | None, one: dict) -> bool:
    """Whether `one` is the first record of a step that `last` is not of. A producer gives each
    step a `step_id` of its own, kept while the step's count or context changes, and a job is
    one producer's."""
    if last is None:
        return True
    return (last.get("job"), last.get("step_id")) != (one.get("job"), one.get("step_id"))


class _Bar(Sink):
    """One `tqdm.auto` bar for the job, which is a widget in a notebook and a line redrawn in
    place in a terminal, reset to each step as it begins."""

    def __init__(self) -> None:
        self.bar = None
        self.last: dict | None = None

    def __call__(self, one: dict) -> None:
        total = one.get("total")
        phase = one.get("phase", {})
        context = f" {one['context']}" if one.get("context") else ""
        bytes_ = one.get("unit") == "bytes"
        shape = {
            "desc": f"[{phase.get('index')}/{phase.get('of')} {phase.get('name')}] "
            f"{one.get('step')}{context}",
            "total": total,
            "unit": "B" if bytes_ else f" {one.get('unit')}",
            "unit_scale": total is not None and (bytes_ or total >= 1000),
            "unit_divisor": 1024 if bytes_ else 1000,
            # A step that counts nothing shows its name and how long it has run.
            "bar_format": None if total is not None else "{desc} [{elapsed}{postfix}]",
        }
        ended = one.get("state", "running") != "running"
        postfix = "rss " + tqdm.format_sizeof(int(one.get("rss_bytes") or 0), "B", 1024)
        postfix += f", {one['state']}" if ended else ""
        if self.bar is None:
            self.bar = tqdm(dynamic_ncols=True, postfix=postfix, **shape)
        elif _begins(self.last, one):
            for name in ("unit", "unit_scale", "unit_divisor", "bar_format"):
                setattr(self.bar, name, shape[name])
            self.bar.set_description_str(shape["desc"], refresh=False)
            self.bar.reset(total=total)
        else:
            self.bar.set_description_str(shape["desc"], refresh=False)
        self.last = one
        self.bar.set_postfix_str(postfix, refresh=False)
        if total is not None:
            self.bar.update(max(int(one.get("done") or 0) - self.bar.n, 0))
        if ended or total is None:
            self.bar.refresh()

    def close(self) -> None:
        if self.bar is not None:
            self.bar.close()
            self.bar = None
