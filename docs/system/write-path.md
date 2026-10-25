# Changing data

You change a Mosaica database while it serves: you insert and edit items, delete or suppress them, and declare new fields, views and layers, all without stopping the map. A database can start empty and take everything this way. For a large first load, `mosaica build` loads a set of files in one pass, which is faster, and the Python client's first commit runs one for you. Every change after that goes to the running database.

Writes go to the control address, set by `[serve] control`. It is separate from the address viewers use, so you can keep it private or put it on a Unix socket. The tables below name each write's route, call or command on each surface.

## The writes

=== "HTTP"

    | Write | Route | Viewers see it |
    |---|---|---|
    | Insert new items | `POST /control/ingest` | at the next flush |
    | Add existing items to another view | `POST /control/ingest` | at the next flush |
    | Edit items | `POST /control/ingest` | at the next flush. The item leaves the map until then |
    | Delete items | `POST /control/changes` | on the next request |
    | Suppress items, and lift a suppression | `POST /control/changes` | on the next request |
    | Declare fields, views, layers and vocabularies, and publish annotations | `/control/attributes`, `/control/views/…`, `/control/view_groups/…`, `/control/layers`, `/control/vocabularies/…` | once Mosaica answers |

=== "Python"

    `Database` writes to a database the Python client runs itself. It sends inserts and declarations at `commit`, and deletions and suppressions at once. On a server elsewhere, `Control` has the TypeScript client's calls, as `ingest`, `changes`, `declare_attribute` and so on.

    | Write | Call | Viewers see it |
    |---|---|---|
    | Insert new items | `Database.insert`, then `commit` | at the next flush |
    | Add existing items to another view | `Database.insert` into the view, then `commit` | at the next flush |
    | Edit items | `Database.insert`, then `commit` | at the next flush. The item leaves the map until then |
    | Delete items | `Database.remove` | on the next request |
    | Suppress items, and lift a suppression | `Database.suppress`, `Database.unsuppress` | on the next request |
    | Declare fields, views, layers and vocabularies, and publish annotations | `Database.declare_attribute`, `declare_view`, `declare_view_group`, `declare_layer`, `declare_labels`, `declare_vocabulary`, and `insert` into a layer to publish, then `commit` | once Mosaica answers |

=== "TypeScript"

    `Control` sends each write with the credential you give it: the operator credential, an API key or an OIDC access token.

    | Write | Call | Viewers see it |
    |---|---|---|
    | Insert new items | `Control.ingest` | at the next flush |
    | Add existing items to another view | `Control.ingest` | at the next flush |
    | Edit items | `Control.ingest` | at the next flush. The item leaves the map until then |
    | Delete items | `Control.changes` | on the next request |
    | Suppress items, and lift a suppression | `Control.changes` | on the next request |
    | Declare fields, views, layers and vocabularies, and publish annotations | `Control.declareAttribute`, `declareView`, `declareLayer`, `declareVocabulary`, `publish` | once Mosaica answers |

=== "CLI"

    | Write | Command | Viewers see it |
    |---|---|---|
    | Insert new items | `mosaica insert` | at the next flush |
    | Add existing items to another view | `mosaica insert --view` | at the next flush |
    | Edit items | `mosaica insert` | at the next flush. The item leaves the map until then |
    | Delete items | `mosaica delete` | on the next request |
    | Suppress items, and lift a suppression | `mosaica suppress`, `mosaica unsuppress` | on the next request |
    | Declare fields, views, layers and vocabularies, and publish annotations | `mosaica declare`, `mosaica publish` | once the server answers |

New and edited items reach the map at the next **flush**, which runs every 90 seconds by default, or sooner once 40,000 items are waiting. [When new data appears](#when-new-data-appears) explains how to see a write sooner.

## Inserting and editing

You insert and edit with the same request. One `POST /control/ingest` sends a batch of rows to one view, named by the `x-mosaica-view` header, which you can leave out when the database has only one view. The body is JSON or Arrow record batches, and each row carries the item's fields, its position in the view and, if you want, its access label.

A row names the item it's about by its `mosaica_id` or by a [unique field](data-model.md#naming-an-item), such as a DOI. Mosaica compares the row with what that item already holds, and decides what to do:

| The row names | and carries | Outcome |
|---|---|---|
| no item | a position | a new item is created |
| no item | no position | refused: a new item needs a position |
| a `mosaica_id` that names nothing | anything | refused: Mosaica chooses a new item's `mosaica_id` |
| an item | a position in a view it is not in, and nothing else new | the item is added to this view |
| an item | new values, a new label or a new position | the item is edited |
| an item | only what it already holds | nothing changes |
| an item an earlier row of the batch names | anything | refused. The earlier row is kept |
| two different items | anything | refused |

A row only changes what it carries. A field it leaves out keeps its stored value, and a null clears it; in Arrow, leaving a column out of the batch leaves it out of every row. The access label is the exception: left out or null, it keeps the item's label, and an empty label takes the view's `point_visibility.default`. Coordinates come both or neither, and a row without them leaves the item where it is.

??? info "Points outside the view's frame"
    A point outside the view's frame is stored on the frame's edge and counted in `clamped`. On a geographic view, a latitude beyond the projection's range is stored on the edge and counted in `clipped`.

The answer lists each row's `mosaica_id`, counts the rows in `created`, `added`, `edited` and `unchanged`, and lists every refused row with its position and a reason, such as `names_two_items` or `unknown_mosaica_id`. A refused row writes nothing, but the batch's other rows still apply. If you'd rather the whole batch were refused when any row is, add `strict=true`.

### Retrying safely

Every batch must carry an id of your choosing, in the `x-mosaica-batch-id` header, so that a client which loses an answer to a timeout can simply send the batch again. Ids belong to the caller that sends them. If the same caller sends the same bytes again under that id, Mosaica writes nothing and answers `replayed: true`, with the first answer's `mosaica_id`s and refused rows and every count 0. Different bytes under an id already used are refused with 409.

!!! note "How long a batch id is remembered"
    A batch id is remembered while the log holds the batch's record. Once the log rotates past it, which can happen soon after a flush, the id is remembered until `flush_max_age_secs` after the batch was accepted, and no longer once `flush_max_items` items of later batches have arrived. A retry after that is a new batch. Retry promptly.

### Answers

| Status | Meaning | What to do |
|---|---|---|
| 200 | Every row was decided. New, added and edited rows are durable, and appear at the next flush. Refused rows are listed | Fix and resend any refused rows |
| 404 | `x-mosaica-view` names no view | Name a declared view |
| 409 | With `strict=true`, a row was refused; the batch id was reused with different bytes; or, rarely, the batch would need more item numbers than a shard holds. Nothing was written | Fix the batch |
| 422 | The batch is malformed: an undeclared column, a wrong type, too many rows, a new item with no label, or a coordinate that is not a number or is out of range | Fix the batch |
| 429 | Mosaica is too busy to take the batch, or too many items are waiting for a flush. The answer says when to retry | Retry after that interval, with the same batch id |
| 500 | The batch could not be made durable. Usually nothing was applied, and the detail says when it may have been | Retry with the same bytes and batch id, which answers as a replay if it was applied |
| 503 | The writer is not running, or Mosaica is serving an older saved state after a fault | Retry later |

## When new data appears

A new or edited item is durable as soon as Mosaica answers, but it isn't on the map yet. It waits in memory until the next **flush**, which writes the waiting items to disc in map order and publishes them. A flush runs every `flush_max_age_secs` (90 seconds by default), or sooner once `flush_max_items` items (40,000 by default) are waiting.

Until then, an inserted item is in no count, sample or bulk read, and opening it answers as if it didn't exist. Its `mosaica_id` and unique values already name it, though, so you can edit or delete it before the flush. After the flush, a viewer sees it on their next request, or shortly after while Mosaica refreshes that viewer's cached state.

If you need a write on the map sooner, ask for a flush. `POST /control/flush` asks for the next flush now, and with `wait=visible` answers once it has run. The same `wait=visible` on an ingest asks for a flush and then holds the answer until the write is on the map, or until `visible_wait_max_secs` (30 seconds by default) passes; `visible` in the answer says which. Each such request starts a flush of its own, though, so a steady stream of them writes many small files, which Mosaica then has to merge.

!!! warning "An edited item leaves the map until the next flush"
    An edit replaces the item's stored row. The old row is hidden as soon as the edit is accepted, and the new one appears at the flush. For up to one flush interval the item is on no map and in no count. Its `mosaica_id`, its views, its annotation memberships and any suppression carry across the edit.

!!! note "New access attributes need a new session"
    A viewer whose session started before the flush that first used one of an item's access attributes does not see the item until they start a new session. Nothing tells the client this has happened.

A viewer's client learns from the content key on its next response that the data has changed, but not what changed.

## Deleting and suppressing

`POST /control/changes` takes a list of changes, and each names its item in `match`, by `mosaica_id` or unique values, as an ingest row does. A `delete` removes the item for good: its `mosaica_id` never names anything again, and its unique values are free for a new item at once. A `suppress` hides the item from every viewer but keeps it, its values and its unique values, which is what you want while you decide what to do with it, and an `unsuppress` brings it back as it was.

The same changes apply to an annotation artifact named by its `mosaica_id`, and a whole layer can be suppressed and unsuppressed too. To delete a layer, use `DELETE /control/layers/{name}`; a `delete` naming it here is refused with 422. Dropping a view of a group, with `DELETE /control/views/{group}/{key}`, deletes every item it leaves in no view, and the answer says how many.

A deletion or suppression applies to every request that starts after Mosaica answers, from every viewer, including your own next request, and no cache can serve the item after that. Because it's a security operation, Mosaica never declines one for being busy: an ingest can be answered 429, but a change can't. Changes are also safe to resend. Suppressing an item twice has no further effect, and a deletion sent again finds no item and is listed as refused rather than failing the request.

??? info "Refused changes"
    A change naming no item, or two, is listed in the answer and the others still apply. With `strict=true` the request is refused instead: 404 where a change names no item, 409 where it names two. A repeated deletion under `strict=true` is therefore a 404.

!!! note "If a change cannot be written to the log"
    If Mosaica can't make a change durable, it answers 500. A deletion or suppression in that request is already in force anyway, since hiding an item early is safe. A lift of a suppression is not applied. Resend the request: until it succeeds, a restart would bring the deleted or suppressed items back.

### Erasing deleted data

A deleted item is hidden at once, but its values stay in the database's files until a [compaction](#background-maintenance) rewrites them, and so do the values an edit replaces. To remove them without waiting for the schedule, send `POST /control/compact` after the deletion.

Compaction never removes three things: an access attribute that only deleted items' labels used, a key an open vocabulary added for a deleted item, and an artifact key a deleted item's insert created on an open annotation layer. To erase those, rebuild the database from sources that leave them out. [Erase deleted data completely](../guides/rebuild.md#erase-deleted-data-completely) gives the steps.

??? info "What a compaction removes, and when"
    After the compaction, the previous files are removed once no request is reading them, usually within one flush interval, and the log's copy goes when the log next rotates. The cache directory holds no field values; counts and positions the item contributed to are dropped when the cache is next swept, or evicted sooner.

!!! note "Confirming that a compaction ran"
    `POST /control/compact` answers 202 with the request's number once it has asked for a compaction, and the compaction can still be refused, for example for lack of disc space, or fail. Check `/control/status` until `compaction.requests.answered` reaches that number. If `compaction.requests.last_refusal.request` then equals it, a shard was refused and `detail` says why; below it, nothing was refused; above it, a later request's refusal has replaced the record. To confirm the deleted rows are gone, check that `compaction.folds` has risen and `overlay.retirable` has fallen; `compaction.fold_failures` counts compactions that failed or were discarded. Where no compaction could start until the server is restarted, the request is answered 503 with the reason.

Copies outside Mosaica are yours to remove: backups, filesystem snapshots, the build's source files, and the request log if `[serve] request_log` is set.

## Durability and restarts

Mosaica writes every accepted write to a write-ahead log, and syncs it to disc, before it answers. After a crash or restart it reads the newest saved state and replays the log over it, so every write it answered comes back: new items, edits, deletions, suppressions and lifted suppressions.

Ingest is limited under load, so that a burst of writes can't starve the map. Once `ingest_buffer_max_items` items are waiting for a flush (a million by default), or too many batches arrive at once, an ingest is refused with 429 and a retry interval. Deletions and suppressions have their own queue, and are never refused for load.

## Background maintenance

Three kinds of background work keep the data quick to read, and none of them changes what a viewer may see. A **flush** writes waiting items to disc as a new segment, a file of rows in map order, and publishes them, which is what makes new and edited items appear. As segments accumulate, a **merge** combines small ones into larger ones, so a request has fewer files to read. And a **compaction** rewrites the data as one segment per view, without the rows of deleted items or the values an edit replaced. It's the only step that removes them and reclaims their space.

Compaction runs on a schedule. By default it runs:

- in a daily window starting at 00:00 UTC and lasting four hours, when a view has 8 or more segments;
- at any time, when a view reaches 64 segments, when 20% of the stored rows belong to deleted items, when the deleted items and annotations reach the overlay's soft limit, or when files no longer used reach the size of those in use.

An edit leaves its old version as a deleted item, so heavy editing counts towards these triggers. The schedule starts at most one compaction a day for each shard, and that limit starts again after a restart. `POST /control/compact` asks for one now. The `compaction_*` settings in the [server configuration](../reference/mosaica-toml.md) change the schedule.

A compaction runs alongside requests, which keep being answered. When it finishes, each viewer's cached state is rebuilt, either by a background pass or on the viewer's next request, so that request can be slower. The identifiers a client holds, such as `mosaica_id`s and tile addresses, keep working across every kind of maintenance.

## Not built yet

- **Telling clients what changed.** A response says that the data has changed, not which items or which part of the map. A client that wants to know re-requests what it shows.
- **Telling a client its session is behind.** Nothing tells a client that items under a new access attribute need a new session.
- **Notifying clients that annotation content they hold was withdrawn** because an item it was generated from was deleted. Each request checks current state, so the content is not served again, but nothing announces the change.
- **Removing access attributes that no remaining label uses, and unused vocabulary keys and artifact keys,** at compaction. Rebuild from source files that leave them out.
- **Replicas.** A deployment is one database on one machine.
- **Writing from the command line.** The `mosaica` command builds and reads. Use the HTTP API or a client to write to a running database.
