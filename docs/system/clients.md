# Clients

Mosaica has three clients beside the HTTP API: a Python package, a TypeScript client with web components, and the `mosaica` command. The Python and TypeScript clients reach every core capability. The command builds, serves and reads a database and manages its users, but cannot write data or declarations to a running server, or draw a map. The [capabilities](../reference/capabilities.md) table names the call for each capability on each surface.

## Which client

### Python: analysis and prototyping

The Python package is designed for data scientists, and for trying an idea out before building an application. It works in two ways, with the same reads, filters and notebook map in each:

- **On your own machine.** `create` makes a database in a directory from data frames or Parquet files, and the package runs a server for it in the background. There is nothing to deploy.
- **Against a shared server.** `connect` reads from a server someone else runs, and `Control` writes to one. Reading and mapping are the same calls on both, so analysis written against a local database runs against a shared one. Writing differs: a local database takes inserts and a commit, and a shared server takes its writes through `Control`.

Results come back as pyarrow tables, and the map is a Jupyter widget built from the TypeScript components.

### TypeScript: applications in the browser

The TypeScript client is designed for maps that other people use:

- **As an application of its own.** The `<mosaica-explorer>` element is a whole map, with filters, layers and record cards, on any page. The single-file bundle needs no build step.
- **Inside an application you already have.** The map, its panels, the store or the deck.gl layer fit into your own page, in any framework, with React wrappers. [Maps in the browser](browser-maps.md) explains the pieces and how to build on them.

### The command line: operators and scripts

The `mosaica` command builds a database from a declaration and its source files, serves it, exports items, artifacts and aggregates as Arrow or Parquet, and manages users and their access.

### The HTTP API: anything else

Both clients are built on the HTTP API, and any other language can call it directly. The [HTTP API reference](../reference/http.md) describes every route.

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

## Not built yet

- **Keeping the notebook map's token off the page.** The widget's token reaches the browser page itself.
