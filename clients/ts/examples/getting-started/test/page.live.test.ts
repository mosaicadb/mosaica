import {mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {chromium, type Browser} from 'playwright';
import {createServer, type ViteDevServer} from 'vite';
import {afterAll, beforeAll, expect, it} from 'vitest';
import {findMosaica, mosaica} from '../dev/mosaica.js';

/**
 * The page as `npm start` serves it, in headless Chromium, over a Mosaica server built into a
 * temporary directory. Skipped, with the reason, where there is no `mosaica` binary.
 */
const root = join(import.meta.dirname, '..');
const binary = findMosaica();
let demo = '';
let vite: ViteDevServer | null = null;
let browser: Browser | null = null;

beforeAll(async () => {
  if (!binary) return console.log('no mosaica binary: set MOSAICA_BIN, put mosaica on PATH, or run cargo build --release -p mosaica');
  demo = await mkdtemp(join(tmpdir(), 'mosaica-getting-started-'));
  process.env.MOSAICA_DEMO_DIR = demo;
  vite = await createServer({root, configFile: join(root, 'vite.config.ts'), logLevel: 'silent', server: {port: 0, strictPort: false}});
  await vite.listen();
  browser = await chromium.launch();
});

afterAll(async () => {
  await browser?.close();
  await vite?.close();
  if (binary) await (await mosaica()).stop();
  if (demo) await rm(demo, {recursive: true, force: true});
});

it.skipIf(!binary)('mounts the explorer, which draws every place', async () => {
  const page = await browser!.newPage({viewport: {width: 1440, height: 900}});
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
  await expect(page.getByText('Places of the world').first().isVisible()).resolves.toBe(true);
  expect(errors).toEqual([]);
});
