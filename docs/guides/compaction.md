# Schedule compactions

Deleted and edited items stay in the bundle, hidden from every viewer, until a compaction removes them. Compaction also rewrites the segments that new data adds as one segment per view, and frees the disc space that a failed or interrupted write leaves behind. The server compacts on its own, mostly in a nightly window, so the main thing to decide is when that window falls. This page explains what starts a compaction, what it needs, and how to start one yourself and check it worked.

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
mosaica compact: waiting for request 1794068664464
16:24:26Z [2/4 Indexing records] attributes shard 0  1s  rss 102 MiB
16:24:28Z [4/4 Publishing] derived shard 0  3s  rss 104 MiB
16:24:35Z [4/4 Publishing] retire shard 0  10s  rss 103 MiB
16:24:36Z [4/4 Publishing] reclaim shard 0  10s  rss 102 MiB  finished
{"answer":{"detail":null,"first":1794068664464,"gate":null,"had_bytes":null,"last":1794068664464,"need_bytes":null,"ok":true,"outcome":"compacted","shard":null},"answered":true,"job":{"context":"shard 0","elapsed_s":10.119004202,"job":"compaction","phase":{"index":4,"name":"Publishing","of":4},"rss_bytes":107151360,"state":"finished","step":"reclaim","step_elapsed_s":0.0242591,"step_id":21},"request":1794068664464}
```

The first line names the request's number. Each time the server starts it numbers its requests from the time in milliseconds, so a number from before a restart is never given again. The lines after it follow the compaction through its four phases, naming the shard it is rewriting, and the last of them says how it ended. The command reads the compaction's progress once a second, so a step shorter than that, here the first phase, may not be shown. In a terminal `--progress` draws a bar in their place, `json` prints each record as a JSON object, and `none` prints nothing. These lines go to stderr, and the server's answer, the last line, goes to stdout. The server compacts every shard with something to reclaim, one after another, and a request made while a compaction is running waits for it to finish. Without `--wait` the command prints `{"request":…}` and returns at once.

`mosaica compact --wait` exits 0 when the request was compacted or had nothing to compact. It exits 1 when a shard was refused, when a compaction for the request failed, and when the server restarted while it waited, which loses the request, and in each case prints the server's explanation on stderr. Over HTTP the request is `POST /control/compact`, and the Python and TypeScript clients have `compact`.

## Read how a request went

`mosaica compact --request` reads a request you made earlier, through `GET /control/compact/{request}`, and `--request` with `--wait` waits for it as above. It needs `admin`:

```console
mosaica$ mosaica compact --request 1794068664464 --control unix:/run/mosaica/control.sock | jq
{
  "answer": {
    "detail": null,
    "first": 1794068664464,
    "gate": null,
    "had_bytes": null,
    "last": 1794068664464,
    "need_bytes": null,
    "ok": true,
    "outcome": "compacted",
    "shard": null
  },
  "answered": true,
  "job": {
    "context": "shard 0",
    "elapsed_s": 10.119004202,
    "job": "compaction",
    "phase": {
      "index": 4,
      "name": "Publishing",
      "of": 4
    },
    "rss_bytes": 107151360,
    "state": "finished",
    "step": "reclaim",
    "step_elapsed_s": 0.0242591,
    "step_id": 21
  },
  "request": 1794068664464
}
```

While the request waits, `answered` is `false`, `answer` is `null`, and `job` is the progress of the compaction serving it, once the server has started one. Once it is answered, `job` is the last record of the compaction that answered it, `answer.ok` says whether the request succeeded, and `answer.outcome` is one of these:

- `compacted`: a compaction for the request was published and the deleted rows are gone.
- `nothing_to_do`: no shard the request named had anything to compact.
- `refused`: a shard was refused. `gate` names why, such as `insufficient_disc`, `detail` says what to do, `shard` names the shard, and for a memory or disc refusal `need_bytes` and `had_bytes` give what the compaction needed and what the host had.
- `failed`: a compaction for the request failed or was discarded. `detail` names the step it stopped in, and `job` is its last record.

Requests made close together are answered together, and `first` and `last` give the range of request numbers that share the answer. The server keeps the latest 1,024 answers, and an older one answers `410`. The [HTTP reference](../reference/http.md) describes the route in full.

A server forgets its requests when it stops, so a number from before a restart answers `404`. This is the request above, read again after the server restarted:

```console
mosaica$ mosaica compact --request 1794068664464 --control unix:/run/mosaica/control.sock
mosaica compact: the server answered 404: {"error":"unknown","detail":"this run of the server has given no compaction request the number 1794068664464: a number from before the server restarted is lost with the request; ask for a compaction again"}
```

A `404` for a number you were given means the server restarted after you asked, and the request was lost with it, so ask again. If the write side stops while a request waits, reading the request answers `503 not-ready`, because it will not be answered.

When no compaction can start until the server restarts, the request is refused at once with `503 not-ready`. That happens when the write side isn't running, when the record of deletions no longer matches the log, when a compaction finished on disc but the server couldn't switch to it, or when part of the server is serving an older state of the bundle than the newest on disc. Except when the write side isn't running, the detail says which, and what to do.

[Background maintenance](../system/write-path.md#background-maintenance) in Changing data explains how compaction fits with flushes and merges.

## A write that holds up other writes

Between compactions, one kind of write can hold up the others for a while. When a publication, a growth, a flush or a merge gives an item a second artifact in an annotation level that holds one artifact per row, the server rewrites that level's files to hold a list, on the thread that applies writes, and other writes wait until it finishes.

??? info "Which levels this affects"
    The rewrite happens only when the server holds the level in memory. A level whose layer declares `layout = "column"`, and whose rows the server holds artifact by artifact, is served by artifact instead, with no rewrite. A level the server doesn't hold is read as a list by the next request that reads it. The next compaction records the level as a list and writes its files again.
