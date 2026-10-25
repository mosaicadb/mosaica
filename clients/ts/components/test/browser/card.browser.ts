import {existsSync, mkdtempSync, readFileSync, rmSync} from 'node:fs';
import {createServer, type Server} from 'node:http';
import {tmpdir} from 'node:os';
import {dirname, join, normalize} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium, type Browser} from 'playwright';
import {build, defaultClientConditions} from 'vite';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {mosaicaDecorators} from '../../vite-plugin-decorators.js';

/**
 * The item card's Show all, in Chromium's layout: on its own, and beside a point in the explorer,
 * where it keeps clear of the cards and tools over the map.
 */

const here = dirname(fileURLToPath(import.meta.url));
let built = '';
let server: Server;
let origin = '';
let browser: Browser;

beforeAll(async () => {
  built = mkdtempSync(join(tmpdir(), 'mosaica-card-'));
  await build({
    configFile: false,
    plugins: [mosaicaDecorators()],
    root: here,
    logLevel: 'silent',
    resolve: {
      conditions: ['mosaica-source', ...defaultClientConditions],
      alias: [{find: /^\.\/lazy\.js$/, replacement: join(here, '..', '..', '..', 'deck', 'src', 'lazy-static.ts')}]
    },
    define: {'process.env.NODE_ENV': JSON.stringify('production')},
    build: {outDir: built, emptyOutDir: true, lib: {entry: join(here, 'card-page.ts'), formats: ['es'], fileName: () => 'page.js'}, rolldownOptions: {output: {codeSplitting: false}}}
  });
  server = createServer((request, response) => {
    const path = normalize(decodeURIComponent(new URL(request.url ?? '/', 'http://x').pathname));
    if (path === '/') return void response.writeHead(200, {'content-type': 'text/html'}).end('<!doctype html><body style="margin:0"><script type="module" src="/page.js"></script></body>');
    const file = join(built, path);
    if (!existsSync(file)) return void response.writeHead(404).end();
    response.writeHead(200, {'content-type': 'text/javascript'}).end(readFileSync(file));
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  origin = `http://127.0.0.1:${typeof address === 'object' && address ? address.port : 0}`;
  browser = await chromium.launch({args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader']});
});

afterAll(async () => {
  await browser?.close();
  await new Promise((resolve) => server?.close(resolve));
  if (built) rmSync(built, {recursive: true, force: true});
});

describe('Show all on a compact item card', () => {
  it('grows the card to hold every field, and the card beside a point up to the height of the map, clear of the cards and tools over the map', async () => {
    const page = await browser.newPage({viewport: {width: 1200, height: 800}});
    const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
    await page.goto(origin);
    await Promise.race([failed, page.waitForFunction(() => (window as unknown as {result?: unknown}).result !== undefined, undefined, {timeout: 80_000})]);
    const r = await page.evaluate(() => (window as unknown as {result: {alone: number; before: number; after: number; inside: boolean; mapHeight: number; covered: string[]; coveredToolsAlone: string[]}}).result);
    // Forty fields at a row each are far taller than 300 px.
    expect(r.alone).toBeGreaterThan(600);
    // Beside its point, the card grows past 300 px and stays on the map, which it scrolls inside.
    expect(r.after).toBeGreaterThan(Math.max(r.before, 300));
    expect(r.after).toBeLessThanOrEqual(r.mapHeight);
    expect(r.inside).toBe(true);
    // Wherever a point is picked, its card keeps clear of the panel, the status strip, the map's
    // tools and the Layers button.
    expect(r.covered).toEqual([]);
    expect(r.coveredToolsAlone).toEqual([]);
  }, 90_000);
});
