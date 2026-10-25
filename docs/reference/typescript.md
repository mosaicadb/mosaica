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

## When the server is busy

A Mosaica server too busy to take a request answers `429 backpressure` with a `Retry-After` header, the number of seconds to wait before sending the same request again. What each part of `@mosaicajs/client` does with it:

| Part | On a `429` |
|---|---|
| `MosaicaClient` | Sends nothing again. The method rejects with a `MosaicaError` whose `status` is `429`, `code` is `backpressure` and `retryAfterS` is the wait in seconds: the `Retry-After` header, else the body's `retry_after_s`, else 1, and never less than 1. `suggest` returns `{status: 'shed', retryAfterS, detail}` instead. To retry, wait `retryAfterS` seconds and call the method again with the same arguments. The [`MosaicaClient`](typescript/client.md) entry has an example. An `items` or `artifacts` read refused part-way reads on from its `cursor`. An `aggregate` refused part-way rejects with a `PartialAggregate` holding the pages it has read, the refusal's `status` and `retryAfterS`, and the cursor to read on from in `result.next`. |
| `Control` | Waits the `Retry-After`, at least 1 s and at most 30 s, and sends the request again while the waits total at most five minutes. The `429` whose wait would take them past it is returned as the call's `Answer`, whose `attempts` counts the requests sent. |
| The store from `createStore` | Sends a map request or an aggregate again up to twice, after 1 s doubled each attempt or the `Retry-After` where that is longer, and at most 8 s; `driver.maxRetries` and `driver.retryBackoffMaxMs` in its options change both. The view's `status`, or the aggregate's, is `retrying` meanwhile, then `refused` with the `429` as its `refusal`, which the components show as Server busy. A typeahead ask is sent again up to five times; after that, one typed in a search box lands in `filters.suggestErrors` as `backpressure`. An item's record is asked for once, and a `429` lands in `selection.itemRefusal`. |
