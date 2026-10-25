/**
 * Development only. A Vite plugin that starts `mosaica serve` over the GeoNames sample in
 * `examples/data/geonames/`, passes the page's requests under `/mosaica` on to it, and mints
 * viewer tokens for the page at `/dev/token`.
 *
 * The bundle is built on the first run, into `mosaica-demo/getting-started/` at the root of the
 * repository, or under `MOSAICA_DEMO_DIR` where that is set. The server listens on ports the system
 * chooses and stops when this process ends. Its operator credential is made up for each run and
 * never leaves this process. Each token it mints reads every place.
 */
import {spawn} from 'node:child_process';
import {randomBytes} from 'node:crypto';
import {existsSync, mkdirSync, openSync, writeFileSync} from 'node:fs';
import {delimiter, dirname, join, resolve} from 'node:path';
import {createInterface} from 'node:readline';
import {fileURLToPath} from 'node:url';
import type {Plugin} from 'vite';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../../../../..');

/** A running Mosaica server: the addresses it listens on, its operator credential, and a way to stop it. */
type Mosaica = {viewer: string; session: string; credential: string; stop: () => Promise<void>};

/** The `mosaica` binary: `MOSAICA_BIN`, else this checkout's release build, else one on `PATH`. */
export function findMosaica(): string | null {
  if (process.env.MOSAICA_BIN) return process.env.MOSAICA_BIN;
  const candidates = [join(repo, 'target/release/mosaica'), ...(process.env.PATH ?? '').split(delimiter).map((dir) => join(dir, 'mosaica'))];
  return candidates.find((path) => existsSync(path)) ?? null;
}

/** Build the bundle where there is none, start the server, and resolve once it listens. */
async function start(): Promise<Mosaica> {
  const binary = findMosaica();
  if (!binary) throw new Error('no mosaica binary. Build one with `cargo build --release -p mosaica` at the root of the repository, or set MOSAICA_BIN to one.');
  const dir = join(process.env.MOSAICA_DEMO_DIR ?? join(repo, 'mosaica-demo'), 'getting-started');
  mkdirSync(dir, {recursive: true});
  const deployment = join(dir, 'mosaica.toml');
  writeFileSync(
    deployment,
    `[bundle]
path  = "bundle"
cache = "cache"
wal   = "wal.log"

[build]
schema = "${join(repo, 'examples/data/geonames/schema.toml')}"

[disclosure]
token_max_lifetime = 3600

[serve]
viewer  = "127.0.0.1:0"
session = "127.0.0.1:0"
control = "127.0.0.1:0"
operator_credential_env = "MOSAICA_OPERATOR_CRED"

[catalogue]
dir = "catalogue"
`
  );
  const log = join(dir, 'mosaica.log');
  const output = openSync(log, 'a');

  if (!existsSync(join(dir, 'bundle/CURRENT'))) {
    console.log('  building the GeoNames bundle, which takes a few seconds');
    const built = await new Promise<number | null>((done, fail) =>
      spawn(binary, ['build', '--deployment', deployment], {stdio: ['ignore', output, output]}).on('error', fail).on('exit', done)
    );
    if (built !== 0) throw new Error(`mosaica build failed; its output is in ${log}`);
  }

  const credential = randomBytes(24).toString('hex');
  // The server stops when its standard input closes, which happens however this process ends.
  const server = spawn(binary, ['serve', '--deployment', deployment, '--exit-on-stdin-close'], {
    stdio: ['pipe', 'pipe', output],
    env: {...process.env, MOSAICA_OPERATOR_CRED: credential}
  });
  // Once all three addresses are bound, it prints one line of JSON naming them.
  const {viewer, session} = await new Promise<{viewer: string; session: string}>((ready, fail) => {
    server.on('error', fail);
    server.on('exit', () => fail(new Error(`mosaica serve stopped while starting; its output is in ${log}`)));
    createInterface({input: server.stdout!}).once('line', (line) => ready(JSON.parse(line)));
  });
  console.log(`  Mosaica is serving 150,000 GeoNames places (log: ${log})`);
  const stopped = new Promise<void>((done) => server.on('exit', () => done()));
  const stop = () => {
    server.stdin!.end();
    return stopped;
  };
  return {viewer: `http://${viewer}`, session: `http://${session}`, credential, stop};
}

/** The server this process started. A restart of the dev server keeps it. */
export function mosaica(): Promise<Mosaica> {
  const shared = globalThis as {mosaicaDev?: Promise<Mosaica>};
  return (shared.mosaicaDev ??= start());
}

/** The plugin: Mosaica beside the dev server, under `/mosaica`, and `GET /dev/token`. */
export function mosaicaDev(): Plugin {
  return {
    name: 'mosaica-dev',
    apply: 'serve',
    async config() {
      const {viewer} = await mosaica();
      return {server: {proxy: {'/mosaica': {target: viewer, rewrite: (path) => path.replace(/^\/mosaica/, '')}}}};
    },
    configureServer(vite) {
      vite.middlewares.use('/dev/token', async (_req, res) => {
        const {session, credential} = await mosaica();
        const answer = await fetch(`${session}/session/authorise`, {
          method: 'POST',
          headers: {authorization: `Bearer ${credential}`, 'content-type': 'application/json'},
          body: JSON.stringify({read_all: true})
        });
        const {token, expires_at} = (await answer.json()) as {token: string; expires_at: number};
        res.setHeader('content-type', 'application/json');
        res.end(JSON.stringify({token, expiresAt: expires_at}));
      });
    }
  };
}
