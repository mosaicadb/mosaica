# Your own interface around the map

The finished project of the second TypeScript tutorial, [Your own interface around the map](../../../../docs/start/typescript-own-ui.md). `mosaica/` holds the server's settings and `web/` the Vite project, with the files the tutorial writes. `web/package.json` asks for any version of `@mosaicajs/client` and `@mosaicajs/components`, which inside this repository are the workspace's builds in `dist/`, so run `npm run build` in `clients/ts` first. A project of your own names the versions it installed, as npm writes them. The package also adds the live test below.

To run it, start Mosaica in `mosaica/` with a `mosaica` binary on your `PATH`:

```bash
cd mosaica
mosaica build --config ../../../../../examples/data/arxiv/schema.toml
mosaica serve
```

Then, in a second terminal, start the page and open <http://localhost:5273/>:

```bash
cd web
npm run dev -- --port 5273
```

The tutorial explains each step. `mosaica/.gitignore` keeps what the build and the server write out of git.

`npm run test:live` in `web/` builds the sample into a temporary directory, serves it in open mode on ports the system chooses, sends the page's requests for port 9241 there, and checks in headless Chromium that the page draws every paper, that the archive list fills from the server, that choosing an archive and typing in the title box change the count and the list of titles, and that opening a topic filters to it. It opens the topic through the store rather than with a click: under headless Chromium's software renderer, deck.gl's pick on a press takes about 300 ms, past the time a press may take to count as a click. It needs `npm run build` in `clients/ts` and a `mosaica` binary in `MOSAICA_BIN`, this checkout's `target/release`, or on `PATH`.
