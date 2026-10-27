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
  // A busy machine slows every step of the page alike, so each locator gets the draw's budget
  // rather than Playwright's 30 s.
  page.setDefaultTimeout(120_000);
  await page.route(`${pageViewer}/**`, async (route) => {
    const url = viewer + route.request().url().slice(pageViewer.length);
    // A request still out when the test closes the page has nowhere to go.
    await route.fetch({url}).then((response) => route.fulfill({response})).catch(() => route.abort().catch(() => {}));
  });
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(vite!.resolvedUrls!.local[0]!);
  return page;
}

it.skipIf(!binary)('draws every paper, lists the archives, and narrows the count and the list to the archive chosen', async () => {
  const errors: string[] = [];
  const page = await open(errors);
  // The map's probe is internal; it is read here to see that marks reached the canvas. The status
  // turns to shown as the last piece of the answer lands, before the frame that draws that piece,
  // so the drawn count is polled until it holds every paper.
  const drawn = () =>
    page.evaluate(() => {
      const probe = (document.querySelector('mosaica-map') as unknown as {probe: {marks: number; view: {status: string; visible: number}}}).probe;
      return probe.view.status === 'shown' && probe.marks > 0 ? probe.view.visible : null;
    });
  await expect.poll(drawn, {timeout: 120_000, interval: 250}).toBe(50_000);

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

it.skipIf(!binary)('filters to a topic whose name is clicked on the map, and searches within it', async () => {
  const errors: string[] = [];
  const page = await open(errors);
  const count = page.locator('#count');
  await expect.poll(() => count.textContent(), {timeout: 120_000}).toBe('50,000 of the 50,000 papers in view match');
  await page.check('#topics');

  // Where "Nanodevices" is drawn, read from the deck's name layer, and pointed at until the map
  // reports the pointer over that artifact. The deck and the hovered artifact are internal.
  const map = page.locator('mosaica-map');
  let nudge = 0;
  await expect
    .poll(
      async () => {
        const at = await map.evaluate((el, topic) => {
          const deck = (el as unknown as {deck: {getViewports(): {project(p: number[]): number[]}[]; layerManager: {getLayers(): {id: string; props: {data: unknown}}[]}}}).deck;
          const viewport = deck?.getViewports()[0];
          for (const layer of deck?.layerManager.getLayers() ?? []) {
            if (!layer.id.endsWith('-labels') || !Array.isArray(layer.props.data)) continue;
            const row = (layer.props.data as {id: bigint; position: number[]; text: string; offset: number[]}[]).find((d) => d.text === topic);
            if (!row || !viewport) continue;
            const [x, y] = viewport.project(row.position);
            const box = el.getBoundingClientRect();
            return {id: String(row.id), x: box.left + x! + row.offset[0]!, y: box.top + y! + row.offset[1]!};
          }
          return null;
        }, 'Nanodevices');
        if (!at) return false;
        // A hover is reported on a move, so each try moves the pointer.
        nudge = 1 - nudge;
        await page.mouse.move(at.x + nudge, at.y);
        return map.evaluate((el, id) => String((el as unknown as {hoveredArtifact: bigint | null}).hoveredArtifact) === id, at.id);
      },
      {timeout: 60_000, intervals: [250]}
    )
    .toBe(true);
  // A press and release where the pointer rests, as a reader clicks, and again while the name has
  // not opened, as a reader clicks again. The map takes a press as a click when it handles the
  // release within a quarter of a second of the press, and a starved browser can run a frame of
  // seconds between the two.
  const chosen = page.locator('#topic strong');
  await expect
    .poll(
      async () => {
        const shown = await chosen.textContent();
        if (shown === '') {
          await page.mouse.down();
          await page.mouse.up();
        }
        return shown;
      },
      {timeout: 60_000, interval: 5_000}
    )
    .toBe('Nanodevices');

  await expect.poll(() => count.textContent(), {timeout: 60_000}).toBe('1,242 of the 50,000 papers in view match');
  await expect.poll(() => page.locator('#results li').count(), {timeout: 60_000}).toBe(20);
  await page.fill('#title', 'graphene');
  await expect.poll(() => count.textContent(), {timeout: 60_000}).toBe('71 of the 50,000 papers in view match');
  await expect.poll(() => page.locator('#results li').first().textContent(), {timeout: 60_000}).toMatch(/graphene/i);

  await page.fill('#title', '');
  await page.click('#topic button');
  await expect.poll(() => count.textContent(), {timeout: 60_000}).toBe('50,000 of the 50,000 papers in view match');
  expect(errors).toEqual([]);
  await page.close();
});
