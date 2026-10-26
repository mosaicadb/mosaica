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

The packages are on npm. Until Mosaica's first full release, install them by the `next` tag, as in `npm install @mosaicajs/client@next`; [Install a client](../start/install.md#install-a-client) says which package to start from. Each package's peer dependencies are below. npm 7 and later installs the required ones itself and leaves the optional ones to the host.

| Package | Peer dependencies |
|---|---|
| `@mosaicajs/client` | None. |
| `@mosaicajs/deck` | `@deck.gl/core`, `@deck.gl/layers` and `@deck.gl/aggregation-layers` `^9.3.7`, and `@luma.gl/core` `^9.3.6`. |
| `@mosaicajs/components` | The same four, and `lit` `^3.3.0`. |
| `@mosaicajs/react` | `react` `^18.0.0` or `^19.0.0`. For the `/components` entry, also `@mosaicajs/components` and `@lit/react` `^1.0.0`, both optional. |

## Connecting

Pass `createStore` the viewer server's base URL as `viewerUrl`, and either a fixed `token` or an `authorise` function that returns one. To share one HTTP client between stores, create a `MosaicaClient` and pass it as `client` instead of `viewerUrl`.

`MosaicaClient` takes two URLs. `viewerUrl` is for the map routes and for `login` and `logout`. `sessionUrl` is for `authorise` and `revoke`, which a back end uses to start and end sessions for someone else; both reject without it. A page that is given a token only needs `viewerUrl`.

## Reading the items behind the map

`store.items` reads the items that match what the map shows, one page at a time as Arrow tables, so that a page can list them beside the map. The store fills in its token, view and filters, and `inView: true` limits the read to the selected region or the camera's box. [`Store.items`](typescript/client.md#items-1) gives the options and an example.

## When the server is busy

A server too busy to take a request answers `429 backpressure`, with a `Retry-After` header giving the seconds to wait. What each part of the client does with it:

| Part | On a `429` |
|---|---|
| `MosaicaClient` | Does not retry, and rejects with a `MosaicaError` that says how long to wait. [`MosaicaClient`](typescript/client.md#mosaicaclient) describes how to retry and gives an example. |
| `Control` | Retries for up to five minutes before returning the `429` as the call's `Answer`, as [`Control`](typescript/client.md#control) describes. |
| The store | Retries a map request or an aggregate twice, waiting 1 s and then 2 s, or the `Retry-After` if longer, up to 8 s. Meanwhile the view's `status`, or the aggregate's, is `retrying`; after the last attempt it is `refused`, and the components show Server busy. A typeahead request is retried as [`Store.suggest`](typescript/client.md#suggest-1) describes. An item's record is not retried, and the refusal lands in `selection.itemRefusal`. |
