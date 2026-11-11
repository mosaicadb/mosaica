#!/usr/bin/env python3
"""Many concurrent users on one box, as memory shrinks below the bundle.

    python3 scripts/bench_capacity.py run --profile smoke --data /data/bench --run-dir /data/runs/smoke
    python3 scripts/bench_capacity.py report /data/runs/smoke

`run` reads a profile from `bench/capacity.toml`, builds any corpus it names that is not yet in
`--data` (`bench_ab.py corpus`), then measures every cell of the profile's matrix:

    corpus x memory cap x cache policy x grant arm x users x think time

For each cell it boots `mosaica serve` inside its own memory cgroup of that cap
(`bench_cgroup.py`), so the page cache is charged to the cap and `bundle bytes / cap` is the
disk-to-memory ratio the cell runs at. It mints the cell's grants, evicts the bundle from the page
cache, and runs the load generator (`mosaica-bench load --mix`) twice: a `cold` window straight
after the boot, then a `warm` window. A sampler reads the cgroup's charge, anonymous memory, page
cache and major faults once a second throughout.

  * **Cache policy.** `default` serves with the server's own cache sizes, which sum to about
    8 GiB, and shows where they stop fitting a cap. `tuned` sets each cache to a fraction of the
    cap, the fractions written in the profile and in every record.
  * **Grant arm.** `A` gives every user its own grant, so each builds its own visible set and row
    projections: the memory a session costs. `B` shares a few grants among all users: the
    request path's throughput.
  * **Think time.** `0` drives each user as a saturating loop; a mean in milliseconds models a
    person who pauses between requests, so a cell of thousands of users stays meaningful.

A server killed by its cap, or one that does not come up, is a result: the cell is recorded with
`status` `oom`, `crashed` or `boot_failed` and the figures up to that point, and the run moves on.
Each cell is one JSON line in `RUN_DIR/cells.jsonl`; a killed run resumes by being run again.

Before any cell, the generator's own ceiling is measured against `/healthz` at each user count. A
cell whose throughput is within a third of it is flagged `generator_bound`: the client, not the
server, may be what it measured.

Creating cgroups needs root or a delegated cgroup subtree. Run under `scripts/bench-slot.sh` on a
shared box.
"""

from __future__ import annotations

import argparse
import itertools
import functools
import json
import math
import os
import random
import shutil
import socket
import subprocess
import sys
import threading
import time
import tomllib
from pathlib import Path

import requests

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
sys.path.insert(0, str(HERE))

from bench_ab import (  # noqa: E402
    CATEGORY_ZIPF,
    KEYWORD_SPAN,
    OPERATOR_CREDENTIAL,
    PROFILES,
    RANGES,
    TEXT_ZIPF,
    WORDS,
    Profile,
    category_size,
    deployment,
)
from bench_cgroup import Capped, drop_page_cache, files_under, human, parse_bytes  # noqa: E402

CONFIG = REPO / "bench" / "capacity.toml"


# ---------------------------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------------------------


def load_profile(path: Path, name: str) -> dict:
    config = tomllib.loads(path.read_text())
    profiles = config.get("profile", {})
    if name not in profiles:
        raise SystemExit(f"{path} has no [profile.{name}]; it has {', '.join(sorted(profiles))}")
    merged = dict(config.get("defaults", {}))
    for key, value in profiles[name].items():
        if isinstance(value, dict) and isinstance(merged.get(key), dict):
            merged[key] = {**merged[key], **value}
        else:
            merged[key] = value
    if isinstance(merged.get("mix"), str):
        mixes = config.get("mixes", {})
        if merged["mix"] not in mixes:
            raise SystemExit(
                f"[profile.{name}] names mix {merged['mix']!r}, but {path} has no "
                f"[mixes.{merged['mix']}]; it has {', '.join(sorted(mixes)) or 'none'}"
            )
        merged["mix"] = dict(mixes[merged["mix"]])
    merged["name"] = name
    return merged


def corpus_dir(data: Path, spec: dict) -> Path:
    return data / f"{spec['profile']}-{spec['items']}-s{spec.get('seed', 0)}"


def ensure_corpus(data: Path, spec: dict, binary: Path, build_cap: str) -> dict:
    out = corpus_dir(data, spec)
    if not (out / "corpus.json").exists():
        shutil.rmtree(out, ignore_errors=True)
        argv = [
            sys.executable,
            str(HERE / "bench_ab.py"),
            "corpus",
            "--binary",
            str(binary),
            "--profile",
            spec["profile"],
            "--items",
            str(spec["items"]),
            "--seed",
            str(spec.get("seed", 0)),
            "--out",
            str(out),
            "--cap",
            build_cap,
        ]
        print(f"building {out.name} under {build_cap}", flush=True)
        subprocess.run(argv, check=True, stdout=subprocess.DEVNULL)
    return json.loads((out / "corpus.json").read_text())


#: Filter families. A family in a mix is `filter_variants` entries named `<family>-NN` whose
#: filters differ only in their values, so the filter cache meets as many distinct filters as the
#: family has entries. `range`, `prefix`, `common`, `rare` and `combined` filter the overview;
#: `pan_range` filters a panning close-up and `agg_range` a cell-count aggregate.
FAMILIES = ("range", "pan_range", "agg_range", "prefix", "common", "rare", "combined")
#: The fewest items of the whole corpus a filter entry is expected to match. One expected to
#: match fewer is refused, since it would time an almost empty answer.
MIN_MATCHES = 100


@functools.cache
def zipf_pmf(exponent: float, size: int) -> tuple[float, ...]:
    """The probability of each code `numpy.minimum(rng.zipf(exponent), size) - 1` draws: the
    Zipf law normalised by the zeta function, summed by Euler-Maclaurin, and the last code taking
    the whole tail."""
    n = 10_000
    zeta = (
        math.fsum(k**-exponent for k in range(1, n))
        + n ** (1 - exponent) / (exponent - 1)
        + n**-exponent / 2
        + exponent * n ** (-exponent - 1) / 12
    )
    head = [(r + 1) ** -exponent / zeta for r in range(size - 1)]
    return (*head, 1 - math.fsum(head))


def word_fraction(rank: int, words: int) -> float:
    """The fraction of items whose text of `words` independent draws holds the word `w<rank>`."""
    return 1 - (1 - zipf_pmf(TEXT_ZIPF, WORDS)[rank]) ** words


def first_word(words: int, fraction: float) -> int:
    """The rank of the most frequent word that at most `fraction` of items hold."""
    return next(r for r in range(WORDS - 1) if word_fraction(r, words) <= fraction)


def prefix_fraction(digits: int) -> float:
    """The fraction of keywords `k<n>`, `n` uniform over `0 .. KEYWORD_SPAN`, that start with
    `k<digits>`."""
    held, scale = 0, 1
    while digits * scale < KEYWORD_SPAN:
        held += min((digits + 1) * scale, KEYWORD_SPAN) - digits * scale
        scale *= 10
    return held / KEYWORD_SPAN


def filter_columns(profile: Profile) -> dict:
    """The columns the filter families read, each None where the profile has none: an indexed
    integer column of at least 65,536 values, an indexed keyword, an indexed text column, and a
    rendered or indexed category of 40 codes."""

    def first(test):
        return next((c for c in profile.columns if test(c)), None)

    return {
        "integer": first(
            lambda c: c.type in RANGES and c.index and RANGES[c.type][1] - RANGES[c.type][0] >= 65_535
        ),
        "keyword": first(lambda c: c.type == "keyword" and c.index),
        "text": first(lambda c: c.type == "text" and c.index),
        "category": first(
            lambda c: c.type == "category" and (c.render or c.index) and category_size(c) == 40
        ),
    }


def filter_family(family: str, corpus: dict, variants: int, close: list, k: int = 200) -> list[tuple]:
    """`variants` requests of one family over `corpus`, each `(route, body, pan, fraction)`, where
    `fraction` is the share of the corpus's items the filter is expected to match under the
    generators in `bench_ab.py`. There integers are uniform, a keyword is `k` and a uniform
    integer, text words are Zipf-ranked and categories Zipf-coded, each column drawn independently
    of the others, so a conjunction's fraction is the product of its leaves'."""
    profile = PROFILES[corpus["profile"]]()
    columns = filter_columns(profile)
    view, extent = corpus["view"], corpus["extent"]

    def column(role: str, what: str):
        if columns[role] is None:
            raise SystemExit(
                f"mix kind {family!r} filters on {what}, and a {profile.name!r} corpus has none; "
                f"serve a `text` or `fields` corpus, or take {family!r} out of the mix"
            )
        return columns[role]

    def window(i: int, slot: int, share: int) -> tuple[dict, float]:
        """A range holding 1/`share` of the integer column's values, at the `i`th of `variants`
        places; `slot` moves it a width along, so that two families' windows differ."""
        c = column("integer", "an indexed integer column")
        lo, hi = RANGES[c.type]
        span = hi - lo + 1
        width = span // share
        start = lo + (i * (span // variants) + slot * width) % (span - width + 1)
        return {c.name: {"range": {"gte": start, "lte": start + width - 1}}}, width / span

    def word(rank: int) -> tuple[dict, float]:
        c = column("text", "an indexed text column")
        return {c.name: {"match": f"w{rank}"}}, word_fraction(rank, profile.text_words)

    def overview(leaf: dict, fraction: float) -> tuple:
        body = {"view": view, "zoom": 2, "bbox": extent, "k": k, "filters": leaf}
        return "/v1/viewport", body, None, fraction

    out = []
    for i in range(variants):
        if family == "range":
            out.append(overview(*window(i, 0, 100)))
        elif family == "agg_range":
            leaf, fraction = window(i, 1, 100)
            body = {
                "view": view,
                "filters": leaf,
                "groupings": [{"count": True, "cells": {"depth": 6}}],
            }
            out.append(("/v1/aggregate", body, None, fraction))
        elif family == "pan_range":
            leaf, fraction = window(i, 2, 100)
            body = {"view": view, "zoom": 6, "bbox": close, "k": k, "filters": leaf}
            out.append(("/v1/viewport", body, extent, fraction))
        elif family == "prefix":
            c = column("keyword", "an indexed keyword column")
            out.append(overview({c.name: {"prefix": f"k{11 + i}"}}, prefix_fraction(11 + i)))
        elif family == "common":
            out.append(overview(*word(first_word(profile.text_words, 0.25) + i)))
        elif family == "rare":
            out.append(overview(*word(first_word(profile.text_words, 1e-4) + i)))
        elif family == "combined":
            if columns["text"] is not None:
                first, first_fraction = word(first_word(profile.text_words, 0.25) + variants + i)
            else:
                c = column("category", "an indexed text column or a category of 40 codes")
                code = i % category_size(c)
                first = {c.name: {"eq": f"{c.vocabulary}{code}"}}
                first_fraction = zipf_pmf(CATEGORY_ZIPF, category_size(c))[code]
            second, second_fraction = window(i, 0, 10)
            out.append(overview({"all_of": [first, second]}, first_fraction * second_fraction))
    return out


def mix_file(
    corpus: dict, weights: dict, path: Path, variants: int = 20, k: int = 200
) -> dict[str, float]:
    """The workload's requests over `corpus`, weighted as the profile says: panning close-ups,
    overviews, filtered close-ups, aggregates and item pages, and the filter families of
    `FAMILIES`, every viewport asking for at most `k` points a tile. Each entry of a family weighs the family's weight and every other kind `variants`
    times its own, so that a family weighs its weight in all. Returns the fraction of the corpus's
    items each family entry is expected to match."""
    view = corpus["view"]
    x0, y0, x1, y1 = corpus["extent"]
    close = [x0, y0, x0 + (x1 - x0) / 8, y0 + (y1 - y0) / 8]
    kinds = {
        "pan": (
            "/v1/viewport",
            {"view": view, "zoom": 6, "bbox": close, "k": k},
            corpus["extent"],
        ),
        "overview": (
            "/v1/viewport",
            {"view": view, "zoom": 2, "bbox": corpus["extent"], "k": k},
            None,
        ),
        "filtered": (
            "/v1/viewport",
            {
                "view": view,
                "zoom": 6,
                "bbox": close,
                "k": k,
                "filters": {"kind": {"in": ["kind0", "kind1", "kind2"]}},
            },
            corpus["extent"],
        ),
        "aggregate": (
            "/v1/aggregate",
            {"view": view, "groupings": [{"count": True, "cells": {"depth": 6}}]},
            None,
        ),
        "items": (
            "/v1/items",
            {"view": view, "fields": [], "page_rows": 1000, "pages": 1},
            None,
        ),
    }
    scale = variants if any(kind in FAMILIES for kind in weights) else 1
    requests_, expected = [], {}
    for kind, weight in weights.items():
        if kind in FAMILIES:
            for i, (route, body, pan, fraction) in enumerate(
                filter_family(kind, corpus, variants, close, k)
            ):
                name = f"{kind}-{i:02d}"
                if fraction * corpus["items"] < MIN_MATCHES:
                    raise SystemExit(
                        f"mix kind {name} is expected to match {fraction * corpus['items']:.0f} of "
                        f"the corpus's {corpus['items']:,} items; serve a corpus of at least "
                        f"{math.ceil(MIN_MATCHES / fraction):,} items, or take {kind!r} out of the mix"
                    )
                entry = {"kind": name, "route": route, "weight": int(weight), "body": body}
                if pan is not None:
                    entry["pan"] = pan
                entry["expected_fraction"] = fraction
                expected[name] = fraction
                requests_.append(entry)
            continue
        if kind not in kinds:
            raise SystemExit(f"mix kind {kind!r} is not one of {', '.join([*kinds, *FAMILIES])}")
        if kind == "filtered" and not any(
            c.name == "kind" for c in PROFILES[corpus["profile"]]().columns
        ):
            raise SystemExit(
                f"mix kind 'filtered' filters on `kind`, which a {corpus['profile']!r} corpus "
                "does not have; take it out of the mix"
            )
        route, body, pan = kinds[kind]
        entry = {"kind": kind, "route": route, "weight": int(weight) * scale, "body": body}
        if pan is not None:
            entry["pan"] = pan
        requests_.append(entry)
    path.write_text(json.dumps({"requests": requests_}, indent=1))
    return expected


def cache_settings(policy: str, cap: int | None, fractions: dict) -> dict:
    if policy == "default" or cap is None:
        return {}
    if policy != "tuned":
        raise SystemExit(f"cache policy {policy!r} is neither default nor tuned")
    return {key: max(int(cap * fraction), 1 << 20) for key, fraction in fractions.items()}


# ---------------------------------------------------------------------------------------------
# One served deployment
# ---------------------------------------------------------------------------------------------


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class Server:
    def __init__(self, binary: Path, corpus: dict, cap: int | None, serve: dict, cpus, work: Path):
        self.work, self.bundle = work, Path(corpus["bundle"])
        shutil.rmtree(work, ignore_errors=True)
        work.mkdir(parents=True)
        self.ports = (free_port(), free_port(), free_port())
        self.dep = work / "mosaica.toml"
        deployment(
            self.dep,
            Path(corpus["bundle"]),
            Path(corpus["schema"]),
            self.ports,
            state=work,
            serve=serve,
        )
        self.log = (work / "serve.log").open("wb")
        env = dict(os.environ, MOSAICA_BENCH_OPERATOR_CRED=OPERATOR_CREDENTIAL)
        self.capped = Capped(
            [str(binary), "serve", "--deployment", str(self.dep)],
            cap,
            name=f"serve-{os.getpid()}-{time.monotonic_ns()}",
            cpus=cpus,
            stdout=self.log,
            stderr=subprocess.STDOUT,
            env=env,
        )
        self.http = requests.Session()

    def url(self, plane: int, route: str = "") -> str:
        return f"http://127.0.0.1:{self.ports[plane]}{route}"

    def start(self, timeout: float) -> float:
        drop_page_cache(files_under([self.bundle]))
        start = time.perf_counter()
        self.capped.start()
        while time.perf_counter() - start < timeout:
            if self.capped.proc.poll() is not None:
                raise ServerDied(self.capped.proc.returncode)
            try:
                if self.http.get(self.url(0, "/readyz"), timeout=2).status_code == 200:
                    return time.perf_counter() - start
            except (requests.ConnectionError, requests.Timeout):
                pass
            time.sleep(0.05)
        raise TimeoutError(f"not ready after {timeout:.0f} s")

    def alive(self) -> bool:
        return self.capped.proc is not None and self.capped.proc.poll() is None

    def authorise(self, terms: list[str]) -> str:
        r = self.http.post(
            self.url(1, "/session/authorise"),
            json={"terms": terms},
            headers={"Authorization": f"Bearer {OPERATOR_CREDENTIAL}"},
            timeout=60,
        )
        r.raise_for_status()
        return r.json()["token"]

    def hwm(self) -> int | None:
        try:
            for line in Path(f"/proc/{self.capped.proc.pid}/status").read_text().splitlines():
                if line.startswith("VmHWM:"):
                    return int(line.split()[1]) * 1024
        except OSError:
            pass
        return None

    def log_tail(self) -> str:
        self.log.flush()
        return (self.work / "serve.log").read_text(errors="replace")[-1500:]

    def close(self) -> None:
        self.capped.close()
        self.log.close()


class ServerDied(RuntimeError):
    pass


class Sampler(threading.Thread):
    """The cgroup's figures once a second, until stopped."""

    def __init__(self, capped: Capped, period: float = 1.0):
        super().__init__(daemon=True)
        self.capped, self.period = capped, period
        self.series: list[dict] = []
        self.halt = threading.Event()
        self.t0 = time.perf_counter()

    def run(self) -> None:
        while not self.halt.is_set():
            self.mark()
            self.halt.wait(self.period)

    def mark(self) -> dict:
        stats = self.capped.stats()
        stats["t"] = round(time.perf_counter() - self.t0, 2)
        self.series.append(stats)
        return stats

    def stop(self) -> list[dict]:
        self.halt.set()
        self.join()
        return self.series


def summarise_memory(series: list[dict]) -> dict:
    if not series:
        return {}
    pick = lambda key: [s[key] for s in series if s.get(key) is not None]  # noqa: E731
    return {
        "usage_max": max(pick("usage"), default=None),
        "anon_max": max(pick("anon"), default=None),
        "file_min": min(pick("file"), default=None),
        "file_mean": sum(pick("file")) / max(len(pick("file")), 1),
        "majfault": series[-1].get("majfault", 0) - series[0].get("majfault", 0),
        "limit_hits": series[-1].get("limit_hits", 0) - series[0].get("limit_hits", 0),
        "oom_kills": series[-1].get("oom_kills", 0),
    }


# ---------------------------------------------------------------------------------------------
# The generator
# ---------------------------------------------------------------------------------------------


def generate(
    bench: Path,
    viewer: str,
    tokens: Path,
    users: int,
    duration: float,
    profile: dict,
    run_dir: Path,
    mix: Path | None,
    think_ms: float,
    healthz: bool = False,
) -> dict | None:
    """One `mosaica-bench load` window; its record, or `None` where it produced none."""
    shutil.rmtree(run_dir, ignore_errors=True)
    argv = [
        str(bench),
        "--run-dir",
        str(run_dir),
        "load",
        "--viewer-url",
        viewer,
        "--tokens",
        str(tokens),
        "--concurrency",
        str(users),
        "--duration-s",
        str(duration),
        "--threads",
        str(profile["generator_threads"]),
        "--think-ms",
        str(think_ms),
        "--hang-timeout-ms",
        str(profile["hang_timeout_ms"]),
        "--seed",
        str(profile["seed"]),
    ]
    if healthz:
        argv.append("--healthz")
    elif mix is not None:
        argv += ["--mix", str(mix)]
    if profile.get("generator_cpus"):
        argv = ["taskset", "-c", profile["generator_cpus"], *argv]
    done = subprocess.run(argv, capture_output=True, text=True)
    out = run_dir / "load.jsonl"
    if not out.exists():
        print(f"    generator produced no record: {done.stderr[-500:]}", flush=True)
        return None
    lines = out.read_text().splitlines()
    return json.loads(lines[-1]) if lines else None


def figures(record: dict | None) -> dict:
    if record is None:
        return {}
    p, t = record.get("params", {}), record.get("timing", {})
    return {
        "throughput_rps": p.get("throughput_rps"),
        "requests": p.get("requests_total"),
        "ok": p.get("requests_ok"),
        "shed_rate": p.get("shed_rate"),
        "errors": p.get("requests_error"),
        "hangs": p.get("requests_hung"),
        "p50_ms": t.get("median_ns", 0) / 1e6,
        "p99_ms": t.get("p99_ns", 0) / 1e6,
        "max_ms": t.get("max_ns", 0) / 1e6,
        "server_us_p99": p.get("server_us_p99"),
        "points_per_second": p.get("points_per_second"),
        "kinds": p.get("kinds"),
        "flags": record.get("flags", []),
    }


# ---------------------------------------------------------------------------------------------
# The run
# ---------------------------------------------------------------------------------------------


def grants(arm: str, users: int, terms: int, width: int, shared: int, seed: int) -> list[list[str]]:
    rng = random.Random(seed)
    count = users if arm == "A" else min(shared, users)
    return [[f"t{t}" for t in rng.sample(range(terms), min(width, terms))] for _ in range(count)]


def cell_key(c: dict) -> str:
    return (
        f"{Path(c['bundle']).parent.name}/cap={human(c['cap'])}/{c['policy']}/arm={c['arm']}"
        f"/users={c['users']}/think={c['think_ms']}"
        + (f"/k={c['k']}" if c["k"] != 200 else "")
    )


def measure_ceiling(profile: dict, binary: Path, bench: Path, corpus: dict, work: Path) -> dict:
    """The generator's own throughput against `/healthz` at each user count."""
    server = Server(binary, corpus, None, {}, profile.get("server_cpus"), work / "ceiling")
    ceiling = {}
    try:
        server.start(profile["boot_timeout_s"])
        tokens = work / "ceiling" / "tokens"
        tokens.write_text("unused\n")
        for users in profile["users"]:
            record = generate(
                bench, server.url(0), tokens, users, 5.0, profile, work / "ceiling" / "gen", None, 0, True
            )
            ceiling[str(users)] = (record or {}).get("params", {}).get("throughput_rps")
            print(f"  generator ceiling at {users} users: {ceiling[str(users)]} rps", flush=True)
    finally:
        server.close()
    return ceiling


def run_cell(cell: dict, profile: dict, binary: Path, bench: Path, corpus: dict, work: Path) -> dict:
    serve = {**profile.get("serve", {}), **cell["cache"]}
    server = Server(binary, corpus, cell["cap"], serve, profile.get("server_cpus"), work / "server")
    record = dict(cell, status="ok")
    try:
        try:
            record["boot_s"] = server.start(profile["boot_timeout_s"])
        except (ServerDied, TimeoutError) as error:
            stats = server.capped.stats()
            record["status"] = "oom" if stats.get("oom_kills") else "boot_failed"
            record["error"] = f"{error}: {server.log_tail()}"
            record["memory"] = {"boot": stats}
            return record
        record["memory_after_boot"] = server.capped.stats()

        mint_start = time.perf_counter()
        tokens = work / "tokens"
        tokens.write_text(
            "\n".join(
                server.authorise(terms)
                for terms in grants(
                    cell["arm"],
                    cell["users"],
                    corpus["terms"],
                    profile["grant_width"],
                    profile["shared_grants"],
                    profile["seed"] + cell["index"],
                )
            )
            + "\n"
        )
        record["mint_s"] = time.perf_counter() - mint_start
        record["distinct_grants"] = len(tokens.read_text().split())

        if server.capped.group is not None:
            server.capped.group.evict(files_under([Path(corpus["bundle"])]))
        mix = work / "mix.json"
        expected = mix_file(
            corpus, profile["mix"], mix, profile.get("filter_variants", 20), cell["k"]
        )
        if expected:
            record["expected_fraction"] = expected

        for phase, duration in (("cold", profile["cold_s"]), ("warm", profile["warm_s"])):
            sampler = Sampler(server.capped)
            sampler.start()
            result = generate(
                bench,
                server.url(0),
                tokens,
                cell["users"],
                duration,
                profile,
                work / f"gen-{phase}",
                mix,
                cell["think_ms"],
            )
            series = sampler.stop()
            record[phase] = figures(result)
            record[phase]["memory"] = summarise_memory(series)
            record[phase]["memory_series"] = [
                [s["t"], s.get("usage"), s.get("anon"), s.get("file"), s.get("majfault")]
                for s in series
            ]
            if not server.alive():
                stats = server.capped.stats()
                record["status"] = "oom" if stats.get("oom_kills") else "crashed"
                record["error"] = f"server exited {server.capped.proc.returncode} in the {phase} window: {server.log_tail()}"
                break
        record["server_hwm"] = server.hwm()
        record["memory_end"] = server.capped.stats()
    except requests.RequestException as error:
        stats = server.capped.stats()
        record["status"] = "oom" if stats.get("oom_kills") else "crashed"
        record["error"] = f"{error}: {server.log_tail()}"
    finally:
        server.close()
    return record


def run(args) -> int:
    profile = load_profile(args.config, args.profile)
    binary, bench = Path(profile["binary"]), Path(profile["bench"])
    binary = binary if binary.is_absolute() else REPO / binary
    bench = bench if bench.is_absolute() else REPO / bench
    data, run_dir = args.data.resolve(), args.run_dir.resolve()
    data.mkdir(parents=True, exist_ok=True)
    run_dir.mkdir(parents=True, exist_ok=True)
    work = run_dir / "work"

    corpora = [ensure_corpus(data, spec, binary, profile["build_cap"]) for spec in profile["corpora"]]
    (run_dir / "profile.json").write_text(json.dumps(profile, indent=1))
    for corpus in corpora:
        mix_file(
            corpus,
            profile["mix"],
            run_dir / f"mix-{Path(corpus['bundle']).parent.name}.json",
            profile.get("filter_variants", 20),
            profile["points_per_tile"][0],
        )

    ceiling_path = run_dir / "ceiling.json"
    if not ceiling_path.exists():
        ceiling_path.write_text(json.dumps(measure_ceiling(profile, binary, bench, corpora[0], work)))
    ceiling = json.loads(ceiling_path.read_text())

    out = run_dir / "cells.jsonl"
    done = set()
    if out.exists():
        done = {json.loads(line)["key"] for line in out.read_text().splitlines() if line.strip()}

    cells = []
    for corpus in corpora:
        for cap_text in profile["caps"]:
            cap = parse_bytes(cap_text)
            for policy in profile["policies"]:
                if policy == "tuned" and cap is None:
                    continue
                for arm in profile["arms"]:
                    for users in profile["users"]:
                        for think_ms, k in itertools.product(
                            profile["think_ms"], profile["points_per_tile"]
                        ):
                            cells.append(
                                {
                                    "profile": profile["name"],
                                    "bundle": corpus["bundle"],
                                    "items": corpus["items"],
                                    "bundle_bytes": corpus["bundle_bytes"],
                                    "cap": cap,
                                    "ratio": corpus["bundle_bytes"] / cap if cap else None,
                                    "policy": policy,
                                    "cache": cache_settings(policy, cap, profile["tuned"]),
                                    "arm": arm,
                                    "users": users,
                                    "think_ms": think_ms,
                                    "k": k,
                                }
                            )
    for index, cell in enumerate(cells):
        cell["index"] = index
        cell["key"] = cell_key(cell)
        if cell["key"] in done:
            continue
        corpus = next(c for c in corpora if c["bundle"] == cell["bundle"])
        print(f"[{index + 1}/{len(cells)}] {cell['key']}", flush=True)
        record = run_cell(cell, profile, binary, bench, corpus, work)
        bound = ceiling.get(str(cell["users"]))
        for phase in ("cold", "warm"):
            got = record.get(phase, {}).get("throughput_rps")
            if bound and got and got * 3 >= bound and not cell["think_ms"]:
                record[phase].setdefault("flags", []).append("generator_bound")
        record["machine"] = {"cpus": os.cpu_count(), "kernel": os.uname().release}
        with out.open("a") as f:
            f.write(json.dumps(record) + "\n")
        warm = record.get("warm", {})
        print(
            f"    {record['status']}: warm {warm.get('throughput_rps') or 0:,.0f} rps, "
            f"p99 {warm.get('p99_ms') or 0:.1f} ms, "
            f"peak {human_bytes((warm.get('memory') or {}).get('usage_max'))}, "
            f"majfault {(warm.get('memory') or {}).get('majfault')}",
            flush=True,
        )
    shutil.rmtree(work, ignore_errors=True)
    return 0


# ---------------------------------------------------------------------------------------------
# The report
# ---------------------------------------------------------------------------------------------


def human_bytes(n) -> str:
    if n is None:
        return "-"
    for unit, size in (("GiB", 1 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)):
        if n >= size:
            return f"{n / size:.1f} {unit}"
    return f"{n} B"


def family_rollup(cells: list[dict]) -> list[str]:
    """The warm window's kinds of each cell, with the kinds `name-NN` of one family summed into a
    row: throughput, sheds and errors summed, the p99 and p99.9 the worst entry's. Empty where no
    cell has a family."""
    rows = []
    for c in sorted(
        cells,
        key=lambda c: (c["items"], -(c["cap"] or 1 << 62), c["policy"], c["arm"], c["users"], c["think_ms"]),
    ):
        groups: dict[str, list[tuple[str, dict]]] = {}
        for kind, k in ((c.get("warm") or {}).get("kinds") or {}).items():
            family, _, number = kind.rpartition("-")
            groups.setdefault(family if family and number.isdigit() else kind, []).append((kind, k))
        if all(len(members) == 1 and members[0][0] == name for name, members in groups.items()):
            continue
        expected = c.get("expected_fraction") or {}
        for family, members in groups.items():
            ks = [k for _, k in members]
            fractions = [expected[kind] for kind, _ in members if kind in expected]
            matched = "-"
            if fractions:
                matched = f"{min(fractions):.4%}"
                if max(fractions) != min(fractions):
                    matched += f"–{max(fractions):.4%}"
            rows.append(
                f"| {c['items']:,} | {human(c['cap'])} | {c['policy']} | {c['arm']} | {c['users']} "
                f"| {c['think_ms']} | {family} | {len(ks)} "
                f"| {sum(k['throughput_rps'] for k in ks):,.0f} | {max(k['p99_ms'] for k in ks):.1f} "
                f"| {max(k['p999_ms'] for k in ks):.1f} | {sum(k['shed_429'] for k in ks)} "
                f"| {sum(k['errors'] for k in ks)} | {matched} |"
            )
    return rows


def report(args) -> int:
    cells = [
        json.loads(line)
        for line in (args.run_dir / "cells.jsonl").read_text().splitlines()
        if line.strip()
    ]
    rows = [
        "| items | bundle | cap | ratio | policy | arm | users | think ms | status | phase | rps | p50 ms "
        "| p99 ms | server p99 ms | shed | peak charge | page cache (min) | major faults | flags |",
        "|" + "---|" * 19,
    ]
    for c in sorted(
        cells,
        key=lambda c: (c["items"], -(c["cap"] or 1 << 62), c["policy"], c["arm"], c["users"], c["think_ms"]),
    ):
        for phase in ("cold", "warm"):
            p = c.get(phase)
            if p is None and phase == "warm" and "cold" in c:
                continue
            p = p or {}
            m = p.get("memory") or {}
            rows.append(
                "| "
                + " | ".join(
                    str(x)
                    for x in (
                        f"{c['items']:,}",
                        human_bytes(c["bundle_bytes"]),
                        human(c["cap"]),
                        f"{c['ratio']:.2f}" if c["ratio"] else "-",
                        c["policy"],
                        c["arm"],
                        c["users"],
                        c["think_ms"],
                        c["status"],
                        phase if p else "-",
                        f"{p.get('throughput_rps') or 0:,.0f}",
                        f"{p.get('p50_ms') or 0:.1f}",
                        f"{p.get('p99_ms') or 0:.1f}",
                        f"{(p.get('server_us_p99') or 0) / 1000:.1f}",
                        f"{(p.get('shed_rate') or 0):.1%}",
                        human_bytes(m.get("usage_max")),
                        human_bytes(m.get("file_min")),
                        m.get("majfault", "-"),
                        ", ".join(p.get("flags") or []),
                    )
                )
                + " |"
            )
    text = "\n".join(rows) + "\n"
    kinds = []
    for c in cells:
        for kind, k in ((c.get("warm") or {}).get("kinds") or {}).items():
            kinds.append(
                f"| {human(c['cap'])} | {c['policy']} | {c['arm']} | {c['users']} | {c['think_ms']} | {kind} "
                f"| {k['throughput_rps']:,.0f} | {k['p50_ms']:.1f} | {k['p99_ms']:.1f} | {k['p999_ms']:.1f} "
                f"| {k['shed_429']} | {k['errors']} |"
            )
    if kinds:
        text += (
            "\nWarm window, by request kind:\n\n"
            "| cap | policy | arm | users | think ms | kind | rps | p50 ms | p99 ms | p99.9 ms | shed | errors |\n"
            "|" + "---|" * 12 + "\n" + "\n".join(kinds) + "\n"
        )
    families = family_rollup(cells)
    if families:
        text += (
            "\nWarm window, by request family (`name-NN` kinds summed; p99 the worst entry's; "
            "matched the fraction of the corpus each entry's filter is expected to match):\n\n"
            "| items | cap | policy | arm | users | think ms | family | entries | rps | p99 ms "
            "| p99.9 ms | shed | errors | matched |\n"
            "|" + "---|" * 14 + "\n" + "\n".join(families) + "\n"
        )
    (args.run_dir / "report.md").write_text(text)
    print(text)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="command", required=True)
    r = sub.add_parser("run", help="measure a profile's matrix")
    r.add_argument("--profile", required=True)
    r.add_argument("--config", type=Path, default=CONFIG)
    r.add_argument("--data", type=Path, required=True, help="where corpora are built and kept")
    r.add_argument("--run-dir", type=Path, required=True)
    p = sub.add_parser("report", help="a Markdown table of a run's cells")
    p.add_argument("run_dir", type=Path)
    args = parser.parse_args()
    return run(args) if args.command == "run" else report(args)


if __name__ == "__main__":
    sys.exit(main())
