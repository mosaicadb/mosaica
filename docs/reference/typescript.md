<!-- The pages under typescript/ are generated from the doc comments in clients/ts. Edit the comments, then run: node clients/ts/scripts/reference.mjs -->

# TypeScript client

The TypeScript client is four packages in `clients/ts`. Each page below lists what a package exports for a host to use, generated from the doc comments in its source. Every name on a page is imported from that package, as in `import {createStore} from '@mosaicajs/client'`.

| Package | What it holds |
|---|---|
| [`@mosaicajs/client`](typescript/client.md) | The headless store that `createStore` returns, the viewer and session client `MosaicaClient`, the operator's control-plane client `Control`, the bulk read `RecordsRead`, the helpers that build the filter drafts and member clauses the store takes and say which layers it draws and colours by, and the count formatters. |
| [`@mosaicajs/deck`](typescript/deck.md) | `MosaicaLayer`, a deck.gl layer that draws the store, for a host that builds its own `Deck`. deck.gl and luma.gl are peer dependencies. |
| [`@mosaicajs/components`](typescript/components.md) | The types of the `mosaica-*` elements' events, parts, context and theme. The elements are on the [Components](components.md) pages. |
| [`@mosaicajs/react`](typescript/react.md) | `useMosaicaStore` and `useProjection`, the store as React hooks. |
| [`@mosaicajs/react/components`](typescript/react/components.md) | Every element as a React component with typed props and event handlers. |

`@mosaicajs/client/internal` and `@mosaicajs/deck/internal` hold what the other packages use beyond these entries. They are not a public API and are not listed here.

Not built yet: the packages are not published to npm. The registry holds an empty placeholder, version 0.0.0, under each of the four names, so `npm install @mosaicajs/client` succeeds and installs no Mosaica code. `npm run build` in `clients/ts` builds each package into its `dist/` directory, and `npm pack` in a package's directory makes a tarball of it. `@mosaicajs/deck` and `@mosaicajs/components` depend on exactly version 0.1.0 of the Mosaica packages they import, which exists only in those tarballs, so install the tarballs together in one `npm install`. The client README in `clients/ts` gives the commands and each package's peer dependencies.

## Connecting

`createStore` takes the viewer server's base URL as `viewerUrl`, or a `MosaicaClient` as `client`, whose URL it then uses. `MosaicaClient` takes `viewerUrl` for the viewer routes and `sessionUrl` for the session routes. A page that is handed a token uses only the viewer routes and can leave `sessionUrl` out; `authorise` and `revoke` reject without it.

## Reading the items the map shows

`store.items` reads the items this viewer may see that match what the map shows, through `POST /v1/items`. It sends the store's token, its current view and the filters the map's own requests carry: the filter controls, the `member_of` clauses, inside or outside an artifact, and the selected region. `inView: true` narrows the read to the area the counts in view cover, which is the selected region where one is selected and the camera's box where none is. The caller names the `fields`, the `order` and the rows per page. `filters` and `view` may be given to read something else, and `filters: null` reads every item the viewer may see.

It returns the client's `RecordsRead`. Each `next()` gives one page as an Arrow table, and the read takes from the server only as pages are asked for, so a page can show the first rows and read on when the reader asks:

```ts
const read = await store.items({fields: ['name', 'country'], pageRows: 25, count: true, inView: true});
const first = await read.next();   // read.head.matched counts every match
const second = await read.next();  // the next 25, read from the cursor
```

The filters are those in force when `items` is called. A read already open does not follow a later change of filters, so a page that lists matches reads again after one.

## When the server is busy

A Mosaica server too busy to take a request answers `429 backpressure` with a `Retry-After` header, the number of seconds to wait before sending the same request again. What each part of `@mosaicajs/client` does with it:

| Part | On a `429` |
|---|---|
| `MosaicaClient` | Sends nothing again. The method rejects with a `MosaicaError` whose `status` is `429`, `code` is `backpressure` and `retryAfterS` is the wait in seconds: the `Retry-After` header, else the body's `retry_after_s`, else 1, and never less than 1. `suggest` returns `{status: 'shed', retryAfterS, detail}` instead. To retry, wait `retryAfterS` seconds and call the method again with the same arguments. The [`MosaicaClient`](typescript/client.md) entry has an example. An `items` or `artifacts` read refused part-way reads on from its `cursor`. An `aggregate` refused part-way rejects with a `PartialAggregate` holding the pages it has read, the refusal's `status` and `retryAfterS`, and the cursor to read on from in `result.next`. |
| `Control` | Waits the `Retry-After`, at least 1 s and at most 30 s, and sends the request again while the waits total at most five minutes. The `429` whose wait would take them past it is returned as the call's `Answer`, whose `attempts` counts the requests sent. |
| The store from `createStore` | Sends a map request or an aggregate again up to twice, after 1 s doubled each attempt or the `Retry-After` where that is longer, and at most 8 s; `driver.maxRetries` and `driver.retryBackoffMaxMs` in its options change both. The view's `status`, or the aggregate's, is `retrying` meanwhile, then `refused` with the `429` as its `refusal`, which the components show as Server busy. A typeahead ask is sent again up to five times; after that, one typed in a search box lands in `filters.suggestErrors` as `backpressure`. An item's record is asked for once, and a `429` lands in `selection.itemRefusal`. |
