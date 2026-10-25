# Getting started: a map in your web page

The finished project of the first TypeScript tutorial, [Your first map in a web page](../../../../docs/start/typescript.md). `mosaica/` holds the server's settings and `web/` the Vite project, with the files the tutorial writes. `web/package.json` asks for any version of `@mosaicajs/components`, which inside this repository is the workspace's build in `dist/`, so run `npm run build` in `clients/ts` first. A project of your own names the version it installed, as npm writes it. The package also adds the live test below.

To run it, start Mosaica in `mosaica/` with a `mosaica` binary on your `PATH`:

```bash
cd mosaica
mosaica build --config ../../../../../examples/data/geonames/schema.toml
mosaica serve
```

Then, in a second terminal, start the page and open <http://localhost:5173/>:

```bash
cd web
npm run dev
```

The tutorial explains each step. `mosaica/.gitignore` keeps what the build and the server write out of git.

`npm run test:live` in `web/` builds the sample into a temporary directory, serves it in open mode on ports the system chooses, sends the page's requests for port 9141 there, and checks in headless Chromium that the page draws every place and that the explorer fills its part of the layout. It needs `npm run build` in `clients/ts` and a `mosaica` binary in `MOSAICA_BIN`, this checkout's `target/release`, or on `PATH`.
