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
 * A map that leaves the page and comes back draws its basemap again in its new deck.gl canvas, and
 * deck.gl raises no error over a layer it finalised.
 */

const here = dirname(fileURLToPath(import.meta.url));
let built = '';
let server: Server;
let origin = '';
let browser: Browser;

beforeAll(async () => {
  built = mkdtempSync(join(tmpdir(), 'mosaica-basemap-'));
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
    build: {outDir: built, emptyOutDir: true, lib: {entry: join(here, 'basemap-page.ts'), formats: ['es'], fileName: () => 'page.js'}, rolldownOptions: {output: {codeSplitting: false}}}
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

describe('a basemap across leaving the page and coming back', () => {
  it('is drawn again, with no error', async () => {
    const page = await browser.newPage({viewport: {width: 400, height: 300}});
    const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
    await page.goto(origin);
    await Promise.race([failed, page.waitForFunction(() => (window as unknown as {result?: unknown}).result !== undefined, undefined, {timeout: 30_000})]);
    const r = await page.evaluate(() => (window as unknown as {result: {world: {before: string[]; after: string[]}; host: {before: string[]; after: string[]}; errors: string[]}}).result);
    const outline = ['basemap-land', 'basemap-coastline', 'basemap-borders'];
    expect(r.world).toEqual({before: outline, after: outline});
    expect(r.host).toEqual({before: ['host-basemap'], after: ['host-basemap']});
    expect(r.errors).toEqual([]);
  });
});
