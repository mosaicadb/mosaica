import {spawn, type ChildProcess} from 'node:child_process';
import {mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createInterface} from 'node:readline';
import {chromium, type Browser, type Page} from 'playwright';
import {createServer, type ViteDevServer} from 'vite';
import {afterAll, beforeAll, expect, it} from 'vitest';
import {findBinary} from '../../../../core/test/served.js';

/**
 * The tutorial's finished page in headless Chromium, served by Vite, over an open Mosaica server
 * built from the example's `mosaica.toml` into a temporary directory. The server listens on ports
 * the system chooses, and the browser's requests to the page's viewer URL, port 9241, are sent
 * there instead. The binary is found as core's live tests find it. Skipped, with the reason, where
 * there is none.
 */
const web = resolve(import.meta.dirname, '..');
const example = resolve(web, '..');
const repo = resolve(example, '../../../..');
const schema = join(repo, 'examples/data/arxiv/schema.toml');
const pageViewer = 'http://localhost:9241';

const binary = findBinary()?.path ?? null;
let dir = '';
let viewer = '';
let server: ChildProcess | null = null;
let vite: ViteDevServer | null = null;
let browser: Browser | null = null;

beforeAll(async () => {
  if (!binary) return console.log('no mosaica binary: set MOSAICA_BIN, put mosaica on PATH, or run cargo build --release -p mosaica');
  dir = await mkdtemp(join(tmpdir(), 'mosaica-own-ui-'));
  const toml = (await readFile(join(example, 'mosaica/mosaica.toml'), 'utf8')).replace(/127\.0\.0\.1:924\d/g, '127.0.0.1:0');
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

async function open(errors: string[]): Promise<Page> {
  const page = await browser!.newPage({viewport: {width: 1440, height: 900}});
  await page.route(`${pageViewer}/**`, async (route) => {
    const url = viewer + route.request().url().slice(pageViewer.length);
    // A request still out when the test closes the page has nowhere to go.
    await route.fulfill({response: await route.fetch({url})}).catch(() => {});
  });
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(vite!.resolvedUrls!.local[0]!);
  return page;
}

it.skipIf(!binary)('draws every paper, lists the archives, and narrows the count and the list to the archive chosen', async () => {
  const errors: string[] = [];
  const page = await open(errors);
  // The map's probe is internal; it is read here to see that marks reached the canvas.
  const drawn = await page.waitForFunction(
    () => {
      const probe = (document.querySelector('mosaica-map') as unknown as {probe: {marks: number; view: {status: string; visible: number}}}).probe;
      return probe.view.status === 'shown' && probe.marks > 0 ? {marks: probe.marks, visible: probe.view.visible} : null;
    },
    undefined,
    {timeout: 120_000, polling: 250}
  );
  expect((await drawn.jsonValue())!.visible).toBe(50_000);

  const options = page.locator('#archive option');
  await expect.poll(() => options.count(), {timeout: 60_000}).toBeGreaterThan(30);
  expect(await options.nth(1).textContent()).toBe('Computer Science (10,478)');

  const count = page.locator('#count');
  await expect.poll(() => count.textContent(), {timeout: 60_000}).toBe('50,000 of the 50,000 papers in view match');
  await expect.poll(() => page.locator('#results li').count(), {timeout: 60_000}).toBe(20);

  await page.selectOption('#archive', 'gr-qc');
  await expect.poll(() => count.textContent(), {timeout: 60_000}).toBe('1,177 of the 50,000 papers in view match');
  await page.fill('#title', 'black hole');
  await expect.poll(() => count.textContent(), {timeout: 60_000}).toBe('133 of the 50,000 papers in view match');
  await expect.poll(() => page.locator('#results li').first().textContent(), {timeout: 60_000}).toMatch(/black hole/i);
  expect(errors).toEqual([]);
  await page.close();
});

it.skipIf(!binary)('filters to a topic opened on the map', async () => {
  const errors: string[] = [];
  const page = await open(errors);
  const count = page.locator('#count');
  await expect.poll(() => count.textContent(), {timeout: 120_000}).toBe('50,000 of the 50,000 papers in view match');
  await page.check('#topics');
  // Under headless Chromium's software renderer deck.gl's pick on a press takes about 300 ms, past
  // the time a press may take to count as a click, so a named topic is opened through the store,
  // which fires the same `mosaica-artifactopen` a click does.
  const named = await page.waitForFunction(
    () => {
      const store = document.querySelector('mosaica-store')!.store!;
      const first = [...store.get('artifacts').attached][0];
      return first ? {id: String(first[0]), name: first[1]} : null;
    },
    undefined,
    {timeout: 60_000, polling: 250}
  );
  const {id, name} = (await named.jsonValue())!;
  const members = await page.evaluate(async (id) => {
    const store = document.querySelector('mosaica-store')!.store!;
    await store.openArtifact(BigInt(id));
    return Number(store.get('selection').artifact!.detail.maskedCount);
  }, id);
  expect(members).toBeGreaterThan(0);
  expect(members).toBeLessThan(50_000);
  await expect.poll(() => page.locator('#topic strong').textContent(), {timeout: 60_000}).toBe(name);
  await expect.poll(() => count.textContent(), {timeout: 60_000}).toBe(`${members.toLocaleString('en-GB')} of the 50,000 papers in view match`);

  await page.click('#topic button');
  await expect.poll(() => count.textContent(), {timeout: 60_000}).toBe('50,000 of the 50,000 papers in view match');
  expect(errors).toEqual([]);
  await page.close();
});
