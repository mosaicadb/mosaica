# Serving

One Mosaica server answers every viewer from one process. This chapter covers what happens between a request and its answer: how an answer reaches the client, what the server does when it is busy, what it keeps ready between requests, and how a client can tell whether what it holds is still current. The [operating guide](../guides/operating.md) covers sizing and settings.

## How an answer arrives

A map answer is streamed in frames, so a client can start drawing before the server has finished:

| Frame | Carries | How many |
|---|---|---|
| counts | each tile's visible, matched, highlighted and drawn counts. A tile with nothing visible has no row | one, first |
| density | counts for each tile's finer cells, over everything the viewer may see, ignoring the filter | one, when asked for |
| points | the drawn points of one or more tiles, with each point's topic in each layer the request names | any number |
| trailer | how many points were sent, marking the answer complete | one, last |

Within a tile, points always come in the same order. An answer that stops early, because the client went away, a time limit passed or the server met a fault, is still correct as far as it goes: each tile's points are the start of that tile's full answer, with nothing skipped or reordered. Only the missing trailer says it is incomplete.

The annotations of each tile arrive the same way: one frame for each tile, each complete in itself, and a trailer. A request for a `nested` or `dag` layer gets one more frame first, holding the tree's cut across the whole screen.

### Bulk reads

A bulk read arrives in pages, each followed by a page end. A response stops when the read is finished, or at its limits of time or size, and its trailer carries a cursor to continue from. The Python and TypeScript clients and the `mosaica` command follow the cursor for you. A client that stops reading for too long is cut off without a trailer. The clients then stop with an error that carries the cursor of the last page end received, from which the caller can read on. A page with no page end after it is discarded.

## When the server is busy

Each kind of request has its own limit on how many run at once, so a long export cannot starve the map:

- **Map and item requests** are refused at once when every place is taken, and otherwise wait briefly for computing capacity.
- **Bulk reads** run two at once by default. One more is refused at once.
- **Annotations by tile** run one for each compute thread by default, with as many again waiting.

A request refused for load is answered `429` with a one-second wait. An insert refused because too many items are waiting gets a wait worked out from how fast the waiting items are being written. A deletion or suppression is never refused for load, as [changing data](write-path.md#deleting-and-suppressing) explains.

Two other `429`s can appear on a quiet server. Both ask the client to retry after the wait given:

- another request is already building this grant's state, described [below](#what-the-server-keeps-ready), and the wait for it ran out;
- the server is rebuilding the grant's state after a merge, and the viewer has already been shown the view.

What retries for you:

- **The TypeScript store** retries a map, annotations, aggregate or suggestion request refused with `429` or `503`. It does not retry bulk reads.
- **`Control`**, in both clients, retries a refused write.
- **The Python client's reads** and **the `mosaica` command** retry nothing.

A map request keeps its place until its whole answer has been sent, but gives up its share of the server's computing as soon as its counts are worked out. A slow reader therefore holds up no one else's work. When a client goes away, the server stops working on its answer.

## What the server keeps ready

Viewers who hold the same access labels share a **grant**, and the server keeps the expensive state per grant rather than per viewer or per request:

- **The visible set**: which items the grant may see, arranged over the map's stored rows. It is built on the grant's first request. The list of items is written to disc and read back at a restart. Its arrangement over the rows is held in memory only, so each grant's first request after a restart builds it again.
- **Each annotation level's figures**: every artifact's count, centre and box over the visible set. They are built the first time the grant reads that level.

Every later request, from any session with the grant, reuses them. They are corrected on each request for whatever has been deleted, suppressed or ingested since they were built. The figures are written to disc too, so a restart reads them back rather than building them again.

The first request from a new grant is therefore the slow one. Its cost grows with how much of the corpus the grant may see. A deployment whose viewers fall into a few groups pays it a few times. A deployment where every user has labels of their own pays it once for each user. The [operating guide](../guides/operating.md#annotation-layers-and-new-viewers) explains how to plan for it.

When new data is flushed or segments are merged, a background pass brings each grant's visible set up to date. Until it reaches a grant, a viewer already shown the view is answered from the previous version after a flush, so new items appear a moment later, and is asked to retry after a merge. A session's first read in the view does the update itself. After a [compaction](write-path.md#background-maintenance), the same pass rebuilds each grant's visible set, and each level's figures are rebuilt for the compacted part of the data the next time they are read.

The data itself is mapped from disc, as the [overview](overview.md#how-it-serves-large-data) describes, so the parts of the map people are looking at stay in memory.

## What a client may already hold

Each map answer, and each answer of annotations by tile, carries three values a client can compare with what it holds:

- **The identity key**, `x-mosaica-identity-key`, says whether a held answer may still be shown at all. It changes when the viewer's access, their visible set or the view changes, and when the database is rebuilt, which gives every item a new `mosaica_id`.
- **The content key**, the `ETag`, changes whenever the data has changed in a way that could add something the client does not have.
- **The generation**, `x-mosaica-pin`, names the version of the data the answer came from. A request that echoes it gets `x-mosaica-stale` in reply, saying whether the data has moved on since.

A client saves work by leaving out the tiles it already holds. The server does nothing for a tile a request does not name.

## Health and readiness

The viewer and session addresses answer `/healthz` and `/readyz`, with no credential. `/healthz` says the process is answering. `/readyz` says whether to send it traffic: it fails when the part of the server that takes writes is not running normally, or when the server has opened an older state of the data because the newest failed its checks.

A server whose write-ahead log has failed, or that has opened an older state of the data, is not ready but still applies deletions and suppressions. Stop sending it ordinary requests, but keep sending it those, and do not restart it because a probe failed: a deletion or suppression that was applied but could not be written to the log is lost at a restart, and the item comes back. Once the log is repaired, `/readyz` reports ready again, but no new items become visible until a restart, and the server logs an alarm saying so. The operating guide explains [what to do when a write to the log fails](../guides/operating.md#when-a-write-to-the-log-fails). A disc that hangs on a write, rather than failing it, leaves `/readyz` reporting ready. The [operating guide](../guides/operating.md#health-and-readiness) covers the checks.

## Not built yet

- **A route for tile-based map tools.** MapLibre, OpenLayers and QGIS ask for one tile at a time. An adapter can turn each tile into a map request of its own, at the cost of one evaluation per tile instead of one for the whole screen.
- **Replicas.** A deployment is one server.
- **Keeping a filter's result between requests.** Panning or zooming with a filter applied evaluates it again on each request.
- **Telling the server what a client already holds.** A client can leave out whole tiles, but cannot say which points of a tile it has. Every tile it names is answered in full.
