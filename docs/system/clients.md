# Clients

Mosaica has three clients beside the HTTP API: a Python package, a TypeScript client with web components, and the `mosaica` command. Each reaches the same capabilities, as the [capabilities](../reference/capabilities.md) table lists. This chapter explains what each one does for you, what a browser client holds and when it lets go of it, and the rules for anyone writing a client of their own.

Every figure a client shows was computed by the server, from the items the viewer may see, before it arrived. A client cannot widen or narrow what a viewer sees. What it can get wrong is how it presents an answer: a sample shown as the whole set, an old view shown as current, or a refusal shown as an empty map.

## Which client

- **Python**, the `mosaica` package, makes a database from data frames and files, reads one as pyarrow tables, and draws a map in a Jupyter notebook. `create` and `open` give a `Database` in a directory, which runs its own server. `connect` gives a `Viewer` that reads from a server someone else runs, and `Control` writes to one. The notebook map embeds the TypeScript components, so it behaves as they do.
- **TypeScript**: `@mosaicajs/client` holds the store, which runs a map for a browser, and `MosaicaClient` and `Control`, which send single requests. `@mosaicajs/components` adds the web components, from a whole explorer down to a single count, `@mosaicajs/deck` a deck.gl layer for a scene of your own, and `@mosaicajs/react` React wrappers.
- **The `mosaica` command** builds a database from a declaration and its source files, serves it, and reads from a running server with a session token: items, artifacts and aggregates, as Arrow or Parquet.

## Connecting from a browser

A browser holds a token for one viewer, and calls the viewer address with it. The token comes from one of two places:

- **The viewer logs in** at the viewer address, with a password, an API key or an OIDC access token.
- **Your own server** asks the session address for a token for a viewer it has already signed in. That address takes an API key that may act as any viewer, so it must never be reached from a browser.

The viewer address answers browsers only from origins the deployment lists, and from none by default. [Access control](access-control.md#getting-a-token) describes both routes in full.

!!! warning "Two integrations that look right and are not"
    - **One broad token for everyone**, with your server filtering per user afterwards. Every user then sees counts, clusters and labels computed for the broad token, which can reveal what they should not see.
    - **A token minted from whatever a caller claims**, so anyone who can call your minting endpoint can be any user. Check the user's identity on your server before asking for their token.

## What the store does

The store is the part of the TypeScript client that runs a map. The components and the notebook map are built on it, and a host with its own drawing can use it directly. The host tells it where the camera is, and which filters, layers, colours and view are chosen. The store works out which requests to make, holds what comes back, and publishes what to draw: the points, the counts, the topics, a legend, and the state of the view.

- **It asks only for what is worth asking.** It picks the zoom level whose tiles give about as many points as the point budget, 250,000 by default. It waits for the camera to settle before asking for more, and fetches a margin around the screen once it has.
- **It keeps what it has fetched**, and asks again only for tiles it does not hold. Topics are fetched by their own request, tile by tile, so they do not blink out as the held points change.
- **It keeps colours stable.** A topic's colour comes from a slot the server gives it, the same for this viewer at every zoom and filter, from one of four palettes: Okabe-Ito, Tableau 10 (the default), Tableau 20 or Kelly. A category's values take their palette colours in order of the server's count of each over the viewer's items, so they are the same on every load. Colours a user picks are handed to the host in events, and the client keeps none of them. A host that wants them kept stores them and passes them back.
- **It picks a level.** On a layer with levels, a map with no level chosen colours, names and counts the topics at the top level served at the camera's zoom.
- **It draws density from the aggregate route.** Shading follows the filter and the highlight, as the points do. The scale is logarithmic by default, since counts on most maps are skewed.
- **It counts for the panels.** A component registers the breakdowns it needs, such as a histogram for a field card, and the store sends them as one aggregate request and asks again when the filter changes.

### What the view shows

The store reports the view's state, so a host never shows one kind of answer as another:

| State | Meaning |
|---|---|
| `idle` | nothing asked for yet |
| `loading` | a request is out |
| `retrying` | the server was busy (`429` or `503`), and the request will be sent again |
| `shown` | points are on screen. Only this state may show counts |
| `empty` | the answer holds nothing this viewer may see in view |
| `refused` | the request failed, so what is here is not known |

Two flags sit beside the state:

- **`stale`**: the data has changed since the points on screen were fetched. The store does not fetch again by itself. It marks the view, and a refresh is one action away.
- **`unreachable`**: a request could not reach the server, or a proxy in front of it answered `502`. The store asks the server again after a second, then twice as long each time, up to 30 seconds, and redraws once it answers. A refusal, a `504` on one route, or a token source that refuses, is not an outage. A `401` on a token that had worked means the session ended, and the store asks the host's `authorise` for a new token once.

### When a client lets go of what it holds

| What happened | What the store does |
|---|---|
| The data changed under the same viewer | Marks the view `stale` and keeps what is on screen until it is refreshed |
| The answers carry a different identity key: another viewer, a renewed token, or a compaction | Drops everything the server answered, reads the deployment's description again and redraws. It keeps the host's own choices |
| The host calls `clear()` | The same, for a host that changes viewer itself |
| The database was rebuilt | Every `mosaica_id` it holds names nothing. Read the items again by a unique field's values |

A merge of segments never reaches a client. A compaction reaches it only through the identity key, and every `mosaica_id` stays valid across it.

### Views

The store keeps what it has fetched for each view it has visited, so a view the viewer returns to is as they left it. All views share one memory budget, and the least recently drawn leave first.

- **Within a view group sharing one layout**, such as a year shown quarter by quarter, a switch keeps the camera and the selection, and asks the next view for the same tiles.
- **Between layouts**, a switch drops the selection and fits the camera to the new view, since a position in one layout means nothing in another.

## Reading data out

Bulk reads and aggregates do not go through the store. Each client sends the request as the caller wrote it, choosing no fields, order or figures on the caller's behalf, and follows the cursor:

| Client | Bulk read | Aggregate |
|---|---|---|
| Python | `db.items`, `db.artifacts`, `selection.items`, and the same on a `Viewer`: one pyarrow table, or pages with `batches=True` | `db.aggregate`, `viewer.aggregate`, `selection.aggregate`: one pyarrow table per grouping |
| TypeScript | `client.items`, `client.artifacts`: Arrow tables, one per page | `client.aggregate`, or `store.setAggregate` for counts that follow the map |
| CLI | `mosaica items`, `mosaica artifacts`: Arrow IPC or Parquet, written page by page | `mosaica aggregate`, one grouping a run |

A read that stops part of the way gives back the rows it read and the cursor to continue from: `PartialRead` in Python, the cursor after each page in TypeScript, and the cursor named on stderr by the command.

## Writing your own client

The server enforces what a viewer may see, but it cannot see a screen. These rules are what a client must do itself. The store follows all of them.

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

- **Animating items between layouts.** A switch between views with different layouts redraws rather than moving each item to its new position.
- **Fetching a view before it is chosen.** A view the viewer has not visited is fetched when they switch to it.
- **An adapter for tile-based map libraries.** The store asks for an area at a time, and nothing sits between it and a library that asks for one tile at a time.
- **Keeping the notebook map's token off the page.** The widget's token reaches the browser page itself.
