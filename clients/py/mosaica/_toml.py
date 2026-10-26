"""The writer of `schema.toml` and `mosaica.toml`, which turns a declaration into the TOML a
person would have written by hand.

The package writes the files and the `mosaica` binary reads them, so the binary checks how each
declaring method maps to a block and nothing here repeats that check. The output keeps the
blocks in the order they were declared, writes one table per block and spells every value as
the declaration spells it.

A plain `dict` becomes a sub-table (`[layer.members]`), a list of dicts becomes an array of
tables (`[[layer.levels]]`), and an `Inline` becomes a one-line table
(`hierarchy = { kind = "flat" }`). The two kinds of dict are told apart by type, because the
declaration uses both at the same depth: `hierarchy` is inline and `members` is a block.

A table's scalar keys are written before its sub-tables, because TOML binds a bare key to the
last table header above it, which would put a scalar written after a sub-table in the wrong
block.
"""

from __future__ import annotations

import datetime as _dt
from typing import Any


class Inline(dict):
    """A table that is written on one line, as in
    `artifact_visibility = { default = "inherited" }`."""


def dumps(document: dict[str, Any]) -> str:
    out: list[str] = []
    _table(out, [], document, root=True)
    return "".join(out)


def _table(
    out: list[str],
    path: list[str],
    table: dict[str, Any],
    root: bool = False,
    header: str | None = None,
) -> None:
    if header is not None:
        out.append(header)
    elif not root:
        out.append(f"[{_path(path)}]\n")
    wrote = False
    for key, value in table.items():
        if not _is_block(value):
            out.append(f"{_key(key)} = {value_to_toml(value)}\n")
            wrote = True
    if wrote or not root:
        out.append("\n")
    for key, value in table.items():
        if not _is_block(value):
            continue
        inner = [*path, key]
        if isinstance(value, dict):
            _table(out, inner, value)
        else:
            for element in value:
                _table(out, inner, element, header=f"[[{_path(inner)}]]\n")


def _is_block(value: Any) -> bool:
    if isinstance(value, Inline):
        return False
    if isinstance(value, dict):
        return True
    return isinstance(value, list) and bool(value) and all(isinstance(e, dict) for e in value)


def _path(path: list[str]) -> str:
    return ".".join(_key(part) for part in path)


_BARE = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-")


def _key(key: str) -> str:
    if key and set(key) <= _BARE:
        return key
    return _string(key)


def value_to_toml(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, str):
        return _string(value)
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        text = repr(value)
        return text if ("." in text or "e" in text or "n" in text) else text + ".0"
    if isinstance(value, _dt.datetime):
        return value.isoformat().replace("+00:00", "Z")
    if isinstance(value, Inline):
        body = ", ".join(f"{_key(k)} = {value_to_toml(v)}" for k, v in value.items())
        return "{ " + body + " }" if body else "{}"
    if isinstance(value, (list, tuple)):
        return "[" + ", ".join(value_to_toml(e) for e in value) + "]"
    if value is None:
        raise ValueError("a TOML document holds no null; leave the key out instead")
    raise TypeError(f"no TOML spelling for {type(value).__name__}")


_ESCAPES = {"\\": "\\\\", '"': '\\"', "\n": "\\n", "\r": "\\r", "\t": "\\t"}


def _string(text: str) -> str:
    body = "".join(_ESCAPES.get(c, c) for c in text)
    return f'"{body}"'
