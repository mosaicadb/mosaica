# Getting started: a map in your web page

The finished project of the first TypeScript tutorial, [Your first map in a web page](../../../../docs/start/typescript.md). `mosaica/` holds the server's settings and `web/` the Vite project, file for file as the tutorial writes them. `web/package.json` names the packages as they will be on npm. Inside this repository they come from the workspace's built `dist/` directories, so run `npm run build` in `clients/ts` first.

To run it, start Mosaica in `mosaica/` with a `mosaica` binary on your `PATH`:

```bash
cd mosaica
mosaica build --config ../../../../../examples/data/geonames/schema.toml
openssl rand -hex 16 > operator.secret
chmod 600 operator.secret
mosaica serve
```

Then, in a second terminal, start the page and open <http://localhost:5173/>:

```bash
cd web
export MOSAICA_CREDENTIAL=$(cat ../mosaica/operator.secret)
npm run dev
```

The tutorial explains each step. `mosaica/.gitignore` keeps what the build and the server write out of git.

`npm run test:live` in `web/` is the one addition: it builds the sample into a temporary directory, serves it on ports the system chooses, and checks in headless Chromium that the page draws every place and that the explorer fills its part of the layout. It needs `npm run build` in `clients/ts` and a `mosaica` binary in `MOSAICA_BIN`, this checkout's `target/release`, or on `PATH`.
