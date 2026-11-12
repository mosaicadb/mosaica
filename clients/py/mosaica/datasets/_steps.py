"""The three steps every dataset takes, fetch, prepare and build, each skipped when what it made is
still current.

A dataset's directory holds `raw/`, the publisher's files with `LOCK.json` saying where each came
from and its SHA-256; `prepared/`, the files the declaration names and the declaration itself as
`schema.toml`; and `db/`, a database `mosaica.open()` reads, whose `mosaica.toml` names that
declaration by absolute path. A dataset taken with `sample=N` prepares and builds into
`prepared-N/` and `db-N/` beside them, from the same `raw/`.

`prepared/` and `db/` each carry `.stamp.json`, the inputs they were made from. A step whose stamp
differs from what it would be made from now starts again from an empty directory, and a directory
without a stamp is never removed.
"""

from __future__ import annotations

import datetime
import hashlib
import http.client
import json
import os
import shutil
import urllib.error
import urllib.request
from pathlib import Path
from typing import Callable, Optional

from .._refusal import Refusal

STAMP = ".stamp.json"
LOCK = "LOCK.json"


def fingerprint(directory: Path) -> str:
    """A digest of the files under `directory` by name, size and modification time, leaving out
    names that start with a dot."""
    digest = hashlib.sha256()
    for path in sorted(directory.rglob("*")):
        relative = path.relative_to(directory)
        if any(part.startswith(".") for part in relative.parts) or not path.is_file():
            continue
        stat = path.stat()
        digest.update(f"{relative}\0{stat.st_size}\0{stat.st_mtime_ns}\n".encode())
    return digest.hexdigest()


def digest_of(paths: tuple[Path, ...]) -> str:
    """A digest of the bytes of `paths`, the files that make up a recipe."""
    digest = hashlib.sha256()
    for path in paths:
        digest.update(path.read_bytes())
    return digest.hexdigest()


def locked(raw: Path) -> dict:
    """`LOCK.json` in `raw`: each fetched file's URL, size, SHA-256 and date, by name."""
    path = raw / LOCK
    return json.loads(path.read_text(encoding="utf-8")) if path.exists() else {}


def record(raw: Path, entries: dict) -> None:
    """Add `entries`, each `{url, bytes, sha256}` by name, to `LOCK.json`, dated today."""
    if not entries:
        return
    lock = locked(raw)
    today = datetime.date.today().isoformat()
    lock.update({name: {**entry, "fetched": today} for name, entry in entries.items()})
    (raw / LOCK).write_text(json.dumps(lock, indent=1) + "\n", encoding="utf-8")


def held(raw: Path, lock: dict, name: str, url: str, sha256: Optional[str] = None) -> bool:
    """Whether `LOCK.json` records `name` as fetched from `url`, with the SHA-256 `sha256` where
    one is given, and the file is there at its size."""
    entry = lock.get(name)
    target = raw / name
    return (
        entry is not None
        and entry.get("url") == url
        and (sha256 is None or entry.get("sha256") == sha256)
        and target.exists()
        and target.stat().st_size == entry.get("bytes")
    )


def kept(
    target: Path, write: Callable[[Path], None], url: str, sha256: Optional[str] = None
) -> dict:
    """Write `target` with `write`, through a `.part` file so that a write that stops, or whose
    bytes do not have the SHA-256 `sha256` where one is given, leaves nothing at `target`, and
    return its lock entry."""
    part = target.with_name(target.name + ".part")
    try:
        write(part)
    except BaseException:
        part.unlink(missing_ok=True)
        raise
    digest = hashlib.sha256()
    with part.open("rb") as written:
        while chunk := written.read(1 << 20):
            digest.update(chunk)
    if sha256 is not None and digest.hexdigest() != sha256:
        part.unlink()
        raise Refusal(
            f"fetch: {url} does not have the SHA-256 this package expects ({sha256}); run again "
            "to fetch it again, and if it differs again the published file has changed"
        )
    size = part.stat().st_size
    os.replace(part, target)
    return {"url": url, "bytes": size, "sha256": digest.hexdigest()}


def fetch(
    raw: Path,
    files: dict[str, str],
    progress: bool = True,
    sha256: Optional[dict[str, str]] = None,
) -> list[Path]:
    """Download each of `files`, `{name: url}`, into `raw` unless `LOCK.json` already holds it,
    record each one fetched, and return their paths. `sha256`, `{name: digest}`, pins a file's
    bytes: one the lock records with another digest is fetched again, and a download with
    another digest is refused."""
    raw.mkdir(parents=True, exist_ok=True)
    pinned = sha256 or {}
    for name, url in files.items():
        if not held(raw, locked(raw), name, url, pinned.get(name)):
            (raw / name).parent.mkdir(parents=True, exist_ok=True)
            entry = kept(
                raw / name,
                lambda part: _download(url, part, progress),
                url,
                pinned.get(name),
            )
            record(raw, {name: entry})
    return [raw / name for name in files]


def width(keys: int) -> str:
    """The narrowest vocabulary width with room for `keys` codes after the absent code 0."""
    return "u8" if keys < 255 else ("u16" if keys < 65_535 else "u32")


def declared(declaration: Path, placeholder: str, text: str, path: Path) -> None:
    """`declaration` written to `path` with its one line reading `placeholder` replaced by
    `text`."""
    lines = declaration.read_text(encoding="utf-8").splitlines()
    hit = [i for i, line in enumerate(lines) if line.strip() == placeholder]
    assert len(hit) == 1, f"{placeholder} appears {len(hit)} times as a line of its own"
    lines[hit[0]] = text
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def _download(url: str, part: Path, progress: bool) -> None:
    from tqdm.auto import tqdm

    from .. import __version__

    request = urllib.request.Request(url, headers={"User-Agent": f"mosaica/{__version__}"})
    try:
        with urllib.request.urlopen(request) as response, part.open("wb") as out:
            total = int(response.headers.get("Content-Length") or 0) or None
            with tqdm(
                total=total, unit="B", unit_scale=True, desc=part.stem, disable=not progress
            ) as bar:
                while chunk := response.read(1 << 20):
                    out.write(chunk)
                    bar.update(len(chunk))
            got = out.tell()
    except (urllib.error.URLError, http.client.HTTPException, OSError) as why:
        raise Refusal(
            f"fetch: {url} could not be downloaded ({getattr(why, 'reason', why)}); run again "
            "to retry it"
        ) from None
    if total is not None and got != total:
        raise Refusal(
            f"fetch: {url} stopped after {got:,} of {total:,} bytes; run again to retry it"
        )


def made(directory: Path, inputs: dict, make: Callable[[], None], doing: str) -> None:
    """Run `make` into an empty `directory` unless its stamp records `inputs`, and stamp it.

    `doing` names the step in what is printed, as in "preparing geonames". The stamp is written
    before `make` starts, so a directory left by a step that stopped is still this package's to
    remove; one without a stamp is refused.
    """
    stamp = directory / STAMP
    if stamp.exists() and json.loads(stamp.read_text(encoding="utf-8")) == inputs:
        return
    if directory.exists() and any(directory.iterdir()):
        if not stamp.exists():
            raise Refusal(
                f"{directory} holds files mosaica.datasets did not make, so it is left alone; "
                "move them, or keep the dataset in another directory"
            )
        print(f"mosaica: {doing} again in {directory}, since its inputs changed")
        # The stamp stays until the rest is gone, so a step stopped here leaves a directory that
        # is still this package's to clear.
        for child in directory.iterdir():
            if child.name == STAMP:
                continue
            if child.is_dir() and not child.is_symlink():
                shutil.rmtree(child)
            else:
                child.unlink()
    else:
        print(f"mosaica: {doing} in {directory}")
    directory.mkdir(parents=True, exist_ok=True)
    stamp.write_text(json.dumps({"incomplete": True}) + "\n", encoding="utf-8")
    make()
    stamp.write_text(json.dumps(inputs, indent=1) + "\n", encoding="utf-8")
