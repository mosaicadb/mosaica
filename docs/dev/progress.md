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

A `mosaica_types::progress::Tracker` holds the job's current step. The workers of a step call `advance(n)` once per batch, which is one relaxed atomic add. A pass that meets its units one at a time keeps its count in a local `Batched`, which calls `advance` once per `ADVANCE_EVERY` (65,536) units. A step whose total is known only after it has begun sets it with `count`, which keeps the step's clock. Totals that take work to find, such as the row counts in the Parquet footers or the sizes of the bundle's files, are read only when the job has a tracker. A reader calls `snapshot()` when it wants a record, which takes a lock and reads `/proc/self/status`, so a job with no reader pays only the adds. The build's renderer reads four times a second in `bar` mode and once a second otherwise, and the server reads when `/control/status` is requested.

## Jobs

### Build

The build's 19 stages fall into five phases. A stage's phase is given by a `match` with no wildcard, so a new stage does not compile until it has one.

| Phase | Stages | What a step counts |
|---|---|---|
| 1. Reading sources | `source_ids`, `dictionary`, `pairs_pack`, `geometry_read` | Rows, with totals read from the Parquet footers before a row is read. `source_ids` is one step per file it reads, named in `context`, and counts the rows of the row groups it keeps. `dictionary` and `pairs_pack` count the access relation's rows, which are the views' points when each view carries its own access lists. `geometry_read` counts every view's points. |
| 2. Assigning ids | `signature_sort`, `assignment` | Nothing in the sort. `assignment` counts items. Both run once per batch when the build does not fit in memory, and `context` names the batch. The work after the last batch is one more `assignment` step with no context. |
| 3. Indexing records | `postings_write`, `attribute_tail`, `layers`, `unique_indexes`, `filter_postings`, `text_index`, `record_blob`, `column_release` | Pairs for `postings_write`. Items for each indexed column in `filter_postings`, and documents tokenised for each text column in `text_index`, with the column in `context`. A view group's own indexed columns are `filter_postings` and `text_index` steps too, with the view and the column in `context`, although they run during `attribute_tail`. The other stages count nothing. |
| 4. Laying out views | `tiler_sort`, `segment_write`, `artifact_pass`, `term_images` | Rows for `segment_write`, terms for `term_images`, nothing for the sort or `artifact_pass`. Runs once per view, and `context` names the view. `term_images` includes the view's field tallies. The containment partitions are composed by one more `artifact_pass` step with no context, after the last view. |
| 5. Verifying the bundle | `manifests` | Bytes digested, out of the bundle's size. |

`filter_postings` and `text_index` share one loop over the declared columns, so each column is its own step, and the record shows them in the order they run. A stage that runs once per shard as well, such as `segment_write` or a column's `filter_postings`, names the shard in `context` when the bundle has more than one, so that two consecutive steps always differ. A context reads like `view s0 (1 of 2), shard (2 of 3)`: a part that is the only one of its kind has no position, and an unnamed part such as a batch or a shard is left out when it is the only one. A part of a source that does not say how many it has reads like `page (37)`. Every job builds its context with `mosaica_types::progress::context`. Where a stage has work of its own after its parts, such as `filter_postings` after its columns, its step is opened again with no context.

`StageTimer` opens each stage's step and times the stage, so for any second of the build the step in the record and the stage that `--stage-timings` charges are the same, and the stages' times sum to the build's wall time. `--stage-timings` and `--stage-timings-json` still report each stage's total when it ends. Byte figures in the build's reports are in binary units, as the progress record's are.

### Compaction

A compaction rewrites one shard at a time, and one compaction runs at a time. `context` names the shard, such as `shard 3`.

| Phase | Steps |
|---|---|
| 1. Rewriting rows | `row_space` |
| 2. Indexing records | `postings`, `term_images`, `unique_indexes`, `edited_items`, `attributes`, `entity_terms`, `field_tallies` |
| 3. Verifying | `digests` |
| 4. Publishing | `hand_off`, `memberships`, `derived`, `report`, `manifest`, `flip`, `retire`, `open`, `adopt`, `warm`, `wal`, `reclaim` |

The steps of phases 1 to 3 are the passes of `compact::execute` on the compaction's own thread, and the steps of phase 4 are the parts of `publish_fold` on the write executor. The `Staircase` that times each pass for `compaction.passes` opens its step through `mosaica_types::progress::Stages`, so a pass's cost row and its step are one boundary, and the error of a pass that fails names its step. `row_space` counts the rows it reads, `postings` the terms of the dictionary and `digests` the bytes it digests. The other steps count nothing.

A compaction's record is listed from its dispatch until its publication ends, and it is failed where the compaction fails on its thread or is discarded at publication.

### Flush

A flush has one phase, Flushing, whose steps are the stages of its execution on the worker pool, from `promote` to `drop_plan`. The flush laps that time those stages open the steps through `Stages`. The `rows` step counts the rows it writes out of the rows the flush took from the buffer. The other steps count nothing, because a flush lasts seconds and its record exists so that a reader of `/control/status` can see that one is running and for how long.

A flush's record is listed for the whole time the flush is in flight, from its dispatch to its publication, and it is failed where the flush fails or is discarded at publication. Not built yet: the flush's publication on the write executor is timed by a separate mechanism and has no steps, so while the flush publishes its record shows `drop_plan`, the last of its pool stages, and while it waits for a worker it shows `promote`, the first. `flush --wait` holds its request until the flush is visible and shows no progress while it waits.

### Ingest

The client that sends an ingest makes its record, because the server sees one request at a time and only the client knows how many it will send.

| Phase | Step | What it counts |
|---|---|---|
| 1. Sending | `pages` | Rows sent, out of the rows in the Parquet file's footer. `context` names the page, such as `page (37 of 120)`. An Arrow IPC or NDJSON file has no total, so its pages are not counted and `context` reads `page (37)`. |
| 2. Waiting for publication | `publication` | Nothing. Only with `--wait`, so an insert without it has one phase. |

## Surfaces

### HTTP

`GET /control/status` has a `jobs` array that holds the record of each flush and compaction in flight, and an empty array when none is. `compaction.last_job` holds the record of the last compaction to end, finished or failed, or `null` before the first, and each entry of `compaction.passes` carries its step's final `done` and `total`. The records are served only on the control plane, because a compaction's counts are totals over the whole corpus rather than counts within one viewer's visible set.

`compaction.requests.last_answer` says how the server answered the latest compaction request: the range of request numbers it covers, `first` to `last`, since requests made before a tick takes them or while an earlier one is answered are answered together, and its `outcome`, which is `compacted`, `nothing_to_do`, `refused` with the `gate` and its `detail`, or `failed`. The server decides the outcome, so every client reads the same answer.

A client that waits for a compaction polls `/control/status` until `compaction.requests.answered` reaches its request number, and shows the compaction's record from `jobs` while it waits. It then reads the outcome from `last_answer`, and the request has failed unless the outcome is `compacted` or `nothing_to_do`. Where `accepted` is below its request number the server restarted and lost the request, and where `answered` has reached the number but `last_answer` begins after it, a later answer has replaced its outcome, which is then not known. Where the outcome is `compacted` or `failed` the client ends with `compaction.last_job` as served, because a compaction shorter than the polling interval is never seen in `jobs`, and a failed compaction's record says which step it stopped in.

### CLI

`mosaica build --progress <MODE>` takes one of the following modes.

| Mode | Output on stderr |
|---|---|
| `auto` | `bar` when stderr is a terminal, and `plain` otherwise. The default. |
| `bar` | Two lines redrawn in place: the phase, then the step with its count, rate, time remaining, elapsed time and resident memory. Line wrapping is off while the bar draws, so a line wider than the terminal is cut at its edge. Other output on stderr is printed over the bar, and text of the bar can remain beside a line shorter than it. |
| `plain` | A line when a step begins, a line with its final count when it ends, and a line every 30 seconds while it runs. Times are UTC. No terminal control codes. |
| `json` | One `Progress` object per line, when a step begins, when it ends and at most once a second while it runs, and a last object with `state` `finished` or `failed`. |
| `none` | Nothing. |

A `plain` line reads as follows.

```
12:04:31Z [3/5 Indexing records] filter_postings column category (3 of 7)  41.2M/74.0M items  1.83M/s  ~18s  7m12s  rss 18.2 GiB
```

`mosaica compact --wait` takes `--progress` and shows the compaction's record from `/control/status` until the request is answered, and exits non-zero unless the server answered it as `compacted` or `nothing_to_do`. `mosaica insert` takes `--progress` and shows its ingest record. `mosaica status` prints the status as JSON, so its `jobs` array holds the running jobs.

### Python

`Database.commit(progress=...)` takes `True` to show progress, `False` to show none, or a callable that receives each record as a `dict`. The default is `True`. The first commit runs `mosaica build --progress json`, reads the records from its stderr as they arrive and keeps every other line for the commit's log. A later commit makes the ingest record itself as it sends its pages, counting the rows of each page answered out of the rows of every page its plan sends, so its step `pages` always has a total. `Control.compact(wait=True, progress=...)` polls `/control/status` and shows the compaction's record until the request is answered, and returns the `202` with `outcome`, `last_answer` and `last_job` added to its body. `outcome` is the served outcome, or `lost` where the server restarted and `unknown` where a later answer replaced the request's, because `Control` returns every refusal in its `Answer` without raising. `Database.compact(wait=True, progress=...)` raises `Refusal` unless the outcome is `compacted` or `nothing_to_do`, as `mosaica compact --wait` exits non-zero. Progress is drawn with `tqdm.auto` where `tqdm` is installed, which is a widget in a notebook, and as a line on stderr where it is not, when a step begins and ends and at most every 30 seconds between.

### TypeScript

`@mosaica/core` exports the `Progress` type, types the `jobs` array of the status answer, and takes `wait` and `onProgress` on `compact`.

## Not covered

Not built yet: merges and coalesces of segments have no record. Each runs in seconds on the write executor, and `/control/status` reports their counts and totals when they end.
