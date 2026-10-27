# Security

Mosaica computes every figure a viewer receives from the items that viewer may see: every count, density, cluster, label, sample and breakdown, not only which items they can open. This chapter sets out exactly what that guarantees, whom it trusts, what a viewer can still learn, what is not claimed, and how the claims are checked. [Access control](access-control.md) describes the labels, access attributes and sessions themselves, and the full [threat model](https://github.com/mosaicadb/mosaica/blob/main/docs/dev/threat-model.md) states each claim in detail, with its evidence.

## What is guaranteed

Everything rests on a viewer's **visible set**: the items their access attributes admit, less every item currently deleted or suppressed. Each request works it out afresh before anything reads it, and every answer is computed from inside it.

- **Counts, densities and breakdowns** are taken over the visible set, never over everything and then filtered. A filter narrows what is drawn and counted within the visible set, and cannot widen it.
- **Samples are drawn from the visible set.** A viewer who may see few items gets a full sample of those, not a sample of everything with most of it removed.
- **Annotations** are served by their layer's label, their own label, and how much of their membership the viewer can see, all tested on the visible set, never the filtered set. Content declared `require_member_visibility = "all"`, such as a name generated from an artifact's members, is served only to a viewer who can see every member it was made from. A topic's count, centre, outline and colour are worked out from what the viewer is served.
- **Deletions and suppressions** apply to every request that starts after the server accepts them.
- **A hidden value answers as one that does not exist.** A category value, a unique field's value, an item's or an artifact's `mosaica_id`, a layer or a view that the viewer may not see gives the same answer, with the same status, as one that was never there.
- **Internal identifiers never leave the server.** A client sees each item's `mosaica_id`, a keyed scrambling of the server's own number for it, so two `mosaica_id`s say nothing about how close their items are. A bulk read's cursor is encrypted, and opens only in the read it came from and in a session with the same access.
- **An incomplete answer is marked.** A streamed answer that stops early lacks its trailer, so a client can tell it from a complete one.

## Who is trusted

| Party | Trusted | What it can do |
|---|---|---|
| A viewer | No | Holds a token and can make any number of requests. The guarantees above hold against them |
| Client code | No | Computes nothing about what is visible. A bug in it can only misdraw what the server sent |
| A writer with `write` but not `write-all` | Within its attributes | Writes only to the items, layers and artifacts its attributes reach, except that deleting an artifact also deletes the artifacts attached to it. It can learn that a unique value or an artifact key it tried to set is held outside its attributes, and nothing else about the holder |
| Your server, with an `authorise-as` key | Yes | Can start a session as any user, holding that user's attributes |
| A user with `admin` | Yes, as the operator | Can grant itself any permission. A role mapping from an OIDC provider to a group holding `admin` extends that trust to the provider |
| An OIDC provider | Yes | Its claims are taken as it asserts them. A change at the provider reaches a session when the session's token expires |
| The operator | Yes | Holds every permission |
| Anyone holding the database's files | Yes | Has everything the server has. Nothing here defends against them |

Three facts bear on how you deploy it. The viewer address serves plain HTTP, and passwords, API keys and session tokens all cross it, so outside a private network put [TLS](../guides/tls.md) in front of it. Mosaica refuses to start with an empty operator credential. And password login is limited per name: by default, ten wrong attempts in fifteen minutes lock that name out of password login until the oldest attempt leaves the window, while its API keys and sessions carry on working. The count is held in memory, so it starts again at a restart.

An open server (`serve.open`) is the exception to all of this. It serves every request as the operator with no credential, so anything on its machine that can reach a loopback address reads and changes everything. It listens on loopback alone, and starts only with no user, provider or CORS origin list configured. It refuses a request whose `Host` doesn't name this machine, a request a browser sends from a page on another site, and a write sent with no content type or as plain text or form data, which such a page could send without the browser asking the server first.

## What a viewer can still learn

A viewer can still learn a little beyond their own items, through the channels below. None of them reveals a field or a label of an item the viewer may not see.

- **Response times.** A request takes longer the more work it does, including over items the viewer cannot see. So timing hints at how much of the database lies outside their set, whether a value they can name exists somewhere, how widely, and how much is being written or deleted. One part of this, how a tile's time relates to the viewer's own count in it, has not been quantified.
- **Shared work.** Sessions with the same access share some cached work, and on a public deployment that is every anonymous viewer. How quickly a first request is answered can show whether another such session recently made a similar one, and, weakly, how busy sessions with other access are.
- **Stable identifiers.** A `mosaica_id` stays the same for an item's life. A viewer can tell when an item they had seen was deleted or suppressed, and two viewers comparing `mosaica_id`s can tell they are looking at the same item.
- **Stored order.** A bulk read in stored order groups the viewer's items by the full set of labels they carry, which can show that some carry a label the viewer does not hold, and roughly when each arrived. It can also show that an item between two of theirs was deleted. Map order returns the same rows and fields and shows none of this.
- **Dense category codes.** Where a vocabulary's values are numbered densely, the largest code a viewer can see bounds how many values exist.
- **Grouping in a region.** A layer's minimum visible count decides whether a grouping is shown at all. It does not protect counts, since the map serves exact counts over any region the viewer names.

## What is not claimed

- **Strength of the `mosaica_id` scrambling.** It is not encryption, and is not meant to resist someone who has pairs of internal numbers and `mosaica_id`s. No route gives out such a pair.
- **Protection of data at rest.** These claims are about what a viewer can learn from answers. Files on disc, and anyone holding them, are outside them.
- **Checking what an operator declares.** A label, a default label or content the operator declares is served as declared. An item with no label of its own takes its view's default, and is refused where the view has none.
- **A quantified timing channel.** Timing is described above, and its per-tile part is open.
- **Isolation between partitions.** Not built yet: a deployment is one partition.

## How the claims are checked

The claims are checked by the [conformance suite](https://github.com/mosaicadb/mosaica/tree/main/conformance), which runs Mosaica alongside an independent implementation and compares their answers:

- **Counts, tiles and density** are compared, value for value, across planted states and users who see different parts of the data.
- **Annotations** are compared tile by tile, before and after a suppression. A server test also shows that an artifact withheld by its own label leaves every answer exactly as if it had never been published.
- **Samples, histograms, breakdowns and field figures** are compared with figures worked out from each user's own visible items.
- **Access expressions** are checked over fifteen users, comparing which items each one reads and the labels shown beside them.
- **Identifiers** are scanned for: every answer, cursor and log line is searched for internal numbers, with a planted case in each scan to show the scan itself works. Counts, coordinates, text and logs are searched only for numbers of 100,000 and above, and cursors for 8-byte values.
- **One shard against several**: each corpus is built both ways, and the answers are compared, through ingests, edits, deletions, a compaction and a restart. Samples drawn by rank, and figures kept per shard, are not compared.

Some things the suite doesn't cover:

- No scan measures response times.
- The independent comparison of tiles and counts runs over corpora whose labels are single attributes. Labels with conjunctions are covered by the access-expression comparison and by Rust tests.
- A hidden value answering as one that does not exist, membership requirements other than "all", and which rows a bulk read serves are checked by Rust tests, not against the independent implementation.

## Reporting a vulnerability

Report a suspected vulnerability privately, as [`SECURITY.md`](https://github.com/mosaicadb/mosaica/blob/main/SECURITY.md) describes. A report is most useful when it names the claim that is broken.
