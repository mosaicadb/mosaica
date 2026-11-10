"""Two `mosaica` binaries on one generated corpus: build, serve, query, ingest, delete, compact.

    python3 scripts/bench_ab.py run --baseline A/mosaica --candidate B/mosaica \\
        --profile fields --work /tmp/ab --out ab-fields.json
    python3 scripts/bench_ab.py report ab-*.json [--summary summary.md] [--fail-on-regression]
    python3 scripts/bench_ab.py corpus --binary target/release/mosaica --profile scatter \\
        --items 10000000 --out /tmp/corpora/scatter-10m [--cap 2G]

`corpus` writes one profile's corpus and builds it into a bundle that is kept, for benchmarks that
serve it many times (`scripts/bench_capacity.py`). `--cap` builds under a memory cap, and
`corpus.json` records the build's time, its peak memory and the bundle's bytes.

`run` writes one profile's corpus from a seed, then runs every step against each binary in turn:
baseline then candidate on even repetitions, candidate then baseline on odd ones, so a machine
that drifts during the run drifts under both. Every step talks to the binary through the CLI and
the HTTP API only, so the two binaries may come from commits whose internals and bundle formats
differ. Each side builds its own bundle.

Each step is checked before its time is kept: the visible count after the build, the ingest, the
changes and the compaction must be the one the step should leave. A step that did less would
otherwise report as faster.

A figure is the minimum over repetitions. A row of the report is marked slower or faster when
the ratio of minima passes `--threshold`, the two sides' ranges do not overlap, and for a timing
the minima differ by more than `--floor`, the jitter of a request over loopback. A `cold` figure
is one answer per run, which varied by more than twice between runs of one binary, so it is shown
without a verdict.

`run` exits non-zero when a candidate step fails: a refused build, a server that does not come up,
a request answered with an error, a count that is wrong. A baseline failure is recorded and
reported, since the baseline may predate an API the driver uses. `report --fail-on-regression`
exits non-zero when a row is marked slower.
"""

from __future__ import annotations

import argparse
import functools
import json
import os
import platform
import shutil
import signal
import socket
import statistics
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq
import requests

OPERATOR_CREDENTIAL = "bench-ab-operator-credential-0123456789abcdef"
VIEW = "map"
TERMS_NARROW = ["t0"]
INGEST_BATCH_ROWS = 10_000
CHANGES_PER_REQUEST = 10_000

# ---------------------------------------------------------------------------------------------
# Profiles
# ---------------------------------------------------------------------------------------------

NUMERIC_TYPES = ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "f32", "f64"]
MODES = {
    "render": (True, False),
    "index": (False, True),
    "both": (True, True),
    "stored": (False, False),
}
ARROW_TYPES = {
    "u8": pa.uint8(),
    "u16": pa.uint16(),
    "u32": pa.uint32(),
    "u64": pa.uint64(),
    "i8": pa.int8(),
    "i16": pa.int16(),
    "i32": pa.int32(),
    "i64": pa.int64(),
    "f32": pa.float32(),
    "f64": pa.float64(),
}
RANGES = {
    "u8": (0, 255),
    "u16": (0, 65_535),
    "u32": (0, 4_000_000_000),
    "u64": (0, 2**63),
    "i8": (-128, 127),
    "i16": (-32_768, 32_767),
    "i32": (-2_000_000_000, 2_000_000_000),
    "i64": (-(2**62), 2**62),
}
#: 2000-01-01 and 2026-01-01, in microseconds.
TIME_RANGE = (946_684_800_000_000, 1_767_225_600_000_000)


@dataclass
class Column:
    """One declared attribute."""

    name: str
    type: str
    render: bool = False
    index: bool = False
    vocabulary: str | None = None


@dataclass
class Profile:
    name: str
    items: int
    about: str
    projection: str = "none"
    columns: list[Column] = field(default_factory=list)
    vocabularies: dict[str, str] = field(default_factory=dict)  # name -> visibility
    terms: int = 16
    contiguous_labels: bool = False
    layers: bool = False
    text_words: int = 6


def fields_profile() -> Profile:
    columns = [
        Column(f"{t}_{m}", t, *MODES[m])
        for t in NUMERIC_TYPES + ["bool", "timestamp_us"]
        for m in MODES
    ]
    columns += [Column(f"cat_{m}", "category", *MODES[m], vocabulary="cat") for m in MODES]
    columns += [Column(f"catd_{m}", "category", *MODES[m], vocabulary="catd") for m in MODES]
    columns += [
        Column(f"{t}_{m}", t, False, m == "index")
        for t in ("keyword", "text")
        for m in ("index", "stored")
    ]
    return Profile(
        "fields",
        5_000_000,
        "every attribute type, each rendered, indexed, both and neither; labels hashed over 16 terms",
        columns=columns,
        vocabularies={"cat": "public", "catd": "derived"},
    )


def small_columns() -> list[Column]:
    return [
        Column("kind", "category", True, True, vocabulary="kind"),
        Column("score", "f32", True, False),
        Column("year", "u16", False, True),
    ]


PROFILES = {
    "fields": fields_profile,
    "scatter": lambda: Profile(
        "scatter",
        100_000_000,
        "few fields; each item's label one of 1,024 terms by hash, so every term is scattered",
        columns=small_columns(),
        vocabularies={"kind": "public"},
        terms=1024,
    ),
    "clustered": lambda: Profile(
        "clustered",
        100_000_000,
        "few fields on a Web Mercator view; each item's label the term of its spatial cluster",
        projection="web_mercator",
        columns=small_columns(),
        vocabularies={"kind": "derived"},
        terms=64,
        contiguous_labels=True,
    ),
    "text": lambda: Profile(
        "text",
        10_000_000,
        "an indexed text column of about 24 words from a Zipf vocabulary, and an indexed keyword",
        columns=[Column("body", "text", index=True), Column("tag", "keyword", index=True)]
        + small_columns(),
        vocabularies={"kind": "public"},
        text_words=24,
    ),
    "layers": lambda: Profile(
        "layers",
        50_000_000,
        "a three-level tiered layer of spatial clusters, and a flat layer over a category",
        columns=small_columns(),
        vocabularies={"kind": "public"},
        layers=True,
    ),
}

BLOBS = 64
LEVELS = (16, 256, 4096)
WORDS = 50_000
#: Rows generated and written at a time, which bounds the generator's memory.
CHUNK = 1_000_000


@functools.cache
def layout(seed: int) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """The blobs every batch of one seed draws its points from: centres, spreads and weights."""
    rng = np.random.default_rng([seed])
    weight = rng.pareto(1.2, size=BLOBS) + 0.1
    return (
        rng.uniform(50, 950, size=(BLOBS, 2)),
        rng.uniform(5, 60, size=BLOBS),
        weight / weight.sum(),
    )


def positions(
    rng: np.random.Generator, n: int, seed: int
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """`n` points in [0, 1000)², nine in ten from the seed's 64 Gaussian blobs and one in ten
    uniform, and the blob each came from, the uniform ones taking blob `BLOBS`."""
    centres, spread, weight = layout(seed)
    blob = rng.choice(BLOBS, size=n, p=weight)
    x = centres[blob, 0] + rng.normal(size=n) * spread[blob]
    y = centres[blob, 1] + rng.normal(size=n) * spread[blob]
    uniform = rng.random(n) < 0.1
    x[uniform] = rng.uniform(0, 1000, size=uniform.sum())
    y[uniform] = rng.uniform(0, 1000, size=uniform.sum())
    blob[uniform] = BLOBS
    return np.clip(x, 0, 999.999), np.clip(y, 0, 999.999), blob


def labelled(prefix: str, numbers: np.ndarray, *more) -> pa.Array:
    """`prefix` followed by each number, and by `more`'s separators and numbers in turn."""
    parts = [pa.scalar(prefix), pc.cast(pa.array(numbers), pa.string())]
    for i in range(0, len(more), 2):
        parts += [pa.scalar(more[i]), pc.cast(pa.array(more[i + 1]), pa.string())]
    return pc.binary_join_element_wise(*parts, "")


def values(rng: np.random.Generator, column: Column, n: int, text_words: int) -> pa.Array:
    t = column.type
    if t in RANGES:
        lo, hi = RANGES[t]
        dtype = np.uint64 if t == "u64" else np.int64
        return pa.array(rng.integers(lo, hi, size=n, endpoint=True, dtype=dtype), ARROW_TYPES[t])
    if t in ("f32", "f64"):
        return pa.array(rng.normal(100.0, 30.0, size=n), ARROW_TYPES[t])
    if t == "bool":
        return pa.array(rng.random(n) < 0.3)
    if t == "timestamp_us":
        return pa.array(rng.integers(*TIME_RANGE, size=n), pa.timestamp("us", tz="UTC"))
    if t == "category":
        size = 2000 if column.vocabulary == "catd" else 40
        return labelled(column.vocabulary, np.minimum(rng.zipf(1.3, size=n), size) - 1)
    if t == "keyword":
        return labelled("k", rng.integers(0, 2**40, size=n))
    if t == "text":
        ranks = np.minimum(rng.zipf(1.15, size=(text_words, n)), WORDS) - 1
        return pc.binary_join_element_wise(*(labelled("w", r) for r in ranks), " ")
    raise ValueError(f"no generator for type {t}")


def rows(profile: Profile, first_id: int, n: int, seed: int) -> pa.Table:
    """Items `first_id .. first_id + n`. A batch is a function of its seed and range alone, so
    the build's rows and each ingest batch are the same for both binaries."""
    rng = np.random.default_rng([seed, first_id])
    x, y, blob = positions(rng, n, seed)
    ids = np.arange(first_id, first_id + n, dtype=np.int64)
    if profile.contiguous_labels:
        term = blob % profile.terms
    else:
        mixed = (ids.astype(np.uint64) * np.uint64(0x9E3779B97F4A7C15)) >> np.uint64(40)
        term = mixed % np.uint64(profile.terms)
    data: dict[str, pa.Array] = {"id": pa.array(ids)}
    if profile.projection == "none":
        data["x"], data["y"] = pa.array(x), pa.array(y)
    else:
        data["lon"], data["lat"] = pa.array(x * 0.36 - 180.0), pa.array(y * 0.17 - 85.0)
    data["access"] = labelled("t", term)
    for column in profile.columns:
        data[column.name] = values(rng, column, n, profile.text_words)
    return pa.table(data)


def layer_keys(x: np.ndarray, y: np.ndarray) -> pa.Array:
    """Each item's key at each of `LEVELS`, by the cell of a square grid it falls in."""
    keys = []
    for cells in LEVELS:
        side = int(np.sqrt(cells))
        cx = np.minimum((x / 1000 * side).astype(np.int64), side - 1)
        cy = np.minimum((y / 1000 * side).astype(np.int64), side - 1)
        keys.append(labelled(f"g{side}-", cx, "-", cy))
    n = len(x)
    order = np.arange(len(LEVELS) * n).reshape(len(LEVELS), n).T.ravel()
    flat = pa.concat_arrays(keys).take(pa.array(order))
    return pa.ListArray.from_arrays(
        pa.array(np.arange(0, len(flat) + 1, len(LEVELS), dtype=np.int32)), flat
    )


def schema_toml(profile: Profile) -> str:
    sources = ['points = "points.parquet"']
    if profile.layers:
        sources.append('members = "members.parquet"')
    out = ["[sources]", *sources, "", "[defaults]", 'source = "points"', ""]
    extent = (
        "extent = { min = 0.0, max = 1000.0 }"
        if profile.projection == "none"
        else "extent = { lon = [-180.0, 180.0], lat = [-85.0, 85.0] }"
    )
    out += [
        "[[view]]",
        f'name = "{VIEW}"',
        f'projection = "{profile.projection}"',
        extent,
        'point_visibility = { field = "access" }',
        "",
    ]
    for name, visibility in profile.vocabularies.items():
        width = "u16" if name == "catd" else "u8"
        out += [
            "[[vocabulary]]",
            f'name = "{name}"',
            f'width = "{width}"',
            'value_set = "open"',
            f'visibility = "{visibility}"',
            "",
        ]
    out += ["[[attribute]]", 'name = "id"', 'type = "i64"', "unique = true", ""]
    for column in profile.columns:
        out += ["[[attribute]]", f'name = "{column.name}"', f'type = "{column.type}"']
        if column.vocabulary:
            out.append(f'vocabulary = "{column.vocabulary}"')
        if column.render:
            out.append("render = true")
        if column.index:
            out.append("index = true")
        out.append("")
    if profile.layers:
        out += [
            "[[layer]]",
            'name = "grid"',
            f'views = ["{VIEW}"]',
            'membership = "enumerated"',
            'value_set = "open"',
            'hierarchy = { kind = "tiered", prune_children = true }',
            'visibility = "public"',
            'artifact_visibility = { default = "inherited" }',
            "require_member_visibility = { count = 1 }",
            "",
        ]
        for level, zoom in enumerate(([0, 3], [3, 6], [6, 16])):
            out += ["[[layer.levels]]", f"level = {level}", f"zoom = {zoom}", ""]
        out += ["[layer.content]", 'computed = ["centroid", "box"]', ""]
        out += ["[layer.members]", 'source = "members"', 'fields = { id = "entity" }', ""]
        out += [
            "[[layer]]",
            'name = "kinds"',
            f'views = ["{VIEW}"]',
            'membership = { attribute = "kind" }',
            'hierarchy = { kind = "flat" }',
            'visibility = "public"',
            'artifact_visibility = { default = "inherited" }',
            'require_member_visibility = "any"',
            "",
        ]
    return "\n".join(out)


def write_corpus(profile: Profile, items: int, seed: int, out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    points = members = None
    try:
        for first in range(0, items, CHUNK):
            table = rows(profile, first, min(CHUNK, items - first), seed)
            if points is None:
                points = pq.ParquetWriter(out / "points.parquet", table.schema, compression="zstd")
            points.write_table(table)
            if profile.layers:
                x = table.column("x").to_numpy()
                y = table.column("y").to_numpy()
                part = pa.table({"entity": table.column("id"), "key": layer_keys(x, y)})
                if members is None:
                    members = pq.ParquetWriter(
                        out / "members.parquet", part.schema, compression="zstd"
                    )
                members.write_table(part)
    finally:
        for writer in (points, members):
            if writer is not None:
                writer.close()
    (out / "schema.toml").write_text(schema_toml(profile))


# ---------------------------------------------------------------------------------------------
# Queries
# ---------------------------------------------------------------------------------------------


def filter_for(column: Column) -> dict | None:
    """A filter on `column` that matches part of the corpus, in an operator of the column's
    family, or None where the column cannot be filtered."""
    if not (column.render or column.index):
        return None
    t = column.type
    if t in RANGES:
        lo, hi = RANGES[t]
        return {column.name: {"range": {"gte": str(lo + (hi - lo) // 2)}}}
    if t in ("f32", "f64"):
        return {column.name: {"range": {"gte": 110.0}}}
    if t == "bool":
        return {column.name: {"eq": True}}
    if t == "timestamp_us":
        return {column.name: {"range": {"gte": "2020-01-01"}}}
    if t == "category":
        return {column.name: {"in": [f"{column.vocabulary}{k}" for k in range(3)]}}
    if t == "keyword":
        return {column.name: {"prefix": "k12"}}
    if t == "text":
        return {column.name: {"match": "w3"}}
    return None


def battery(profile: Profile) -> dict[str, tuple[str, dict]]:
    """name -> (route, body). Every request is made by both principals."""
    whole = (
        [0.0, 0.0, 1000.0, 1000.0] if profile.projection == "none" else [-180.0, -85.0, 180.0, 85.0]
    )
    x0, y0, x1, y1 = whole
    corner = [x0, y0, x0 + (x1 - x0) / 8, y0 + (y1 - y0) / 8]
    queries = {
        "viewport z0 counts": ("/v1/viewport", {"view": VIEW, "zoom": 0, "bbox": whole, "k": 0}),
        "viewport z2 k200": ("/v1/viewport", {"view": VIEW, "zoom": 2, "bbox": whole, "k": 200}),
        "viewport z6 k200": ("/v1/viewport", {"view": VIEW, "zoom": 6, "bbox": corner, "k": 200}),
        "aggregate count": ("/v1/aggregate", {"view": VIEW, "groupings": [{"count": True}]}),
        "aggregate cells d6": (
            "/v1/aggregate",
            {"view": VIEW, "groupings": [{"count": True, "cells": {"depth": 6}}]},
        ),
        "items 1 page": (
            "/v1/items",
            {"view": VIEW, "fields": [], "page_rows": 10_000, "pages": 1},
        ),
    }
    for column in profile.columns:
        leaf = filter_for(column)
        if leaf is not None:
            queries[f"viewport z2 filter {column.name}"] = (
                "/v1/viewport",
                {"view": VIEW, "zoom": 2, "bbox": whole, "k": 200, "filters": leaf},
            )
        if column.type == "category" and (column.render or column.index):
            queries[f"aggregate top {column.name}"] = (
                "/v1/aggregate",
                {
                    "view": VIEW,
                    "groupings": [{"by": {"field": column.name, "top": 20}, "count": True}],
                },
            )
    if profile.layers:
        queries["viewport z4 layers"] = (
            "/v1/viewport",
            {"view": VIEW, "zoom": 4, "bbox": whole, "k": 200, "layers": ["grid", "kinds"]},
        )
        queries["artifacts viewport z4"] = (
            "/v1/artifacts/viewport",
            {"view": VIEW, "zoom": 4, "bbox": whole, "layers": ["grid"], "per_tile": 32},
        )
        queries["aggregate by layer"] = (
            "/v1/aggregate",
            {
                "view": VIEW,
                "groupings": [{"by": {"layer": "grid", "level": 1, "top": 50}, "count": True}],
            },
        )
    return queries


# ---------------------------------------------------------------------------------------------
# One side's run
# ---------------------------------------------------------------------------------------------


class StepFailed(RuntimeError):
    pass


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def du(path: Path) -> int:
    return sum(p.stat().st_size for p in path.rglob("*") if p.is_file())


def timed_child(argv: list[str], log: Path) -> tuple[float, int]:
    """Run `argv` to completion; its wall time and its peak resident set in bytes."""
    with log.open("wb") as out:
        start = time.perf_counter()
        proc = subprocess.Popen(argv, stdout=out, stderr=subprocess.STDOUT)
        _, status, usage = os.wait4(proc.pid, 0)
        wall = time.perf_counter() - start
    if os.waitstatus_to_exitcode(status) != 0:
        raise StepFailed(
            f"{' '.join(argv[:2])} exited {os.waitstatus_to_exitcode(status)}: {log.read_text()[-2000:]}"
        )
    return wall, usage.ru_maxrss * 1024


def deployment(
    path: Path,
    bundle: Path,
    schema: Path,
    ports: tuple[int, int, int],
    state: Path | None = None,
    serve: dict | None = None,
) -> None:
    """A deployment serving `bundle`. Its cache, write-ahead log and catalogue sit beside the
    bundle, or in `state` where given; `serve` adds keys to `[serve]`."""
    viewer, session, control = ports
    stem = state / "state" if state is not None else bundle
    extra = "".join(f"{key} = {json.dumps(value)}\n" for key, value in (serve or {}).items())
    path.write_text(f"""
[bundle]
path = "{bundle}"
cache = "{stem}.cache"
wal = "{stem}.wal"

[catalogue]
dir = "{stem}.catalogue"

[build]
schema = "{schema}"

[disclosure]
token_max_lifetime = 86400

[serve]
viewer = "127.0.0.1:{viewer}"
session = "127.0.0.1:{session}"
control = "127.0.0.1:{control}"
operator_credential_env = "MOSAICA_BENCH_OPERATOR_CRED"
{extra}
[ingest]
flush_max_age_secs = 86400
compaction_window_start = "off"
""")


class Side:
    """One binary's run over the corpus, from an empty work directory."""

    def __init__(
        self,
        binary: Path,
        corpus: Path,
        work: Path,
        profile: Profile,
        seed: int,
        items: int,
        samples: int,
    ):
        self.binary, self.corpus, self.work = binary, corpus, work
        self.profile, self.seed, self.items, self.samples = profile, seed, items, samples
        self.figures: dict[str, float] = {}
        self.http = requests.Session()
        self.server: subprocess.Popen | None = None

    def run(self) -> dict[str, float]:
        shutil.rmtree(self.work, ignore_errors=True)
        self.work.mkdir(parents=True)
        bundle = self.work / "bundle"
        self.ports = (free_port(), free_port(), free_port())
        dep = self.work / "mosaica.toml"
        deployment(dep, bundle, self.corpus / "schema.toml", self.ports)
        try:
            wall, rss = timed_child(
                [str(self.binary), "build", "--deployment", str(dep), "--out", str(bundle)],
                self.work / "build.log",
            )
            self.figures["build s"] = wall
            self.figures["build peak RSS bytes"] = rss
            self.figures["bundle bytes"] = du(bundle)
            self.serve(dep)
            self.expect(self.items, "the build")
            self.queries("")
            ingested = self.ingest()
            self.expect(self.items + ingested, "the ingest")
            self.queries("after ingest ", only=("viewport z2 k200", "aggregate count"))
            removed = self.changes()
            self.expect(self.items + ingested - removed, "the deletions and suppressions")
            self.compact()
            self.expect(self.items + ingested - removed, "the compaction")
            self.figures["server peak RSS bytes"] = self.server_hwm()
        finally:
            self.stop()
            shutil.rmtree(self.work, ignore_errors=True)
        return self.figures

    # -- the server ----------------------------------------------------------------------------

    def url(self, plane: int, route: str) -> str:
        return f"http://127.0.0.1:{self.ports[plane]}{route}"

    def serve(self, dep: Path) -> None:
        env = dict(os.environ, MOSAICA_BENCH_OPERATOR_CRED=OPERATOR_CREDENTIAL)
        self.server_log = (self.work / "serve.log").open("wb")
        start = time.perf_counter()
        self.server = subprocess.Popen(
            [str(self.binary), "serve", "--deployment", str(dep)],
            stdout=self.server_log,
            stderr=subprocess.STDOUT,
            env=env,
        )
        deadline = start + 600
        while time.perf_counter() < deadline:
            if self.server.poll() is not None:
                raise StepFailed(
                    f"mosaica serve exited {self.server.returncode}: {self.log_tail()}"
                )
            try:
                if self.http.get(self.url(0, "/readyz"), timeout=2).status_code == 200:
                    self.figures["serve ready s"] = time.perf_counter() - start
                    return
            except (requests.ConnectionError, requests.Timeout):
                pass
            time.sleep(0.05)
        raise StepFailed(f"mosaica serve was not ready after 600 s: {self.log_tail()}")

    def log_tail(self) -> str:
        return (self.work / "serve.log").read_text(errors="replace")[-2000:]

    def server_hwm(self) -> int:
        for line in Path(f"/proc/{self.server.pid}/status").read_text().splitlines():
            if line.startswith("VmHWM:"):
                return int(line.split()[1]) * 1024
        return 0

    def stop(self) -> None:
        if self.server is not None and self.server.poll() is None:
            self.server.send_signal(signal.SIGTERM)
            try:
                self.server.wait(timeout=120)
            except subprocess.TimeoutExpired:
                self.server.kill()
                self.server.wait()
        self.server = None

    def post(
        self,
        plane: int,
        route: str,
        token: str,
        body,
        *,
        content_type: str = "application/json",
        headers=None,
    ) -> requests.Response:
        data = body if isinstance(body, bytes) else json.dumps(body).encode()
        response = self.http.post(
            self.url(plane, route),
            data=data,
            timeout=1800,
            headers={
                "Authorization": f"Bearer {token}",
                "Content-Type": content_type,
                **(headers or {}),
            },
        )
        if response.status_code >= 300:
            raise StepFailed(
                f"POST {route} answered {response.status_code}: {response.text[:1000]}"
            )
        return response

    def session(self, body: dict) -> str:
        return self.post(1, "/session/authorise", OPERATOR_CREDENTIAL, body).json()["token"]

    def count(self, token: str, filters: dict | None = None) -> int | None:
        """The items `token` sees, or those of them `filters` matches."""
        request = {"view": VIEW, "groupings": [{"count": True}]}
        if filters is not None:
            request["filters"] = filters
        body = self.post(0, "/v1/aggregate", token, request).content
        offset, got = 0, None
        while offset < len(body):
            kind, length = body[offset], int.from_bytes(body[offset + 1 : offset + 5], "little")
            if kind == 7:
                got = (
                    pa.ipc.open_stream(body[offset + 5 : offset + 5 + length])
                    .read_all()
                    .column("count")[0]
                    .as_py()
                )
            offset += 5 + length
        return got

    def expect(self, wanted: int, after: str) -> None:
        """Refuse a step that did less than it was asked to, so that its time is not reported."""
        got = self.count(self.session({"read_all": True}))
        if got != wanted:
            raise StepFailed(f"{got} items visible after {after}, where {wanted:,} were expected")

    # -- the steps -----------------------------------------------------------------------------

    def queries(self, prefix: str, only: tuple[str, ...] | None = None) -> None:
        """Each request of the battery, by each principal: the first answer after the session is
        minted (`cold`, which builds the authorised set) for the first request, then the median of
        `samples` answers for every request. A filter that matches nothing would time an empty
        answer, so each is refused unless it matches some item."""
        principals = {"all": {"read_all": True}, "narrow": {"terms": TERMS_NARROW}}
        for who, grant in principals.items():
            token = self.session(grant)
            first = True
            for name, (route, body) in battery(self.profile).items():
                if only is not None and name not in only:
                    continue
                if who == "all" and "filters" in body and not self.count(token, body["filters"]):
                    raise StepFailed(f"{name}: the filter {body['filters']} matches no item")
                if first:
                    start = time.perf_counter()
                    self.post(0, route, token, body).content
                    self.figures[f"{prefix}{name} [{who}] cold s"] = time.perf_counter() - start
                    first = False
                times = []
                for _ in range(self.samples):
                    start = time.perf_counter()
                    self.post(0, route, token, body).content
                    times.append(time.perf_counter() - start)
                self.figures[f"{prefix}{name} [{who}] s"] = statistics.median(times)

    def ingest(self) -> int:
        """New items, five in a hundred of the corpus, in Arrow batches; then a flush that waits
        until they are served. Returns the number of items."""
        total = max(INGEST_BATCH_ROWS, self.items // 20)
        batches = []
        for first in range(self.items, self.items + total, INGEST_BATCH_ROWS):
            table = rows(
                self.profile, first, min(INGEST_BATCH_ROWS, self.items + total - first), self.seed
            )
            sink = pa.BufferOutputStream()
            with pa.ipc.new_stream(sink, table.schema) as writer:
                writer.write_table(table)
            batches.append(sink.getvalue().to_pybytes())
        start = time.perf_counter()
        for number, batch in enumerate(batches):
            self.post(
                2,
                "/control/ingest",
                OPERATOR_CREDENTIAL,
                batch,
                content_type="application/vnd.apache.arrow.stream",
                headers={"x-mosaica-view": VIEW, "x-mosaica-batch-id": f"bench-{number}"},
            )
        self.figures[f"ingest {total} rows s"] = time.perf_counter() - start
        start = time.perf_counter()
        self.post(2, "/control/flush?wait=visible", OPERATOR_CREDENTIAL, b"")
        self.figures["flush until visible s"] = time.perf_counter() - start
        return total

    def changes(self) -> int:
        """Delete one item in a hundred and suppress one in a thousand, spread over the corpus.
        Returns the number of items changed."""
        step, changed = 100, 0
        for op, ids in (
            ("delete", range(0, self.items, step)),
            ("suppress", range(1, self.items, step * 10)),
        ):
            ids = list(ids)
            start = time.perf_counter()
            for i in range(0, len(ids), CHANGES_PER_REQUEST):
                body = [{"op": op, "match": {"id": n}} for n in ids[i : i + CHANGES_PER_REQUEST]]
                self.post(2, "/control/changes", OPERATOR_CREDENTIAL, body)
            self.figures[f"{op} {len(ids)} items s"] = time.perf_counter() - start
            changed += len(ids)
        return changed

    def compact(self) -> None:
        start = time.perf_counter()
        request = self.post(2, "/control/compact", OPERATOR_CREDENTIAL, b"").json()["request"]
        while True:
            answer = self.http.get(
                self.url(2, f"/control/compact/{request}"),
                headers={"Authorization": f"Bearer {OPERATOR_CREDENTIAL}"},
                timeout=60,
            ).json()
            if answer.get("answered"):
                break
            if time.perf_counter() - start > 3600:
                raise StepFailed("compaction was not answered within an hour")
            time.sleep(0.1)
        self.figures["compact s"] = time.perf_counter() - start
        if answer["answer"]["outcome"] != "compacted":
            raise StepFailed(f"compaction answered {json.dumps(answer['answer'])}")


def run(args) -> int:
    profile = PROFILES[args.profile]()
    items = args.items or profile.items
    work = Path(args.work)
    corpus = work / "corpus"
    start = time.perf_counter()
    write_corpus(profile, items, args.seed, corpus)
    print(
        f"{profile.name}: {items:,} items written in {time.perf_counter() - start:.1f} s",
        flush=True,
    )

    sides = {"baseline": Path(args.baseline).resolve(), "candidate": Path(args.candidate).resolve()}
    result = {
        "profile": profile.name,
        "about": profile.about,
        "items": items,
        "seed": args.seed,
        "repeat": args.repeat,
        "samples": args.samples,
        "machine": {
            "cpus": os.cpu_count(),
            "cpu": cpu_model(),
            "kernel": platform.release(),
            "memory_bytes": os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES"),
        },
        "labels": {"baseline": args.baseline_label, "candidate": args.candidate_label},
        "figures": {"baseline": {}, "candidate": {}},
        "errors": {"baseline": [], "candidate": []},
    }
    for rep in range(args.repeat):
        for name in ("baseline", "candidate") if rep % 2 == 0 else ("candidate", "baseline"):
            start = time.perf_counter()
            side = Side(sides[name], corpus, work / name, profile, args.seed, items, args.samples)
            try:
                figures = side.run()
            except (StepFailed, requests.RequestException, OSError, KeyError, ValueError) as error:
                result["errors"][name].append(f"repetition {rep}: {error}")
                print(f"  {name} repetition {rep} FAILED: {error}", flush=True)
                continue
            for key, value in figures.items():
                result["figures"][name].setdefault(key, []).append(value)
            print(f"  {name} repetition {rep}: {time.perf_counter() - start:.0f} s", flush=True)
    Path(args.out).write_text(json.dumps(result, indent=1))
    shutil.rmtree(work, ignore_errors=True)
    return 1 if result["errors"]["candidate"] else 0


def corpus(args) -> int:
    """Write a profile's corpus and build it, keeping the bundle."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from bench_cgroup import Capped, parse_bytes

    profile = PROFILES[args.profile]()
    items = args.items or profile.items
    out = Path(args.out).resolve()
    source, bundle = out / "corpus", out / "bundle"
    if (out / "corpus.json").exists():
        print(f"{out}: already built; remove it to rebuild", flush=True)
        return 0
    start = time.perf_counter()
    write_corpus(profile, items, args.seed, source)
    written = time.perf_counter() - start
    print(f"{profile.name}: {items:,} items written in {written:.1f} s", flush=True)
    dep = out / "build.toml"
    deployment(dep, bundle, source / "schema.toml", (free_port(), free_port(), free_port()))
    cap = parse_bytes(args.cap)
    argv = [str(Path(args.binary).resolve()), "build", "--deployment", str(dep), "--out", str(bundle)]
    with (out / "build.log").open("wb") as log:
        start = time.perf_counter()
        with Capped(argv, cap, stdout=log, stderr=subprocess.STDOUT) as build:
            code = build.proc.wait()
            stats = build.stats()
        wall = time.perf_counter() - start
    if code != 0:
        print(f"build exited {code}: {(out / 'build.log').read_text()[-2000:]}", file=sys.stderr)
        print(json.dumps(stats), file=sys.stderr)
        return 1
    figures = {
        "profile": profile.name,
        "items": items,
        "seed": args.seed,
        "view": VIEW,
        "extent": [0.0, 0.0, 1000.0, 1000.0]
        if profile.projection == "none"
        else [-180.0, -85.0, 180.0, 85.0],
        "terms": profile.terms,
        "bundle": str(bundle),
        "schema": str(source / "schema.toml"),
        "corpus_bytes": du(source),
        "bundle_bytes": du(bundle),
        "write_s": written,
        "build_s": wall,
        "build_cap_bytes": cap,
        "build_peak_bytes": stats.get("peak"),
        "build_oom_kills": stats.get("oom_kills"),
    }
    if not args.keep_source:
        for name in ("points.parquet", "members.parquet"):
            (source / name).unlink(missing_ok=True)
    (out / "corpus.json").write_text(json.dumps(figures, indent=1) + "\n")
    print(json.dumps(figures, indent=1), flush=True)
    return 0


def cpu_model() -> str:
    for line in Path("/proc/cpuinfo").read_text().splitlines():
        if line.startswith("model name"):
            return line.split(":", 1)[1].strip()
    return platform.processor()


# ---------------------------------------------------------------------------------------------
# The report
# ---------------------------------------------------------------------------------------------


def human(name: str, value: float) -> str:
    if name.endswith("bytes"):
        return f"{value / 2**20:,.1f} MiB"
    return f"{value * 1000:,.1f} ms" if value < 1 else f"{value:,.2f} s"


def compare(
    base: list[float], cand: list[float], threshold: float, floor: float
) -> tuple[float, str]:
    """The ratio of minima, and a verdict where each side has two figures or more to give it a
    range and the minima differ by more than `floor`."""
    ratio = min(cand) / min(base) if min(base) > 0 else float("inf")
    if min(len(base), len(cand)) < 2 or abs(min(cand) - min(base)) <= floor:
        return ratio, ""
    if ratio > 1 + threshold and min(cand) > max(base):
        return ratio, "slower"
    if ratio < 1 - threshold and max(cand) < min(base):
        return ratio, "faster"
    return ratio, ""


def report(args) -> int:
    lines, slower = [], 0
    for path in args.results:
        result = json.loads(Path(path).read_text())
        labels, machine = result["labels"], result["machine"]
        lines += [
            f"### {result['profile']}: {result['items']:,} items",
            "",
            f"{result['about']}. Baseline `{labels['baseline']}`, candidate `{labels['candidate']}`; "
            f"{result['repeat']} repetitions each, alternating, on {machine['cpus']} × {machine['cpu']}.",
            "",
        ]
        for side, errors in result["errors"].items():
            for error in errors:
                lines.append(f"- **{side} failed** {error[:500]}")
        if any(result["errors"].values()):
            lines.append("")
        base, cand = result["figures"]["baseline"], result["figures"]["candidate"]
        rows = []
        for name in base:
            if name not in cand:
                continue
            floor = args.floor if name.endswith(" s") else 0.0
            ratio, verdict = compare(base[name], cand[name], args.threshold, floor)
            if name.endswith(" cold s"):
                verdict = ""
            slower += verdict == "slower"
            mark = {"slower": "🔴 slower", "faster": "🟢 faster"}.get(verdict, "")
            rows.append(
                f"| {name} | {human(name, min(base[name]))} | {human(name, min(cand[name]))} | {ratio:.2f} | {mark} |"
            )
        if rows:
            lines += [
                "| figure | baseline | candidate | ratio | |",
                "|---|---:|---:|---:|---|",
                *rows,
                "",
            ]
    verdict = f"**{slower} figure(s) slower.**" if slower else "No figure slower."
    text = "\n".join([verdict, "", *lines])
    print(text)
    if args.summary:
        with open(args.summary, "a") as out:
            out.write(text + "\n")
    return 1 if args.fail_on_regression and slower else 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="command", required=True)
    r = sub.add_parser("run", help="measure one profile with both binaries")
    r.add_argument("--baseline", required=True, help="the baseline mosaica binary")
    r.add_argument("--candidate", required=True, help="the candidate mosaica binary")
    r.add_argument(
        "--baseline-label", default="baseline", help="what the report calls the baseline"
    )
    r.add_argument(
        "--candidate-label", default="candidate", help="what the report calls the candidate"
    )
    r.add_argument("--profile", required=True, choices=sorted(PROFILES))
    r.add_argument(
        "--items", type=int, help="items in the built corpus; absent is the profile's own"
    )
    r.add_argument("--repeat", type=int, default=3, help="runs of each binary")
    r.add_argument("--samples", type=int, default=5, help="answers timed per request per run")
    r.add_argument("--seed", type=int, default=0)
    r.add_argument("--work", required=True, help="scratch directory, emptied at the end")
    r.add_argument("--out", required=True, help="the result, as JSON")
    p = sub.add_parser("report", help="a Markdown table of one or more results")
    p.add_argument("results", nargs="+")
    p.add_argument(
        "--threshold",
        type=float,
        default=0.15,
        help="the ratio of minima a verdict needs, as a fraction",
    )
    p.add_argument(
        "--floor",
        type=float,
        default=0.002,
        help="the difference in seconds a timing's verdict needs, beyond the loopback's jitter",
    )
    p.add_argument("--summary", help="append the table to this file, such as $GITHUB_STEP_SUMMARY")
    p.add_argument("--fail-on-regression", action="store_true")
    c = sub.add_parser("corpus", help="write a corpus and build its bundle, and keep it")
    c.add_argument("--binary", required=True, help="the mosaica binary that builds")
    c.add_argument("--profile", required=True, choices=sorted(PROFILES))
    c.add_argument("--items", type=int, help="items in the corpus; absent is the profile's own")
    c.add_argument("--seed", type=int, default=0)
    c.add_argument("--out", required=True, help="directory for the corpus, bundle and corpus.json")
    c.add_argument("--cap", default="unbounded", help="memory cap on the build: 2G, or unbounded")
    c.add_argument(
        "--keep-source", action="store_true", help="keep the source Parquet once built"
    )
    args = parser.parse_args()
    return {"run": run, "report": report, "corpus": corpus}[args.command](args)


if __name__ == "__main__":
    sys.exit(main())
