import {existsSync, mkdtempSync, readFileSync, rmSync} from 'node:fs';
import {createServer} from 'node:http';
import {tmpdir} from 'node:os';
import {dirname, join, normalize} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium, type Browser} from 'playwright';
import {build, defaultClientConditions} from 'vite';
import {mosaicaDecorators} from '../../vite-plugin-decorators.js';

const here = dirname(fileURLToPath(import.meta.url));

/**
 * A test page in this directory built by Vite from the packages' sources into one script, served
 * on a port the system chooses, and a headless Chromium on SwiftShader to open it. `close` stops
 * both and deletes the build.
 */
export async function servePage(entry: string): Promise<{origin: string; browser: Browser; close(): Promise<void>}> {
  const built = mkdtempSync(join(tmpdir(), 'mosaica-page-'));
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
    build: {outDir: built, emptyOutDir: true, lib: {entry: join(here, entry), formats: ['es'], fileName: () => 'page.js'}, rolldownOptions: {output: {codeSplitting: false}}}
  }).catch((e: unknown) => {
    rmSync(built, {recursive: true, force: true});
    throw e;
  });
  const server = createServer((request, response) => {
    const path = normalize(decodeURIComponent(new URL(request.url ?? '/', 'http://x').pathname));
    if (path === '/') return void response.writeHead(200, {'content-type': 'text/html'}).end('<!doctype html><body style="margin:0"><script type="module" src="/page.js"></script></body>');
    const file = join(built, path);
    if (!existsSync(file)) return void response.writeHead(404).end();
    response.writeHead(200, {'content-type': 'text/javascript'}).end(readFileSync(file));
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  const origin = `http://127.0.0.1:${typeof address === 'object' && address ? address.port : 0}`;
  const browser = await chromium.launch({args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader']});
  return {
    origin,
    browser,
    async close() {
      await browser.close();
      await new Promise((resolve) => server.close(resolve));
      rmSync(built, {recursive: true, force: true});
    }
  };
}
