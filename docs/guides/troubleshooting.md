# Troubleshooting

This page starts from what you see and points to the cause. Most problems fall into three groups: the map shows less than you expect, a request is refused, or the server is slow.

## The map shows less than you expect

### The map is blank, or says Nothing in view

Mosaica draws only what the viewer may see, so a blank map usually means the viewer's session admits none of the items. Check, in this order:

1. **The items' access labels.** If your data has no access rules, every item needs the label `public`, which every session holds. The build's report says how many rows took the view's default label and how many carried one of their own.
2. **The viewer's access attributes.** A user sees an item only when their attributes satisfy its label. List what they hold with `mosaica principal show <name>`.
3. **The view's own label.** A view that the viewer may not see answers as if it didn't exist.
4. **The filter.** Clear it, to rule it out.

A map that draws points but none where you expected them may have the wrong frame: see the next section.

### Points pile up along the edge of the map

A view's frame is fixed at its first load, and a point outside it is drawn on its edge. The build's report and each insert's answer count how many points that moved. Give the view an explicit frame wide enough for all its data, and rebuild. [The frame](../system/views.md#the-frame) explains how an `auto` frame is fitted.

### New items don't appear

New and edited items appear at the next flush, which runs every 90 seconds by default, or sooner once 40,000 rows are waiting. To wait for a write to be visible, ask for it in the write itself, as [When new data appears](../system/write-path.md#when-new-data-appears) describes.

If the items carry an access attribute that no item carried before, a viewer whose session started earlier doesn't see them until they start a new session.

### A user sees fewer items than their attributes admit

The same cause: their session started before the first item with one of their attributes was flushed. Signing in again fixes it.

## A request is refused

Every refusal carries an error code and a `detail` that says what was wrong.

| Answer | What it means | What to do |
|---|---|---|
| `401 bad-credential` | The credential is missing, malformed or unknown. On a session token that worked before, the session has ended and been forgotten, as after a restart | Sign in again, or start a new session |
| `403 expired-token` | The session has ended: it reached its lifetime, was revoked, or a catalogue change affected it | Start a new session |
| `403 forbidden` | The credential lacks a permission, such as a key without `authorise-as`, or a user without `read` | Grant the permission. [Permissions](../system/users-and-roles.md#permissions) lists what each one allows |
| `404 unknown` | No such view, item or artifact, or one the session may not see. The two answer the same | Check the name, then the viewer's access |
| `409 conflict` | A write clashes with what the database holds, such as a unique value another item has | Read the detail, which names the row |
| `422 contract` | The request is malformed, such as a filter on a field that isn't indexed | Change the request. The detail says what is wrong |
| `429 backpressure` | The server is busy | Send the same request again after `Retry-After` seconds |
| `500 fail-closed` | The server couldn't complete the request | Send it again. For a write, the detail says whether part of it may already be in force |
| `503 not-ready` | The part of the server that takes writes isn't running | Read the detail, and check `/control/status`. Some causes last until a restart |

A `400`, `413` or `415` comes back as plain text rather than a JSON error. It means the body isn't valid JSON, is too large for the route, or was sent without `content-type: application/json`.

### The browser reports a CORS error

A page can call the viewer address only from an origin the server admits. Add the page's origin, such as `https://maps.example.org`, to `cors_origins` under `[serve]` in `mosaica.toml`, and restart. For a notebook or a page on `localhost` whose port changes, set `cors_loopback = true` instead.

### The server won't start

The server logs why, in one line, and exits. The commonest causes are an address already in use, an operator credential file it can't read, and a bundle built by a different version of Mosaica, which it refuses. Rebuild the bundle with the version you serve it with.

## The server is slow

### The first request from a new user is slow

The first request from a new set of access attributes works out which items they admit, and its cost grows with how many items that is. Later requests reuse the result. [What the server keeps ready](../system/serving.md#what-the-server-keeps-ready) explains the cost, and [Annotation layers and new viewers](operating.md#annotation-layers-and-new-viewers) how to plan for it.

### Requests slow down under load, or when memory is short

Mosaica maps its files rather than loading them, so the parts in use sit in the kernel's page cache. When the memory cap leaves little room beyond the caches, the server reads from disc more often and slows down before it fails. Compare `heap.anon_bytes` and `heap.file_bytes` in `/control/status` with the cap, and raise the cap or lower the cache limits, as [Memory](operating.md#memory) describes.

### A build runs out of memory

Set `--memory-budget`, such as `mosaica build --memory-budget 24g`. Without it, a build sizes itself from the machine's free memory, which is too much when a server runs beside it.

## When the log fails

If `/readyz` answers 503 and `write_executor.posture` in `/control/status` is `wal-poisoned`, the server couldn't write to its write-ahead log. [When a write to the log fails](operating.md#when-a-write-to-the-log-fails) gives the steps, which matter: a restart at the wrong moment can bring back a deleted item.
