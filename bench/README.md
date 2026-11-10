# Running the benchmarks

How to run the suite at whatever scales and label sets you want, and how to read what comes out.

Findings and methodology live elsewhere: each arm's module doc in
[`crates/mosaica-bench/src/arms/`](../crates/mosaica-bench/src/arms/) records what it measured and
what it means. This file is operational.

---

## 0. Prerequisites

```bash
cargo build --release -p mosaica     # the build/serve binary
cargo build --release -p mosaica-bench   # the harness (bench-timing is ON by default here)
```

The corpus must be present under `data/scaled/` — `geometry.parquet` (10⁹ rows) plus one
`<label-set>.pairs.parquet` per label set. See the fixture design for what
it is and how it was built.

The concurrency arm additionally needs the Python environment at `reference/.venv`.

---

## 1. The two configuration axes

**Scale is a prefix filter, not a separate corpus.** `mosaica build --limit N` keeps source rows
with `entity_id < N`. Any N works — 5,000,000 is as valid as the defaults — because there is one
geometry file and a scale is a view onto it. Never take "the first N rows" of `geometry.parquet`
instead: it is stored in Morton order, not entity order (the fixture design).

**Label set is the pairs file.** Geometry is label-independent and shared; only the
`(entity_id, term_id)` relation changes. But the label set *does* decide entity-ID assignment via
the signature sort, so it cannot be varied without a rebuild — see §6.

| label set | terms @10⁹ | character |
|---|---|---|
| `categories-archive` | 15,694 | maximal entity-space contiguity |
| `categories-subclass` | 47,968 | realistic baseline |
| `hash-flat` | 10,000 | maximal scatter — **the G0b control arm** |
| `surnames` | 116,946,544 | dictionary scale, near-unique signatures |
| `hiterms`, `hiterms-ov0.5`, `hiterms-ov0.9` | 1,000,000 | high terms-per-item |

> **The `hiterms` trio covers `entity_id < 10⁷` only.** Above that its tail carries no terms at
> all, so those items are invisible to every principal — a corpus with a hole in it rather than a
> smaller corpus. `bench_build_fixtures.sh` skips those combinations rather than building them.

---

## 2. Build fixtures

Defaults are 250k / 2.42M / 25M across all seven label sets (~7 GB, ~40 min):

```bash
./scripts/bench_build_fixtures.sh
```

Both axes take comma-separated flags; `--help` lists them all:

```bash
# Just the two contiguity extremes, at one scale
./scripts/bench_build_fixtures.sh --scales 2422486 --label-sets categories-archive,hash-flat

# A scale that isn't one of the defaults — any integer works, it's a --limit prefix filter
./scripts/bench_build_fixtures.sh --scales 5000000 --label-sets categories-subclass

# Somewhere other than /tmp, and preview instead of building
./scripts/bench_build_fixtures.sh --fixtures /data/fixtures --list
```

`--list` prints the plan and builds nothing — worth running first, since a full default build is
~40 minutes. Idempotent otherwise: a bundle whose `CURRENT` exists is reported `HAVE` and skipped,
so a killed run resumes by re-invocation. Per-build logs go to `--log-dir` (default
`/tmp/mosaica-bench/logs`).

Budget roughly **47 B/item** of disk (measured flat across 100× of scale), plus the label set's
postings — `hiterms` is ~10× the others because it carries ~130 terms per item.

Check what you have:

```bash
./target/release/mosaica-bench fixtures
```

---

## 3. Run one arm

Every arm takes `--scale` and `--label-set` as comma-separated filters. Omit them for every
fixture found. A filter matching nothing is an error, not an empty run.

```bash
# Mask construction across every fixture
./target/release/mosaica-bench authorise --run-dir /tmp/run1

# One scale, two label sets, custom axes
./target/release/mosaica-bench gather \
    --scale 25000000 --label-set categories-archive,hash-flat \
    --pattern contiguous,scattered --k 1000 --coverage 0.05 \
    --repeat 7 --run-dir /tmp/run1

# Whole-viewport latency with per-stage attribution
./target/release/mosaica-bench viewport \
    --scale 2422486 --mode battery --k 30 --coverage 0.05 --run-dir /tmp/run1

# Design §7.2's density rule, swept over its own clause parameters
./target/release/mosaica-bench viewport \
    --scale 2422486 --mode battery --k 1000 --coverage 0.05 \
    --k-max-marks 30,128,500,1000 --theta-target 16,128 --run-dir /tmp/selection
```

**`--k-max-marks` and `--theta-target` are not free axes.** §7.2's clause parameters are resolved at
`Engine::open`, not per request, so each value costs a full open — which digest-verifies the bundle.
Both default to the server's own single value, so an ordinary run pays nothing; pass a list only
when the sweep is the point. What they move that `--k` does not: the cap bounds the selection heap
and the output gather, and θ decides how many tiles skip the counting pass entirely. A
`--theta-target` large enough to saturate (e.g. `1099511627776`) makes every tile take the serve-all
branch, which is the control arm for the other values.

`--help` on any subcommand lists its axes and defaults. Global flags: `--fixtures`, `--run-dir`,
`--repeat`, `--scale`, `--label-set`.

Arms available: `authorise`, `tiles`, `gather`, `viewport`, `changes`, `ingest-batch`,
`ingest-continuous`, `ingest-build`, `load`, plus `fixtures`, `calibrate` and `matrix`.

Run `calibrate` before believing anything — it reports clock overhead (the instrumentation's own
perturbation), core count, free memory, and whether `bench-timing` is actually on.

---

## 4. Run a campaign

[`bench/matrix.toml`](matrix.toml) declares which arms run at which scales against which label
sets. Preview before committing to a long run:

```bash
./target/release/mosaica-bench matrix --plan
```

It reports **cell counts**, which is the number that matters — the obvious every-axis matrix is
~27,000 cells; the shipped one is ~6,900. Then:

```bash
./target/release/mosaica-bench matrix --run-dir /tmp/mosaica-bench/runs/$(date +%F)
./target/release/mosaica-bench matrix --only gather --run-dir ...   # one arm
```

Resumable at cell granularity: re-invoking the same command skips what the run directory's ledger
records as done. A failing arm doesn't abort the campaign, and only successes are recorded, so a
re-run picks up exactly what's missing.

**To change the campaign**, edit `matrix.toml`. Empty `scales` or `label_sets` means every fixture
discovered. Per-arm `repeat` and `seed` override `[defaults]`. The file explains why it's trimmed
the way it is — the short version is that grants are free to vary within a bundle and label sets
are not, so the label-set axis only earns its cost where signature-sorted contiguity is under test.

> Keep `hash-flat` in at least one arm. It is gate G0b's control: its run ratio must come back at
> exactly 1.00 or the estimator is broken and no other ratio in the run is interpretable.

---

## 5. Concurrency, and blank-database ingest

Two arms don't fit the matrix.

**`load`** needs a booted server, N authorised sessions, and server-side RSS/CPU sampling, so it's
driven from Python:

```bash
reference/.venv/bin/python scripts/bench_concurrency.py \
    --bundle /tmp/mosaica-bench/fixtures/2422486/categories-subclass \
    --scale 2422486 --concurrency 5,10,100,1000 --arms B,A --duration 8 --w 10
```

`--arms B` shares fragments across users (request-path contention); `--arms A` gives each user its
own grant set (per-session memory, and the F4 lock). It calibrates against `/healthz` first and
flags any cell within 3× of the generator's own ceiling.

**`ingest-build`** is minutes per cell and writes a whole bundle to temp:

```bash
./target/release/mosaica-bench ingest-build \
    --scale 250000,2422486,25000000 --label-set categories-subclass \
    --data-root . --run-dir /tmp/run1
```

---

## 6. Adding a label set

The label set decides entity-ID assignment through the signature sort, so a new one needs a new
bundle — it cannot be synthesised from an existing one at query time.

1. Generate `data/scaled/pairs/<name>.pairs.parquet` with columns `(entity_id, term_id)`.
2. `./scripts/bench_build_fixtures.sh --label-sets <name>`
3. Add it to `matrix.toml`, or pass `--label-set <name>` directly.

If it has an entity cap, add it to `CAP` in `bench_build_fixtures.sh` so out-of-range scales are
skipped rather than silently built with a hole.

**What you don't need a new label set for:** grant width, coverage, scatter, or posting shape.
Those are all constructible from grants within any existing bundle, for free — that's what
`--w`, `--shape` and `--coverage` sweep. The only thing a rebuild buys is a different entity-ID
assignment.

---

## 7. Collate and gate

```bash
# First run: record the baseline
reference/.venv/bin/python scripts/bench_collate.py --run-dir /tmp/run1 --tag tier1 --update

# Later runs: compare, and fail on regression
reference/.venv/bin/python scripts/bench_collate.py --run-dir /tmp/run1 --tag tier1 --gates
```

Exit codes: `0` pass, `1` a gate failed, `2` the run itself is invalid (G0/G0b). Baselines land in
`docs/archive/plans/bench-baselines/`.

Gates, most authoritative first:

| gate | checks | on failure |
|---|---|---|
| **G0** | container counts and Σvisible identical to baseline | **the run is void** — the corpus or grant construction moved, so no latency comparison means anything |
| **G0b** | `hash-flat` run ratio == 1.00 | the estimator is broken; abort before reporting |
| **G1** | normalised costs (ns per row-visible / point / tile / container) within +15% | regression |
| **G2** | battery p99−p50 within +15% | a per-request stall, with workload variance removed |
| **G3** | each stage's share of total within 10 points | one stage got slower while another got faster |

Raw random-sweep p99 is emitted for a dashboard but is **never** a gate: the
tail-attribution memo showed it mostly
measures the input distribution, not the system.

---

## 8. Reading the output

One JSONL record per cell, per arm, under the run directory. Every record carries `work` (container
counts, Σvisible, coverage, run ratio, pages touched) beside `timing` — deliberately, because the
cost model is O(containers touched) and a latency without a container count can't be told apart
from a workload that simply moved.

`min_ns` is the headline, following Phase 0: *"a single-shot first pass reported figures 5–10×
higher and was noise — repeat before believing."*

Flags worth knowing:

| flag | meaning |
|---|---|
| `degenerate` | coverage ≈ 100% — measures the *absence* of masking; never compare against a real principal |
| `low_container_resolution` | fewer than 32 containers, so `ns_per_container` is meaningless. A corpus of 250k spans **4** containers total; 2.42M spans 37; 25M spans 382. Excluded from G1 |
| `clamped_w=N` | the requested grant width exceeded the vocabulary. At 2.42M `categories-subclass` there are only 176 terms, so `w=1000` and `w=10000` both measure `w=176` |
| `selection_overdraw=Nx` | selection materialised N× more rows than it returned — finding F1 |
| `generator_bound` | throughput within 3× of the load generator's own ceiling; a floor, not a measurement |
| `major_faults` | the cell read from disk, so its latency is a page-cache artefact |
| `single_repetition` | a build, timed once — min-of-N is unaffordable at minutes per cell |

With `bench-timing` (on by default in `mosaica-bench`), viewport records also carry `stages`: a
per-stage nanosecond breakdown plus `clock_overhead_ns`, the perturbation the instrumentation
itself introduced. Subtract it rather than assuming it's zero.

**Since the intra-request rayon tile sweep (D-D/D-F), `stages`' per-tile fields
(`count_ns`/`select_ns`/`gather_ns`/`underlay_ns` and the row counters beside them) stop
partitioning wall clock the moment `EngineConfig::compute_threads > 1`.** They become
cross-worker CPU-time sums, so their total can legitimately exceed the request's own wall time —
`unattributed_ns` correspondingly floors at zero rather than reporting idle time. Comparing a
`compute_threads = 1` cell against a `compute_threads > 1` cell on these fields is comparing CPU
time against CPU time, which is a meaningful comparison for *cost*, but not for *latency* — read
`min_ns`/wall clock for latency, and the per-stage breakdown for where CPU time went. See
`mosaica_engine::timing`'s module doc for the full reasoning; nothing above this paragraph
changed.

**That cross-worker-sum behaviour only applies above the calibration serial fallback.** Below
`SERIAL_FALLBACK_MAX_ROWS` (200,000 rows-in-ranges), `Engine::viewport` folds the tile sweep
serially regardless of `compute_threads` — the `pool.install` fan-out never runs for those cells —
so for a request below that line, the per-tile stage fields still partition the request's own
wall clock exactly as before the intra-request rayon change, at ANY thread count. Read
`env`/`work` in the record (or the request's own row-range total) to tell which regime a cell fell
into before treating its per-tile fields as either a wall-clock partition or a CPU-time sum.

---

## 9. Things that will bite you

**Everything is WSL2 on a shared 12-core box.** The benchmark report is the standing instruction:
*"treat ratios as evidence and absolutes as a starting point."* Check `env.load_avg` in the records
— a fixture build running in another terminal contaminates everything.

**The container axis has little range at these scales.** A Roaring container holds 2¹⁶ entities, so
the whole corpus spans 4 containers at 250k and 382 at 25M, against 15,259 at 10⁹. Phase 0's cost
model was fitted across 399 → 1.5M containers. Below 32 the normalisation is flagged and excluded.

**A uniform-random viewport usually contains nothing.** The geometry is UMAP output and heavily
concentrated. The `battery` mode probes the corpus for density deciles before measuring; an
unprobed random battery measures the cost of finding nothing, ten times.

**Ingested rows never become visible.** Phase 1 has no flush, so a buffered entity has no row in
the segment and `compose` skips it. `ingest-continuous` measures composition over *rejected*
entries. The `changes` arm measures resolved ones, which is why the two together bracket F2 rather
than duplicating each other.

**`ingest-build` at the smallest scale is contaminated if it runs first** — it reads the 6.6 GB
`geometry.parquet` cold while later builds hit the page cache. Compare adjacent scales, not the
first row.

---

## 10. The nightly comparison

[`scripts/bench_ab.py`](../scripts/bench_ab.py) needs no corpus and no fixtures. It writes a
profile's corpus from a seed and runs two `mosaica` binaries over it through the CLI and the HTTP
API: build, start, a battery of viewport, aggregate and bulk-read requests by a principal who sees
everything and one who sees one term, an ingest of 5% more items with a flush, deletion of 1% and
suppression of 0.1%, and a compaction. Each step's result is checked by the visible count before
its time is kept. The binaries alternate, baseline first on even repetitions, so drift in the
machine falls on both.

```bash
cargo build --release -p mosaica && cp target/release/mosaica /tmp/candidate
python3 scripts/bench_ab.py run --profile fields --baseline /tmp/baseline --candidate /tmp/candidate \
    --work /tmp/ab --out ab-fields.json
python3 scripts/bench_ab.py report ab-*.json
```

| profile | items | what it varies |
|---|---|---|
| `fields` | 5,000,000 | every attribute type, each rendered, indexed, both and neither |
| `scatter` | 100,000,000 | labels hashed over 1,024 terms: the least contiguous a term can be |
| `clustered` | 100,000,000 | a Web Mercator view, labels by spatial cluster: the most contiguous |
| `text` | 10,000,000 | an indexed text column of about 24 words, and an indexed keyword |
| `layers` | 50,000,000 | a three-level tiered layer and a flat layer over a category |

The sizes are what fits a standard GitHub runner with room to spare. At 100,000,000 items one run
of one binary took 400 s on four cores, with the build at 4.3 GiB resident, the server at 8.4 GiB
including the bundle pages it maps, and 16 GB of disc. `--items` changes a profile's size. The
Python it needs is in [`scripts/bench_ab.requirements.txt`](../scripts/bench_ab.requirements.txt).

[`.github/workflows/nightly.yml`](../.github/workflows/nightly.yml) runs every profile each
evening, comparing main with main as it stood 24 hours earlier, and puts the tables in the run's
summary; a manual run takes any baseline. Each binary runs five times. A row is marked slower or
faster when the ratio of the two sides' minima is past 15%, their five figures do not overlap, and
for a timing they differ by more than 2 ms. A slower row turns the run red. A `cold` row, the first
answer after a session is minted, is one answer per run and differed by more than twice between
runs of one binary, so it is shown without a verdict.

Two binaries that do not differ still separate completely on one row in 252, by chance, and a
night has hundreds of rows, so the 15% and 2 ms conditions are what keep a red run meaningful.
They were set from one repetition of a binary run against itself on a 4-core machine: filtered
viewports differed by up to 20%, build and compaction by under 5%, and ingest by up to 50% on the
`fields` profile. How often a hosted runner raises a false alarm under these conditions has not
been measured, so rerun a red row with the same baseline before acting on it. Hosted runners
differ from one another by more than any of these figures, so only the ratio, taken on one
machine, means anything, and the absolutes belong to that runner.

---

## 11. Capacity: many users as memory shrinks

[`scripts/bench_capacity.py`](../scripts/bench_capacity.py) measures one box serving many concurrent
users while the ratio of bundle bytes to memory rises, from a bundle held whole in memory to one
many times larger than the memory the server may use. A profile in
[`capacity.toml`](capacity.toml) names the corpora and the matrix: memory cap, cache policy, grant
arm, number of users and think time. The script builds any corpus it lacks with `bench_ab.py
corpus` and keeps it, so a second run only serves.

```bash
python3 scripts/bench_capacity.py run --profile smoke --data /data/bench --run-dir /data/runs/smoke
python3 scripts/bench_capacity.py report /data/runs/smoke
```

Each cell boots `mosaica serve` inside its own memory cgroup
([`scripts/bench_cgroup.py`](../scripts/bench_cgroup.py)), using cgroup v2 where the host has it, then
cgroup v1, then a `systemd-run` scope. The page cache the server reads the bundle through is charged
to the cap, so `bundle bytes / cap` is the disk-to-memory ratio the cell runs at. Creating a group
needs root or a delegated subtree. A server the cap kills is recorded with status `oom`, not
treated as a failure of the run.

A cell runs two windows of the load generator, a cold one straight after the boot and an eviction
of the bundle from the page cache, then a warm one, and samples the cgroup's charge, anonymous
memory, page cache and major faults once a second throughout. The generator draws each user's
next request from the profile's mix of panning close-ups, filtered close-ups, overviews, aggregates
and item pages (`mosaica-bench load --mix`; the format is in `arms/load.rs`). A user given a think
time pauses for an exponentially distributed time of that mean between requests, so that a cell
of thousands of users models people rather than saturating loops.

| axis | values | what it isolates |
|---|---|---|
| cap | sizes, or `unbounded` | the disk-to-memory ratio |
| cache policy | `default`, `tuned` | `default` keeps the server's cache sizes, about 8 GiB in all, and shows where they stop fitting; `tuned` sets each to a fraction of the cap |
| arm | `A`, `B` | `A` gives each user its own grant, so the memory a session costs; `B` shares four grants, so the request path's throughput |
| users, think time | counts, ms | load as concurrent loops, or as people |

The admission gate sheds what the server cannot take with a 429, which a closed-loop user retries at
once, so at high user counts with no think time the record's `shed_rate` is large while throughput
counts only answered requests. The generator's own ceiling against `/healthz` is measured first,
and a cell within a third of it is flagged `generator_bound`. The `small` profile pins the server
to three cores and the generator to the fourth, which suits a 4-core box. On a larger box, use the
`large` profile, change `caps` and `corpora` to its memory and disc, and give the generator enough
cores or run it on another machine. A `scatter` bundle is about 62 bytes an item.
