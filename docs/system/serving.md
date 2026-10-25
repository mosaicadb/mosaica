# Serving

Mosaica answers every viewer from one process, and most of what it does between a request and its answer is about keeping the map quick for everyone at once. It streams answers so a client can start drawing early, keeps busy requests from crowding each other out, and does the expensive work for a set of viewers once rather than on every request. Each answer also tells the client whether what it already holds is still current. For sizing and settings, see the [operating guide](../guides/operating.md).

## How an answer arrives

A map answer comes in frames, so a client can start drawing before Mosaica has finished:

| Frame | Carries | How many |
|---|---|---|
| counts | each tile's visible, matched, highlighted and drawn counts. A tile with nothing visible has no row | one, first |
| density | counts for each tile's finer cells, over everything the viewer may see, ignoring the filter | one, when asked for |
| points | the drawn points of one or more tiles, with each point's topic in each layer the request names | any number |
| trailer | how many points were sent, marking the answer complete | one, last |

Because a tile's points always come in the same order, an answer that stops early is still correct as far as it goes. Whether the client went away, a time limit passed or Mosaica met a fault, each tile's points are the start of that tile's full answer, with nothing skipped or reordered, and only the missing trailer shows that the answer is incomplete.

The annotations of each tile arrive the same way, as one self-contained frame per tile and a trailer. A request for a `nested` or `dag` layer gets one extra frame first, holding the tree's cut across the whole screen.

### Bulk reads

A bulk read arrives in pages, each followed by a page end. A response stops when the read is finished or when it reaches its time or size limit, and its trailer then carries a cursor to continue from. The Python and TypeScript clients and the `mosaica` command follow the cursor for you.

??? info "If a client stops reading"
    A client that stops reading for too long is cut off without a trailer. The clients then stop with an error carrying the cursor of the last page end they received, so the caller can read on from there. A page with no page end after it is discarded.

## When Mosaica is busy

Each kind of request has its own limit on how many run at once, so a long export can't starve the map. Map and item requests wait briefly for computing capacity, and are refused at once only when every place is taken. Bulk reads run two at a time by default, and a third is refused at once. Annotations by tile run one per compute thread by default, with as many again waiting.

A request refused for load is answered `429` with a one-second wait. An insert refused because too many items are already waiting gets a wait worked out from how fast those items are being written, which can also be a second. Deletions and suppressions are never refused for load, as [changing data](write-path.md#deleting-and-suppressing) explains.

??? info "Two `429`s you can see on a quiet server"
    Both ask the client to retry after the wait given:

    - another request is already building this grant's state, described [below](#what-the-server-keeps-ready), and the wait for it ran out;
    - Mosaica is rebuilding the grant's state after a merge, and the viewer has already been shown the view.

Who retries depends on the client. The TypeScript store retries a map, annotations, aggregate or suggestion request refused with `429` or `503`, but not a bulk read. `Control`, in both clients, retries a refused write. The Python client's reads and the `mosaica` command retry nothing, so your code decides.

A map request keeps its place until its whole answer has been sent, but it gives up its share of the computing as soon as its counts are worked out, so a slow reader holds up no one else's work. When a client goes away, Mosaica stops working on its answer.

## What the server keeps ready

Viewers who hold the same access labels share a **grant**, and Mosaica keeps its expensive state per grant, not per viewer or per request. For each grant it keeps the visible set, which items the grant may see, arranged over the map's stored rows, built on the grant's first request. It also keeps each annotation level's figures, every artifact's count, centre and box over that visible set, built the first time the grant reads the level.

Every later request, from any session with the grant, reuses them, and each request corrects them for whatever has been deleted, suppressed or ingested since they were built. The figures and the visible set's list of items are written to disc and read back after a restart. The visible set's arrangement over the rows is held in memory only, so each grant's first request after a restart builds that again.

So the first request from a new grant is the slow one, and its cost grows with how much of the data the grant may see. You pay it once for each grant, so a few groups of viewers cost a few first requests, while users who each hold terms of their own cost one each. The [operating guide](../guides/operating.md#annotation-layers-and-new-viewers) explains how to plan for it.

??? info "After a flush, a merge or a compaction"
    When new data is flushed or segments are merged, a background pass brings each grant's visible set up to date. Until it reaches a grant, a viewer already shown the view is answered from the previous version after a flush, so new items appear a moment later, and is asked to retry after a merge. A session's first read in the view does the update itself. After a [compaction](write-path.md#background-maintenance), the same pass rebuilds each grant's visible set, and each level's figures are rebuilt for the compacted part of the data the next time they are read.

The data itself is mapped from disc, as the [overview](overview.md#how-it-serves-large-data) describes, so the parts of the map people are looking at stay in memory.

## What a client may already hold

Each map answer, and each answer of annotations by tile, carries three values a client can compare with what it holds. The identity key, `x-mosaica-identity-key`, says whether a held answer may still be shown at all. It changes when the viewer's access, their visible set or the view changes, and when the database is rebuilt, which gives every item a new `mosaica_id`. The content key, the `ETag`, is narrower: it changes whenever the data has changed in a way that could add something the client doesn't have. And the generation, `x-mosaica-pin`, names the version of the data the answer came from. Echo it in a request and the reply's `x-mosaica-stale` says whether the data has moved on since.

To save work, a client can leave out the tiles it already holds. Mosaica does nothing for a tile a request doesn't name.

## Health and readiness

The viewer and session addresses answer `/healthz` and `/readyz` without a credential. `/healthz` says the process is answering, and `/readyz` says whether to send it traffic. It fails when the part of Mosaica that takes writes isn't running normally, or when Mosaica has opened an older state of the data because the newest failed its checks.

!!! warning "Don't restart on a failed readiness probe"
    When the write-ahead log has failed, or Mosaica has opened an older state of the data, it reports not ready but still applies deletions and suppressions. Stop sending it ordinary requests, but keep sending it those, and don't restart it because the probe failed: a deletion or suppression that was applied but couldn't be written to the log is lost at a restart, and the item comes back.

    Once the log is repaired, `/readyz` reports ready again, but no new items become visible until a restart, and Mosaica logs an alarm saying so. The operating guide explains [what to do when a write to the log fails](../guides/operating.md#when-a-write-to-the-log-fails).

A disc that hangs on a write, rather than failing it, leaves `/readyz` reporting ready. The [operating guide](../guides/operating.md#health-and-readiness) covers the checks.

## Not built yet

- **A route for tile-based map tools.** MapLibre, OpenLayers and QGIS ask for one tile at a time. An adapter can turn each tile into a map request of its own, at the cost of one evaluation per tile instead of one for the whole screen.
- **Replicas.** A deployment is one database on one machine.
- **Keeping a filter's result between requests.** Panning or zooming with a filter applied evaluates it again on each request.
- **Telling Mosaica what a client already holds.** A client can leave out whole tiles, but cannot say which points of a tile it has. Every tile it names is answered in full.
