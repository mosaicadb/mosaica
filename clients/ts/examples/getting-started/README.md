# Getting started: a map in your web page

The code of the first TypeScript tutorial, [Your first map in a web page](../../../../docs/start/typescript.md). The tutorial walks through it step by step; this file only says how to run it.

From `clients/ts` in a clone of the repository:

```bash
npm ci
cd examples/getting-started
npm start
```

Then open <http://localhost:5310/>. `npm start` needs a `mosaica` binary: build one with `cargo build --release -p mosaica` at the root of the repository, or set `MOSAICA_BIN` to one. On the first run it builds the GeoNames sample in `examples/data/geonames/` into `mosaica-demo/getting-started/` at the root of the repository. Ctrl-C stops Mosaica and the page together.

`npm test` builds the page and checks the output. `npm run test:live` starts it as `npm start` does and checks in a browser that every place is drawn.
