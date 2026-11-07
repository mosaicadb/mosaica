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

`GET /control/compact/{request}` says where one compaction request is, on the same credential as `/control/status`. Its `answered` is `false` while the request is pending, and its `job` is then the record of the compaction in flight, where one is. Once the request is answered, `answer` holds the `outcome`, the range of requests `first` to `last` answered with it and, for `refused`, the `gate` and its `detail`, and `job` holds the last record of the compaction that answered it, finished or failed, or `null` where none ran. Requests made before a tick takes them, or while an earlier one is answered, are answered together and share one answer, which the server keeps for as long as it runs. The outcome is `refused` where a shard the requests named was refused, otherwise `failed` where a compaction they started failed, was discarded or panicked, otherwise `compacted` where one was published, and `nothing_to_do` where no shard had anything to compact. A number the server has not given since it started is `404`, because numbering starts again at 1 after a restart and a request made before one is lost.

A client that waits for a compaction reads the route about once a second until `answered` is `true`, showing `job` while it waits, then shows the answered `job` and takes the outcome: the request succeeded where it is `compacted` or `nothing_to_do`, and a `404` means the request was lost.

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

`mosaica compact --wait` takes `--progress` and waits as the HTTP section describes, showing the compaction's record, and exits non-zero when the request was refused, a compaction for it failed, or the server lost it. `mosaica insert` takes `--progress` and shows its ingest record. `mosaica status` prints the status as JSON, so its `jobs` array holds the running jobs.

### Python

`Database.commit(progress=...)` and `compact(wait=True, progress=...)` on `Database` and on `Control` take `True` to draw progress with `tqdm.auto`, which is a widget in a notebook and a bar on stderr in a terminal, `False` to draw none, or a function that receives each record as a `dict`. The default is `True`, and `tqdm` is a dependency of the package. The first commit runs `mosaica build --progress json`, gives the sink each line of its stderr that is a record as it arrives, and keeps every other line for the commit's log. A later commit makes the ingest record itself from its plan, so the step `pages` counts the rows of each page answered out of the rows of every page the plan sends. `Control.compact(wait=True)` waits as the HTTP section describes and returns the `202` as the server sent it, with how the wait ended in the answer's `waited`, because `Control` returns every refusal in its `Answer`. `Database.compact(wait=True)` raises `Refusal` where the request did not succeed, with that result as the refusal's `report`, as `mosaica compact --wait` exits non-zero.

### TypeScript

`@mosaicajs/client` exports `Progress` with the types of its fields, and `Control.status` types the status answer's `jobs`, `compaction.last_job` and `compaction.requests`, leaving the rest of the status as `unknown`. `Control.compact({wait: true, onProgress})` polls `/control/status` about once a second, calls `onProgress` with the compaction's record from `jobs` at each reading that lists one and with `compaction.last_job` as served at the end where the outcome is `compacted` or `failed`, and resolves with the `202` as the server sent it and with `waited`, which says how the wait ended. Where the request was answered, `waited` holds `last_answer` and `last_job` as served. It says `lost` where the server restarted, `unknown` where a later answer replaced the request's, and `unread` with the answer to a reading of `/control/status` that was refused. `waited.ok` is `true` only where the outcome is `compacted` or `nothing_to_do`. The body of the `202` is left as it is because the body of an `Answer` is always the route's response, and no outcome is thrown because `Control` returns every refusal in its `Answer`.

## Not covered

Not built yet: merges and coalesces of segments have no record. Each runs in seconds on the write executor, and `/control/status` reports their counts and totals when they end.
