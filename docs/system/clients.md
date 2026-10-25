# Clients

You can reach Mosaica from Python, from TypeScript in the browser, from the `mosaica` command line, or directly over its HTTP API, which the two clients are built on. The Python and TypeScript clients reach every core capability, so the choice between them comes down to where your work happens. The [capabilities](../reference/capabilities.md) table names the call for each capability on each surface.

## Which client

### Python: analysis and prototyping

The Python package is built for data scientists, and for trying an idea out before you build an application. You can use it on your own machine, where `create` makes a database from data frames or Parquet files and the package runs it in the background, with nothing to deploy. Or you can point it at a shared database someone else runs, with `connect` to read and `Control` to write.

Reads, filters and the notebook map are the same calls either way, so analysis you write against a local database runs unchanged against a shared one. Writing is where the two differ: a local database takes inserts and a commit, while a shared one takes its writes through `Control`.

Results come back as pyarrow tables, and the map is a Jupyter widget built from the TypeScript components.

### TypeScript: applications in the browser

The TypeScript client is for maps that other people use. For a map that is an application of its own, the `<mosaica-explorer>` element gives you a whole map, with filters, layers and record cards, on any page, and the single-file bundle needs no build step. To add a map to an application you already have, the map, its panels, the store or the deck.gl layer fit into your own page, in any framework, with React wrappers. [Maps in the browser](browser-maps.md) explains the pieces and how to build on them.

### The command line: operators and scripts

The `mosaica` command builds a database from a declaration and its source files, serves it, declares, inserts, deletes and suppresses on a running server from Parquet, Arrow or NDJSON files, exports items, artifacts and aggregates as Arrow or Parquet, and manages users and their access.

### The HTTP API: anything else

From any other language, call the HTTP API directly. The [HTTP API reference](../reference/http.md) describes every route, and [Call Mosaica from a backend](../guides/backend.md) walks through the ones that answer in JSON.

## Connecting

Every read carries a token for one viewer, which the client sends to the viewer address. The viewer can get that token by logging in there, with a password, an API key or an OIDC access token. Or, if your own server has already signed them in, it can ask the session address for a token on their behalf. The session address takes an API key that may act as any viewer, so only your server should reach it.

Writes go to the control address, with the operator credential, an API key or an OIDC access token. A browser can call the viewer address only from the origins the deployment lists, and by default it lists none. [Access control](access-control.md#getting-a-token) describes each route in full.

## Reading data out

Each client sends a read as you wrote it, choosing no fields, order or figures for you, and follows the cursor until the read is complete:

| Client | Bulk read | Aggregate |
|---|---|---|
| Python | `db.items`, `db.artifacts`, `selection.items`, and the same on a `Viewer`: one pyarrow table, or pages with `batches=True` | `db.aggregate`, `viewer.aggregate`, `selection.aggregate`: one pyarrow table per grouping |
| TypeScript | `client.items`, `client.artifacts`: Arrow tables, one per page | `client.aggregate`, or `store.setAggregate` for counts that follow the map |
| CLI | `mosaica items`, `mosaica artifacts`: Arrow IPC or Parquet, written page by page | `mosaica aggregate`, one grouping a run |

??? info "When a read stops part of the way"
    A read that stops early gives back what it read and the cursor to continue from. In Python, a bulk read or an aggregate raises `PartialRead`. In TypeScript, a bulk read hands back the cursor after each page, and an aggregate raises `PartialAggregate`. The command line prints the cursor on stderr.

## Not built yet

- **Keeping the notebook map's token off the page.** The widget's token reaches the browser page itself.
