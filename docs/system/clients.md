# Clients

Mosaica has three clients beside the HTTP API: a Python package, a TypeScript client with web components, and the `mosaica` command. The Python and TypeScript clients reach every core capability. The command builds, serves and reads a database and manages its users, but cannot write to a running server or draw a map. The [capabilities](../reference/capabilities.md) table names the call for each capability on each surface.

Every figure a client shows was computed by the server, from the items the viewer may see, before it arrived. A client cannot widen or narrow what a viewer sees. What it can get wrong is how it presents an answer: a sample shown as the whole set, an old view shown as current, or a refusal shown as an empty map.

## Which client

- **Python**, the `mosaica` package, makes a database from data frames and files, reads one as pyarrow tables, and draws a map in a Jupyter notebook. `create` and `open` give a `Database` in a directory, which runs its own server. `connect` gives a `Viewer` that reads from a server someone else runs, and `Control` writes to one.
- **TypeScript** runs maps in the browser: a store that drives a map, web components from a whole explorer down to a single count, a deck.gl layer, and React wrappers. `MosaicaClient` and `Control` send single requests. [Maps in the browser](browser-maps.md) explains how the pieces fit. The Python notebook map embeds the same components.
- **The `mosaica` command** builds a database from a declaration and its source files, serves it, and reads from a running server with a session token: items, artifacts and aggregates, as Arrow or Parquet.

## Connecting

Every read carries a token for one viewer, sent to the viewer address. A token comes from one of two places:

- **The viewer logs in** at the viewer address, with a password, an API key or an OIDC access token.
- **Your own server** asks the session address for a token for a viewer it has signed in. That address takes an API key that may act as any viewer, so it is reached only from your server.

Writes go to the control address, with the operator's credential. The viewer address answers browsers only from origins the deployment lists, and from none by default. [Access control](access-control.md#getting-a-token) describes each route in full.

## Reading data out

Each client sends the request as the caller wrote it, choosing no fields, order or figures on the caller's behalf, and follows the cursor:

| Client | Bulk read | Aggregate |
|---|---|---|
| Python | `db.items`, `db.artifacts`, `selection.items`, and the same on a `Viewer`: one pyarrow table, or pages with `batches=True` | `db.aggregate`, `viewer.aggregate`, `selection.aggregate`: one pyarrow table per grouping |
| TypeScript | `client.items`, `client.artifacts`: Arrow tables, one per page | `client.aggregate`, or `store.setAggregate` for counts that follow the map |
| CLI | `mosaica items`, `mosaica artifacts`: Arrow IPC or Parquet, written page by page | `mosaica aggregate`, one grouping a run |

A read that stops part of the way gives back what it read and the cursor to continue from: `PartialRead` for a bulk read in Python; in TypeScript, the cursor after each page of a bulk read, and `PartialAggregate` for an aggregate; and the cursor named on stderr by the command.

## Writing your own client

The server enforces what a viewer may see, but it cannot see a screen. These rules are what a client must do itself. The TypeScript store follows all of them.

| # | Rule | What goes wrong otherwise |
|---|---|---|
| 1 | Show a state that matches what happened, and show a number only in a shown view | A refusal drawn as an empty map looks like a place with no data |
| 2 | Show a sample's size beside the total it was drawn from, or neither | A count of drawn points reads as everything there is |
| 3 | Mark a view stale once the data has moved on, and keep refresh one action away | An old view reads as current |
| 4 | Show a topic's count as what this viewer can see, never as its size | "12,040 of 12,040" asserts a total nobody sent |
| 5 | Treat a missing topic, and a filter that matches nothing, as ordinary answers | Saying "no such value" lets a viewer probe for what is hidden |
| 6 | Fetch topics with their own request, never from held point tiles | Topics blink out of a view that has not moved |
| 7 | Drop what you hold when the viewer changes, and held points when the filter changes | One viewer's points or topics stay on screen for another |
| 8 | When zooming in, never ask for fewer points per tile than before | Points the viewer was looking at disappear |
| 9 | Hold a `mosaica_id` as a 64-bit integer, not a JavaScript `number` | A click opens the wrong record, or none |
| 10 | Have any proxy in front of the viewer address pass every response header through | A cached view can be shown under another viewer's token |
| 11 | Treat a refused credential and an expired token alike: the session has ended | A client retries a request that can never succeed |
| 12 | Choose a zoom level that gives about as many points as the screen can show | The map shows a handful of points, or asks for far more than it can draw |

## Not built yet

- **Keeping the notebook map's token off the page.** The widget's token reaches the browser page itself.
