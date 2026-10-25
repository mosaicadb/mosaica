# Schedule compactions

Deleted and edited items stay in the bundle, hidden from every viewer, until a compaction removes them. Compaction also merges the segments that new data adds and frees the disc space that merging leaves behind. The server compacts on its own, mostly in a nightly window, so the main thing to decide is when that window falls. This page explains what starts a compaction, what it needs, and how to start one yourself and check it worked.

The examples use the [systemd install](systemd.md), whose control address is a Unix socket. Under Docker, write `--control http://127.0.0.1:9163` instead.

## When the server compacts

Each time the server makes new items visible, it adds a segment to the bundle, and it merges small segments as they build up. A compaction rewrites the bundle as one segment per view, without the rows that were deleted or replaced. The server starts one when any of these holds:

- during a four-hour window from midnight UTC, a view has 8 segments or more;
- at any hour, a view has 64 segments;
- 500,000 deleted items and annotations are waiting to be removed;
- deleted items are a fifth or more of all rows;
- files the bundle no longer uses take as much space as the files it does.

Set `compaction_window_start` under `[ingest]` to the start of your quietest hours, in UTC as `"HH:MM"`. Once one compaction starts, none starts again for a day (`compaction_min_interval_secs`), whatever else holds. The [mosaica.toml reference](../reference/mosaica-toml.md) lists the key for each condition, and writing `"off"` for one turns it off and leaves the others.

Viewers are answered as usual while a compaction runs. When it finishes, every session's cached view of the map is out of date at once. A background pass rebuilds them, and a session it hasn't reached yet pays for its own on its next request, so expect some slower requests straight after a compaction.

??? info "Details of the schedule"
    - The server keeps the time of the last compaction in memory, so after a restart the first condition that holds starts one straight away.
    - A compaction that was refused doesn't count towards the interval.
    - A bundle of more than one shard (`[shards] size` in the schema) is compacted a shard at a time. Each shard is checked against the conditions on its own counts, and the interval applies to each shard on its own.

## What a compaction needs

A compaction needs free space on the bundle's disc of one and a half times the bundle's current size, or of its largest shard, and enough memory for its own estimate, within the cap you set. Without either it is refused, and `/control/status` records why under `compaction.last_refusal`. [Disc](sizing.md#disc) explains how to plan disc space for it.

## Start one yourself

`mosaica compact` asks for a compaction now, whatever the schedule says. It needs a credential with `write`, read from `MOSAICA_CREDENTIAL`:

```console
mosaica$ export MOSAICA_CREDENTIAL=$(cat secrets/operator.secret)
mosaica$ mosaica compact --control unix:/run/mosaica/control.sock
{"request":1}
```

The server compacts every shard with something to reclaim, one after another. The answer is the request's number, counted from 1 each time the server starts. A request made while a compaction is running waits for it to finish. Over HTTP, the same request is `POST /control/compact`, and the Python and TypeScript clients have `compact`.

To see how it went, read `compaction` in `mosaica status`:

```console
mosaica$ mosaica status --control unix:/run/mosaica/control.sock | jq '.compaction | {folds, requests}'
{
  "folds": 1,
  "requests": {
    "accepted": 1,
    "answered": 1,
    "last_refusal": null
  }
}
```

Your request is finished once `answered` reaches its number. If `last_refusal` is null, or its `request` is below your number, nothing refused it. To confirm the deleted rows are gone, check that `folds` rose and `overlay.retirable` fell.

??? info "Reading the outcome in full"
    `last_refusal` holds only the latest refusal that answered a request, and its `request` is the highest request it answered. Once your request is answered:

    - `last_refusal.request` below your number: none of your request's shards was refused.
    - Equal to it: a shard was refused while answering it. `last_refusal` names the shard and the reason in `gate`, gives the bytes needed and available for a memory or disc refusal, and says what to do in `detail`.
    - Above it: a later request's refusal has replaced the record. Requests answered together share a refusal, so it may or may not concern yours.

    A request answered without a refusal may still have failed: `fold_failures` counts the compactions that failed or were discarded. A compaction the schedule started and the server refused is recorded only under `compaction.last_refusal`, not under `requests`. If the write side stops after the request was accepted, the request is never answered, and `write_executor.posture` reads `dead`.

When no compaction can start until the server restarts, the request is refused at once with `503 not-ready`. That happens when the write side isn't running, when the record of deletions no longer matches the log, when a compaction finished on disc but the server couldn't switch to it, or when part of the server is serving an older state of the bundle than the newest on disc. The detail says which, and what to do.

## A write that holds up other writes

Between compactions, one kind of write can hold up the others for a while. When a publication, a growth, a flush or a merge gives an item a second artifact in an annotation level that holds one artifact per row, the server rewrites that level's files to hold a list, on the thread that applies writes, and other writes wait until it finishes.

??? info "Which levels this affects"
    The rewrite happens only when the server holds the level in memory. A level whose layer declares `layout = "column"`, and whose rows the server holds artifact by artifact, is served by artifact instead, with no rewrite. A level the server doesn't hold is read as a list by the next request that reads it. The next compaction records the level as a list and writes its files again.
