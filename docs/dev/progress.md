# Progress of long-running jobs

A build of a large corpus runs for hours, and a compaction or a flush after a large ingest runs for minutes. This document describes how each of those jobs says how far it has gone, in one record that every surface serves: `mosaica build` and the other CLI commands, `/control/status`, the Python client and the TypeScript client.

The readers are the operator who runs a database, in a terminal or in a log collected by CI, Kubernetes or systemd, and the Python user who calls `Database.commit()` in a notebook.

## The record

A job is a fixed list of **phases**, named for a person. Each phase runs one or more **steps**, named as the code names them, and a step that processes a known number of things counts them. `mosaica_types::progress::Progress` is the record, and it serialises as follows.

```json
{"job": "build", "state": "running",
 "phase": {"index": 3, "of": 5, "name": "Indexing records"},
 "step": "filter_postings", "context": "column category (3 of 7)",
 "done": 41200000, "total": 74000000, "unit": "items",
 "rate": 1830000.0, "eta_s": 17.4,
 "elapsed_s": 412.0, "step_elapsed_s": 22.5, "rss_bytes": 19500000000}
```

| Field | Meaning |
|---|---|
| `job` | `build`, `ingest`, `flush` or `compaction`. |
| `state` | `running`, `finished` or `failed`. A failed job's record keeps the step it stopped in. |
| `phase` | Position among the job's phases, counted from 1, and the phase's name. |
| `step` | The step's stable name. For a build it is the stage name that `--stage-timings` prints. |
| `context` | Which part of the step is running, such as a view, a batch, a column or a shard. Absent when the step has one part. |
| `done`, `total`, `unit` | The step's count. All three are absent for a step that counts nothing, such as a sort. |
| `rate` | Units per second, measured over the last 20 seconds of the step. |
| `eta_s` | Seconds until `done` reaches `total` at `rate`. |
| `elapsed_s`, `step_elapsed_s` | Wall time since the job and the step began. |
| `rss_bytes` | The process's resident memory when the record was taken. A build's limit is `--memory-budget`. |

`eta_s` is the only estimate the record carries, and it covers the current step only. The steps of one job scale differently, since the sorts are n log n, the digest is linear in bytes and several passes depend on the size of the source files rather than on the item count, so a time remaining for the whole job would be a model fitted to other corpora. A step with no count has no `eta_s`, and neither does a step younger than two seconds, because its first batches measure start-up rather than throughput. A reader that polls less often than every 20 seconds gets the step's average rate since it began.

### Cost

A `mosaica_types::progress::Tracker` holds the job's current step. The workers of a step call `advance(n)` once per batch, which is one relaxed atomic add, and never once per unit. A reader calls `snapshot()` when it wants a record, which takes a lock and reads `/proc/self/status`, so a job with no reader pays only the adds. The build's renderer reads once a second, and the server reads when `/control/status` is requested.

## Jobs

### Build

The build's 19 stages fall into five phases. A stage's phase is given by a `match` with no wildcard, so a new stage does not compile until it has one.

| Phase | Stages | What a step counts |
|---|---|---|
| 1. Reading sources | `source_ids`, `dictionary`, `pairs_pack`, `geometry_read` | Rows of the source file. The total is read from the Parquet footer before a row is read. |
| 2. Assigning ids | `signature_sort`, `assignment` | Nothing in the sort. `assignment` counts items. Both run once per batch when the build does not fit in memory, and `context` names the batch. |
| 3. Indexing records | `postings_write`, `attribute_tail`, `layers`, `unique_indexes`, `filter_postings`, `text_index`, `record_blob`, `column_release` | Items, per column for `filter_postings` and `text_index`, with the column in `context`. |
| 4. Laying out views | `tiler_sort`, `segment_write`, `artifact_pass`, `term_images` | Rows for the writes, nothing for the sort, terms for `term_images`. Runs once per view, and `context` names the view. |
| 5. Verifying the bundle | `manifests` | Bytes digested, out of the bundle's size. |

`filter_postings` and `text_index` share one loop over the declared columns, so each column is its own step, and the record shows them in the order they run. `--stage-timings` and `--stage-timings-json` are unchanged and still report each stage's total when it ends.

### Compaction

A compaction rewrites one shard at a time, and one compaction runs at a time. `context` names the shard.

| Phase | Steps |
|---|---|
| 1. Rewriting rows | `row_space` |
| 2. Indexing records | `postings`, `term_images`, `unique_indexes`, `edited_items`, `attributes`, `entity_terms`, `field_tallies` |
| 3. Verifying | `digests` |
| 4. Publishing | `publish` |

The steps are the passes of `compact::execute`, which already checks between passes whether the engine has closed. A pass that loops over the shard's rows or terms counts them.

### Flush

A flush has one phase, Flushing, whose steps are the flush's stages from planning to publication. The `rows` step counts the rows it writes out of the rows the flush took from the buffer. The other steps count nothing, because a flush lasts seconds and its record exists so that a reader of `/control/status` can see that one is running and for how long. `flush --wait` holds its request until the flush is visible and shows no progress while it waits.

### Ingest

The client that sends an ingest makes its record, because the server sees one request at a time and only the client knows how many it will send.

| Phase | Step | What it counts |
|---|---|---|
| 1. Sending | `pages` | Rows sent, out of the rows planned. `context` names the page, such as `page 37 of 120`. |
| 2. Waiting for publication | `publication` | Nothing. `context` names the publication number that is awaited. |

## Surfaces

### HTTP

`GET /control/status` has a `jobs` array that holds the record of each running flush and compaction, and an empty array when none is running. The records are served only on the control plane, because a compaction's counts are totals over the whole corpus rather than counts within one viewer's visible set. A client that waits for a compaction polls `/control/status` until `compaction.requests.answered` reaches its request number, and shows the compaction's record while it waits.

### CLI

`mosaica build --progress <MODE>` takes one of the following modes.

| Mode | Output on stderr |
|---|---|
| `auto` | `bar` when stderr is a terminal, and `plain` otherwise. The default. |
| `bar` | Two lines redrawn in place: the phase, then the step with its count, rate, time remaining, elapsed time and resident memory. |
| `plain` | A line when a step begins or ends, and a line every 30 seconds while a step runs. No terminal control codes. |
| `json` | One `Progress` object per line, when a step begins and at most once a second while it runs, and a last object with `state` `finished` or `failed`. |
| `none` | Nothing. |

A `plain` line reads as follows.

```
12:04:31 [3/5 Indexing records] filter_postings column category (3 of 7)  41.2M/74.0M items  1.83M/s  ~18s  7m12s  rss 18.2 GiB
```

`mosaica compact --wait` takes `--progress` and shows the compaction's record from `/control/status` until the request is answered. `mosaica insert` takes `--progress` and shows its ingest record. `mosaica status` prints each running job in the `plain` form.

### Python

`Database.commit(progress=...)` takes `True` to show progress, `False` to show none, or a callable that receives each record as a `dict`. The default is `True`. The first commit runs `mosaica build --progress json`, reads the records from its stderr as they arrive and keeps every other line for the commit's log. A later commit makes the ingest record itself as it sends its pages. `Control.compact(wait=True, progress=...)` polls `/control/status` and shows the compaction's record until the request is answered. Progress is drawn with `tqdm` where it is installed, and as a line on stderr where it is not.

### TypeScript

`@mosaica/core` exports the `Progress` type, types the `jobs` array of the status answer, and takes `wait` and `onProgress` on `compact`.

## Not covered

Not built yet: merges and coalesces of segments have no record. Each runs in seconds on the write executor, and `/control/status` reports their counts and totals when they end.
