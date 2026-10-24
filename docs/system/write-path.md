# Changing data

A running Mosaica server takes writes while it serves: new items, edits, deletions, suppressions, and new declarations. This chapter describes each write, when viewers see it, what the server answers, and what the server does in the background to keep the data quick to read. The examples continue with the research papers of the [data model](data-model.md).

Writes go to the control plane, which is usually kept to a private address or a Unix socket. The Python and TypeScript clients wrap every write, and the [capabilities](../reference/capabilities.md) table names the call on each surface. Not built yet: writing to a running server from the `mosaica` command line.

## The writes

| Write | Route | Viewers see it |
|---|---|---|
| Insert new items | `POST /control/ingest` | at the next flush |
| Add existing items to another view | `POST /control/ingest` | at the next flush |
| Edit items | `POST /control/ingest` | at the next flush. The item leaves the map until then |
| Delete items | `POST /control/changes` | on the next request |
| Suppress items, and lift a suppression | `POST /control/changes` | on the next request |
| Declare fields, views, layers and vocabularies, and publish annotations | `/control/attributes`, `/control/views/…`, `/control/layers/…`, `/control/vocabularies/…` | once the server answers |

A **flush** is how new and edited items reach the map. It runs every 90 seconds by default, or sooner once 40,000 rows are waiting. See [when new data appears](#when-new-data-appears).

## Inserting and editing

One `POST /control/ingest` sends a batch of rows to one view, named by the `x-mosaica-view` header. The body is JSON, or Arrow record batches. Each row carries the item's fields, its position in the view, and optionally its access label.

Each row names the item it is about by its `mosaica_id` or by a [unique field](data-model.md#naming-an-item), such as a DOI. The server compares each row with what that item holds, and decides:

| The row names | and carries | Outcome |
|---|---|---|
| no item | a position | a new item is created |
| no item | no position | refused: an item is created in a view |
| an item | a position in a view it is not in, and nothing else new | the item is added to this view |
| an item | new values, a new label or a new position | the item is edited |
| an item | only what it already holds | nothing changes |
| two different items | anything | refused |

In a row, a field you leave out keeps its value, and a null clears it. A row carries both coordinates or neither. A point outside the view's frame is stored on the frame's edge and counted as `clamped` in the answer.

The answer lists each row's `mosaica_id`, counts the rows that created, added, edited and changed nothing, and lists every refused row with its position and a reason, such as `names_two_items` or `unknown_mosaica_id`. A refused row writes nothing, and the batch's other rows still apply. Add `strict=true` to refuse the whole batch instead if any row is refused.

### Retrying safely

Every batch carries an id of your choosing, in the `x-mosaica-batch-id` header. If you send the same bytes again under the same id, the server answers what it answered the first time and writes nothing new. Sending different bytes under an id already used is refused. So a client that loses an answer to a timeout can always send the batch again.

!!! note "How long a batch id is remembered"
    A batch id is remembered while its record is in the server's log, and after that for the flush interval, or until `flush_max_items` rows of later batches arrive. A retry sent after that is treated as a new batch. Retry promptly.

### Answers

| Status | Meaning | What to do |
|---|---|---|
| 200 | Every row was decided. New, added and edited rows are durable, and appear at the next flush. Refused rows are listed | Fix and resend any refused rows |
| 409 | With `strict=true`, a row was refused; or the batch id was reused with different bytes. Nothing was written | Fix the batch |
| 422 | The batch is malformed: an undeclared column, a wrong type, too many rows, a new item with no label, or a coordinate that is not a number | Fix the batch |
| 429 | The server is too busy to take the batch, or too many rows are waiting for a flush. The answer says when to retry | Retry after that interval, with the same batch id |
| 500 | The batch could not be written to the log. Nothing was applied | Retry with the same bytes and batch id |
| 503 | The server's writer is not running | Retry later |

## When new data appears

A new or edited item is durable as soon as the server answers, but it is not yet on the map. It waits in memory until the next **flush**, which writes the waiting rows to disc in map order and publishes them. A flush runs every `flush_max_age_secs` (90 seconds by default), or sooner once `flush_max_items` rows (40,000 by default) are waiting.

Until the flush, an inserted item is in no count, sample or bulk read, and opening it answers as if it did not exist. After the flush, a viewer sees it on their next request, or shortly after while the server refreshes that viewer's cached state.

To see a write sooner:

- **`wait=visible`** on an ingest holds the answer until the write is on the map, or until `visible_wait_max_secs` (30 seconds by default) passes, and says which.
- **`POST /control/flush`** asks for the next flush now. With `wait=visible` it answers once that flush has run.

!!! warning "An edited item leaves the map until the next flush"
    An edit replaces the item's stored row. The old row is hidden as soon as the edit is accepted, and the new one appears at the flush. For up to one flush interval the item is on no map and in no count. Its `mosaica_id`, its views, its annotation memberships and any suppression carry across the edit.

Other viewers' sessions learn only that something has changed, on their next response, through the content key the server sends with every answer. They are not told what changed.

## Deleting and suppressing

`POST /control/changes` takes a list of changes, each one of:

- **`delete`**: removes the item for good. Its `mosaica_id` never names anything again, and its unique values are free for a new item at once.
- **`suppress`**: hides the item from every viewer, but keeps it, its values and its unique values. Use it to withdraw an item while you decide what to do with it.
- **`unsuppress`**: lifts a suppression. The item comes back as it was.

Each change names its item in `match`, by `mosaica_id` or unique values, as an ingest row does. The same operations apply to an annotation artifact, or a whole layer, named by its `mosaica_id`.

A deletion or suppression applies to every request that starts after the server answers, from every viewer, including the caller's own next request. No cache can serve the item after that.

- **Never refused for load.** A deletion or suppression is a security operation, so the server never declines one because it is busy. Ingest can be declined with a 429. Changes cannot.
- **Safe to resend.** Suppressing an item twice has no further effect. A deletion sent again finds no item, and is listed as refused rather than failing the request.
- **Refused changes.** A change naming no item, or two, is listed in the answer and the others apply. With `strict=true` the request is refused instead: 404 where a change names no item, 409 where it names two.

!!! note "If a change cannot be written to the log"
    If the server cannot make a change durable, it answers 500. A deletion or suppression in that request is already in force anyway, since hiding an item early is safe. A lift of a suppression is not applied. Resend the request: until it succeeds, a restart would bring the deleted or suppressed items back.

!!! warning "Deleted data stays on disc until a compaction"
    A deleted item is hidden at once, but its stored values remain in the database's files until the next [compaction](#background-maintenance) removes them from the current version of the data. The server also keeps a few earlier versions for recovery, which can still hold the deleted values. If you delete data because the law or a privacy request requires it, run `POST /control/compact` afterwards.

## Durability and restarts

The server writes every accepted write to a write-ahead log, and syncs it to disc, before it answers. After a crash or restart it reads the newest saved state and replays the log over it, so every write it answered is restored: new items, edits, deletions, suppressions and lifted suppressions.

Ingest is limited under load, so a burst of writes cannot starve the map. Past `ingest_buffer_max_items` rows waiting for a flush (a million by default), or too many batches at once, an ingest is refused with 429 and a retry interval. Deletions and suppressions have their own queue and are never refused for load.

## Background maintenance

Three kinds of background work keep the data quick to read. None of them changes what a viewer may see.

- **Flush** writes waiting rows to disc as a new segment, a file of rows in map order, and publishes them. This is what makes new and edited items appear.
- **Merge** combines small segments into larger ones, so a request has fewer files to read. It runs as segments accumulate, and changes nothing a viewer sees.
- **Compaction** rewrites the data as one segment per view, without the rows of deleted items. It is the only step that removes deleted data from disc and reclaims its space.

Compaction is scheduled. By default it runs in a daily window starting at 00:00 UTC and lasting four hours, when a view has 8 or more segments, and at any time when a view reaches 64 segments, when 20% of the stored rows belong to deleted items, or when deleted items pass the overlay's soft limit. The schedule starts at most one compaction a day. `POST /control/compact` runs one now. The `compaction_*` settings in the [server configuration](../reference/mosaica-toml.md) change the schedule.

A compaction runs alongside requests, which keep being answered. When it finishes, each viewer's cached state is rebuilt, either by a background pass or on the viewer's next request, so that request can be slower. Identifiers a client holds, such as `mosaica_id`s and tile addresses, keep working across every kind of maintenance.

## Not built yet

- **Telling clients what changed.** A response says that the data has changed, not which items or which part of the map. A client that wants to know re-requests what it shows.
- **Notifying clients when a label they hold is invalidated** by a deletion. A label check always reads current state, so nothing stale is served, but nothing announces the change either.
- **Replicas.** A deployment is one server.
- **Writing from the command line.** The `mosaica` command builds and reads. Use the HTTP API or a client to write to a running server.
