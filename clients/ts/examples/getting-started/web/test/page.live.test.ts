import {spawn, type ChildProcess} from 'node:child_process';
import {mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createInterface} from 'node:readline';
import {chromium, type Browser} from 'playwright';
import {createServer, type ViteDevServer} from 'vite';
import {afterAll, beforeAll, expect, it} from 'vitest';
import {findBinary} from '../../../../core/test/served.js';

/**
 * The tutorial's finished page in headless Chromium, served by Vite, over an open Mosaica server
 * built from the example's `mosaica.toml` into a temporary directory. The server listens on ports
 * the system chooses, and the browser's requests to the page's `viewer-url`, port 9141, are sent
 * there instead. The binary is found as core's live tests find it. Skipped, with the reason, where
 * there is none.
 */
const web = resolve(import.meta.dirname, '..');
const example = resolve(web, '..');
const repo = resolve(example, '../../../..');
const schema = join(repo, 'examples/data/geonames/schema.toml');
const pageViewer = 'http://localhost:9141';

const binary = findBinary()?.path ?? null;
let dir = '';
let viewer = '';
let server: ChildProcess | null = null;
let vite: ViteDevServer | null = null;
let browser: Browser | null = null;

beforeAll(async () => {
  if (!binary) return console.log('no mosaica binary: set MOSAICA_BIN, put mosaica on PATH, or run cargo build --release -p mosaica');
  dir = await mkdtemp(join(tmpdir(), 'mosaica-getting-started-'));
  const toml = (await readFile(join(example, 'mosaica/mosaica.toml'), 'utf8')).replace(/127\.0\.0\.1:914\d/g, '127.0.0.1:0');
  await writeFile(join(dir, 'mosaica.toml'), toml);
  await new Promise<void>((done, fail) =>
    spawn(binary, ['build', '--config', schema], {cwd: dir, stdio: 'ignore'})
      .on('error', fail)
      .on('exit', (code) => (code === 0 ? done() : fail(new Error(`mosaica build exited ${code}`))))
  );

  server = spawn(binary, ['serve', '--exit-on-stdin-close'], {cwd: dir, stdio: ['pipe', 'pipe', 'ignore']});
  const listening = await new Promise<{viewer: string}>((ready, fail) => {
    server!.on('error', fail).on('exit', () => fail(new Error('mosaica serve stopped while starting')));
    createInterface({input: server!.stdout!}).once('line', (line) => ready(JSON.parse(line)));
  });
  viewer = `http://${listening.viewer}`;

  vite = await createServer({root: web, logLevel: 'silent', server: {port: 0}});
  await vite.listen();
  browser = await chromium.launch();
});

afterAll(async () => {
  await browser?.close();
  await vite?.close();
  if (server && server.exitCode === null) {
    const exited = new Promise((done) => server!.once('exit', done));
    server.stdin?.end();
    await exited;
  }
  if (dir) await rm(dir, {recursive: true, force: true});
});

it.skipIf(!binary)('draws every place with no token, in an explorer that fills its part of the page', async () => {
  const page = await browser!.newPage({viewport: {width: 1440, height: 900}});
  await page.route(`${pageViewer}/**`, async (route) => {
    const url = viewer + route.request().url().slice(pageViewer.length);
    await route.fulfill({response: await route.fetch({url})});
  });
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(vite!.resolvedUrls!.local[0]!);
  // The map's probe is internal; it is read here to see that marks reached the canvas.
  const drawn = await page.waitForFunction(
    () => {
      const map = (document.querySelector('mosaica-explorer') as unknown as {map: {probe: {marks: number; view: {status: string; visible: number}}} | null}).map;
      const probe = map?.probe;
      return probe && probe.view.status === 'shown' && probe.marks > 0 ? {marks: probe.marks, visible: probe.view.visible} : null;
    },
    undefined,
    {timeout: 120_000, polling: 250}
  );
  const {marks, visible} = (await drawn.jsonValue())!;
  expect(visible).toBe(150_000);
  expect(marks).toBeGreaterThan(100_000);

  const box = await page.locator('mosaica-explorer').boundingBox();
  const header = await page.locator('body > header').boundingBox();
  const aside = await page.locator('main > aside').boundingBox();
  expect(box!.y).toBeCloseTo(header!.y + header!.height, 0);
  expect(box!.y + box!.height).toBeCloseTo(900, 0);
  expect(box!.x + box!.width).toBeCloseTo(aside!.x, 0);
  expect(errors).toEqual([]);
});

it.skipIf(!binary)('names the countries when the Countries and regions layer is ticked', async () => {
  const page = await browser!.newPage({viewport: {width: 1440, height: 900}});
  await page.route(`${pageViewer}/**`, async (route) => {
    const url = viewer + route.request().url().slice(pageViewer.length);
    await route.fulfill({response: await route.fetch({url})});
  });
  await page.goto(vite!.resolvedUrls!.local[0]!);
  const explorer = page.locator('mosaica-explorer');
  await explorer.locator('[part~="layers-toggle"]').click();
  await explorer.locator('mosaica-layer-picker [data-layer="admin/hierarchy"] input').check();
  // The map's probe is internal; it is read here to see that names were placed.
  const placed = await page.waitForFunction(
    () => {
      const map = (document.querySelector('mosaica-explorer') as unknown as {map: {probe: {timings: {labels: number}}} | null}).map;
      return (map?.probe.timings.labels ?? 0) > 0 ? map!.probe.timings.labels : null;
    },
    undefined,
    {timeout: 60_000, polling: 250}
  );
  expect(await placed.jsonValue()).toBeGreaterThan(20);
});
