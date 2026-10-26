<!-- The pages under typescript/ are generated from the doc comments in clients/ts. Edit the comments, then run: node clients/ts/scripts/reference.mjs -->

# TypeScript client

The TypeScript client puts a Mosaica map in a web page. It is four packages built around one object, the store. The store holds the state of a map for one viewer. You tell it where the camera is, what to filter and which layers to draw. It fetches what that needs from the server and publishes the result as projections, read-only snapshots such as the points to draw or the counts in view. The other packages draw from a store. `@mosaicajs/deck` is a deck.gl layer for an app that builds its own scene, `@mosaicajs/components` gives ready-made elements, and `@mosaicajs/react` wraps both for React.

| Package | What it holds |
|---|---|
| [`@mosaicajs/client`](typescript/client.md) | The store (`createStore`), the HTTP client `MosaicaClient` it is built on, the control-plane client `Control`, and helpers for filters, layers, coordinates, colours and counts. It has no DOM dependency. |
| [`@mosaicajs/deck`](typescript/deck.md) | `MosaicaLayer`, a deck.gl layer that draws a store, for an app that builds its own `Deck`. deck.gl and luma.gl are peer dependencies. |
| [`@mosaicajs/components`](typescript/components.md) | The types of the `mosaica-*` elements' events, parts, context and theme. The elements themselves are on the [Components](components.md) pages. |
| [`@mosaicajs/react`](typescript/react.md) | `useMosaicaStore` and `useProjection`, which make and read a store from React. |
| [`@mosaicajs/react/components`](typescript/react/components.md) | Every element as a React component, with typed props and event handlers. |

Each page lists what its package exports, and every name on it is imported from that package, as in `import {createStore} from '@mosaicajs/client'`. The `/internal` entries of `@mosaicajs/client` and `@mosaicajs/deck` are shared between the packages and are not a public API.

Not built yet: the packages are not published to npm. The registry holds an empty placeholder, version 0.0.0, under each name, so `npm install @mosaicajs/client` succeeds and installs no Mosaica code. Build and pack the packages from `clients/ts` instead, and install the tarballs together in one `npm install`, since `@mosaicajs/deck` and `@mosaicajs/components` depend on the exact version of the Mosaica packages they import, which exists only in those tarballs. The README in `clients/ts` gives the commands and each package's peer dependencies.

## Connecting

Pass `createStore` the viewer server's base URL as `viewerUrl`, and either a fixed `token` or an `authorise` function that returns one. To share one HTTP client between stores, create a `MosaicaClient` and pass it as `client` instead of `viewerUrl`.

`MosaicaClient` takes two URLs. `viewerUrl` is for the map routes and for `login` and `logout`. `sessionUrl` is for `authorise` and `revoke`, which a back end uses to start and end sessions for someone else; both reject without it. A page that is given a token only needs `viewerUrl`.

## Reading the items behind the map

`store.items` reads the items that match what the map shows, one page at a time, as Arrow tables. You choose the `fields`, the `order` and the page size. The store fills in its token, its current view and its filters, and `inView: true` limits the read to the selected region, or to the camera's box if no region is selected:

```ts
const read = await store.items({fields: ['name', 'country'], pageRows: 25, count: true, inView: true});
const first = await read.next();   // read.head.matched counts every match
const second = await read.next();  // the next 25
```

Pages are fetched only as you ask for them. A read keeps the filters it started with, so start a new one when the filters change. Pass `filters: null` to read every item the viewer may see. With `inView`, a call made before the map has a camera or a region waits for one, so a page can ask for its list as soon as it loads.

## When the server is busy

A server too busy to take a request answers `429 backpressure`, with a `Retry-After` header giving the seconds to wait. What each part of the client does with it:

| Part | On a `429` |
|---|---|
| `MosaicaClient` | Does not retry. The call rejects with a `MosaicaError` whose `retryAfterS` is the wait in seconds, at least 1; `suggest` returns `{status: 'shed', retryAfterS}` instead. Wait and call again with the same arguments. A paged read refused part-way can continue from its `cursor`, and an `aggregate` refused part-way rejects with a `PartialAggregate` that holds the pages read so far. The [`MosaicaClient`](typescript/client.md#mosaicaclient) entry has an example. |
| `Control` | Waits the `Retry-After`, between 1 and 30 seconds, and sends the request again, for up to five minutes in total. After that the `429` is returned as the call's `Answer`. |
| The store | Retries a map request or an aggregate twice, waiting 1 s and then 2 s, or the `Retry-After` if longer, up to 8 s. Meanwhile the view's `status`, or the aggregate's, is `retrying`; after the last attempt it is `refused`, and the components show Server busy. A typeahead request is retried up to five times; after that `filters.suggestErrors[column]` holds a `backpressure` refusal. An item's record is not retried, and the refusal lands in `selection.itemRefusal`. |
