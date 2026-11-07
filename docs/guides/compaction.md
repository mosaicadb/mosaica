# Schedule compactions

Deleted and edited items stay in the bundle, hidden from every viewer, until a compaction removes them. Compaction also merges the segments that new data adds and frees the disc space that merging leaves behind. The server compacts on its own, mostly in a nightly window, so the main thing to decide is when that window falls. This page explains what starts a compaction, what it needs, and how to start one yourself and check it worked.

The examples use the [systemd install](systemd.md), whose control address is a Unix socket. Under Docker, write `--control http://127.0.0.1:9163` instead.

## When the server compacts

Each time the server makes new items visible, it adds a segment to the bundle, and it merges small segments as they build up. A compaction rewrites the bundle as one segment per view, without the rows that were deleted or replaced. The server starts one when any of these holds:

- during a four-hour window from midnight UTC, a view has 8 segments or more;
- at any hour, a view has 64 segments;
- 500,000 deleted items and annotations are waiting to be removed, or whatever `overlay_soft_limit` is set to;
- deleted items are a fifth or more of all rows;
- files the bundle no longer uses take as much space as the files it does.

Set `compaction_window_start` under `[ingest]` to the start of your quietest hours, in UTC as `"HH:MM"`. Once a compaction starts, the schedule starts no other compaction of that shard for a day (`compaction_min_interval_secs`), and never two at once. A compaction you ask for yourself doesn't wait for the interval. The [mosaica.toml reference](../reference/mosaica-toml.md) lists the key for each condition, and writing `"off"` for one turns it off and leaves the others.

Viewers are answered as usual while a compaction runs. When it finishes, every session's cached view of the map is out of date at once. A background pass rebuilds them, and a session it hasn't reached yet pays for its own on its next request, so expect some slower requests straight after a compaction.

??? info "Details of the schedule"
    - The server keeps the time of the last compaction in memory, so after a restart the first condition that holds starts one straight away.
    - A compaction that was refused doesn't count towards the interval.
    - A bundle of more than one shard (`[shards] size` in the schema) is compacted a shard at a time. Each shard is checked against the conditions on its own counts, and the interval applies to each shard on its own.

## What a compaction needs

A compaction needs free space on the bundle's disc of one and a half times the size of the files the bundle uses, not counting the ones it has left behind, and enough memory for its own estimate, within the cap you set. Without either it is refused, and `/control/status` records why under `compaction.last_refusal`. In a bundle of several shards, the rule applies to the shard being compacted, so plan for the largest. [Disc](sizing.md#disc) explains how to plan disc space for it.

## Start one yourself

`mosaica compact` asks for a compaction now, whatever the schedule says, and with `--wait` it stays until the server has answered the request. It needs a credential with `write`, and with `--wait` also `admin`, as the operator credential has, read from `MOSAICA_CREDENTIAL`. Here a deleted place, Dublin, is waiting to be removed:

```console
mosaica$ export MOSAICA_CREDENTIAL=$(cat secrets/operator.secret)
mosaica$ mosaica compact --wait --progress plain --control unix:/run/mosaica/control.sock
{"request":1}
14:33:32Z [1/4 Rewriting rows] row_space shard 0  0/150k rows  0s  rss 94.0 MiB
14:33:33Z [2/4 Indexing records] attributes shard 0  1s  rss 102 MiB
14:33:35Z [4/4 Publishing] derived shard 0  3s  rss 103 MiB
14:33:43Z [4/4 Publishing] reclaim shard 0  11s  rss 102 MiB  finished
```

The first line, on stdout, is the request's number, counted from 1 each time the server starts. The lines after it, on stderr, follow the compaction through its four phases, naming the shard it is rewriting, and the last says how it ended. In a terminal `--progress` draws a bar in their place, `json` prints each record as a JSON object, and `none` prints nothing. The server compacts every shard with something to reclaim, one after another, and a request made while a compaction is running waits for it to finish. Without `--wait` the command prints the request's number and returns at once.

`mosaica compact --wait` exits 0 when the request was compacted or had nothing to compact. It exits 1 when a shard was refused, with a line saying which gate refused it and what to do, when a compaction for the request failed, and when the server restarted while it waited, which loses the request. Over HTTP the request is `POST /control/compact`, and the Python and TypeScript clients have `compact`.

## Read how a request went

`GET /control/compact/{request}` says where a request is. It needs `admin`, and over the unix socket `curl` reaches it like this:

```console
mosaica$ curl --silent --unix-socket /run/mosaica/control.sock -H "Authorization: Bearer $MOSAICA_CREDENTIAL" http://localhost/control/compact/1 | jq
{
  "answer": {
    "detail": null,
    "first": 1,
    "gate": null,
    "last": 1,
    "outcome": "compacted"
  },
  "answered": true,
  "job": {
    "context": "shard 0",
    "elapsed_s": 10.850382514,
    "job": "compaction",
    "phase": {
      "index": 4,
      "name": "Publishing",
      "of": 4
    },
    "rss_bytes": 106913792,
    "state": "finished",
    "step": "reclaim",
    "step_elapsed_s": 0.272100022
  },
  "request": 1
}
```

While the request waits, `answered` is `false`, `answer` is `null`, and `job` is the progress of the compaction running, if one is. Once it is answered, `job` is the last record of the compaction that answered it, and `answer.outcome` is one of these:

- `compacted`: a compaction for the request was published and the deleted rows are gone.
- `nothing_to_do`: no shard the request named had anything to compact.
- `refused`: a shard was refused. `gate` names why, such as `insufficient_disc`, and `detail` says what to do.
- `failed`: a compaction for the request failed or was discarded. `job` is then the failed record, and its `step` names where the compaction stopped.

Where a shard was refused the outcome is `refused` whatever happened to the others, and otherwise `failed` where any compaction failed. Requests made close together are answered together, and `first` and `last` give the range of request numbers that share the answer. The server keeps every answer until it stops.

A number the server has not given since it started answers `404`:

```console
mosaica$ curl --silent --unix-socket /run/mosaica/control.sock -H "Authorization: Bearer $MOSAICA_CREDENTIAL" http://localhost/control/compact/2 | jq
{
  "error": "unknown",
  "detail": "this server has given no compaction request the number 2: request numbers start again at 1 when the server restarts, so a request made before a restart is lost; ask for a compaction again"
}
```

Request numbers start again at 1 when the server restarts, so a `404` for a number you were given means the server restarted after you asked, and the request was lost with it. Ask again. If the write side stops after a request was accepted, the request stays waiting and `write_executor.posture` in `mosaica status` reads `dead`.

When no compaction can start until the server restarts, the request is refused at once with `503 not-ready`. That happens when the write side isn't running, when the record of deletions no longer matches the log, when a compaction finished on disc but the server couldn't switch to it, or when part of the server is serving an older state of the bundle than the newest on disc. Except when the write side isn't running, the detail says which, and what to do.

[Background maintenance](../system/write-path.md#background-maintenance) in Changing data explains how compaction fits with flushes and merges.

## A write that holds up other writes

Between compactions, one kind of write can hold up the others for a while. When a publication, a growth, a flush or a merge gives an item a second artifact in an annotation level that holds one artifact per row, the server rewrites that level's files to hold a list, on the thread that applies writes, and other writes wait until it finishes.

??? info "Which levels this affects"
    The rewrite happens only when the server holds the level in memory. A level whose layer declares `layout = "column"`, and whose rows the server holds artifact by artifact, is served by artifact instead, with no rewrite. A level the server doesn't hold is read as a list by the next request that reads it. The next compaction records the level as a list and writes its files again.
