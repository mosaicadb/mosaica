# Clients

Mosaica has three clients beside the HTTP API: a Python package, a TypeScript client with web components, and the `mosaica` command. The Python and TypeScript clients reach every core capability. The command builds, serves and reads a database and manages its users, but cannot write to a running server or draw a map. The [capabilities](../reference/capabilities.md) table names the call for each capability on each surface.

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

The TypeScript store handles these, and a client built on the HTTP API alone has to handle them itself:

- **A `mosaica_id` is a 64-bit integer.** A JavaScript `number` is exact only up to 2^53, so read and send ids as `bigint`s or as strings of digits.
- **Keep `k` steady or raise it as the viewer zooms in.** The sample nests across zoom levels only while each tile may draw as many points as before. Lowering `k` drops points the viewer was already looking at.
- **Choose the zoom level yourself.** Nothing in a request or an answer states the right depth. `/v1/meta` publishes the sampling settings under `selection`, from which a client can work out how many points a level's tiles will draw, and pick the level that fills the screen.
- **Drawn points are a sample.** A tile's `served` count is how many points it drew. Its `visible` and `matched` counts are the totals.
- **Topics come from their own request.** Each point carries its topic's `mosaica_id` in each layer named. The topics' names, counts and positions come from [annotations by tile](queries.md#annotations-on-the-map).
- **Held tiles answer for one filter and one identity key.** Drop them when either changes. A change of content key means the data has moved on, and the held tiles are older than it.
- **A proxy in front of the viewer address must pass Mosaica's response headers through:** `x-mosaica-identity-key`, `x-mosaica-pin` and the `ETag`. Without them, a client cannot tell when to drop what it holds.

## Not built yet

- **Keeping the notebook map's token off the page.** The widget's token reaches the browser page itself.
