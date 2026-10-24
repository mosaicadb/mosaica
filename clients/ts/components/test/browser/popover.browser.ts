import {mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium, type Browser, type Page} from 'playwright';
import {build} from 'vite';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';

/** The Layers popover against the explorer's visible box, in Chromium, from the single-file bundle. */

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
let out = '';
let browser: Browser;

beforeAll(async () => {
  out = mkdtempSync(join(tmpdir(), 'mosaica-components-'));
  await build({configFile: join(root, 'vite.config.ts'), root, logLevel: 'silent', build: {outDir: out, emptyOutDir: true}});
  browser = await chromium.launch({args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader']});
});

afterAll(async () => {
  await browser?.close();
  if (out) rmSync(out, {recursive: true, force: true});
});

/** An explorer `height` pixels tall in a window `width` by `windowHeight`, its Layers popover open. */
async function opened(width: number, windowHeight: number, height: string): Promise<Page> {
  const p = await browser.newPage({viewport: {width, height: windowHeight}});
  await p.setContent(`<!doctype html><html><head><style>body { margin: 0; height: 2000px; } mosaica-explorer { --mosaica-explorer-height: ${height}; }</style></head><body><mosaica-explorer></mosaica-explorer></body></html>`);
  await p.addScriptTag({path: join(out, 'mosaica-components.js'), type: 'module'});
  await p.evaluate(async () => {
    await customElements.whenDefined('mosaica-explorer');
    const ex = document.querySelector('mosaica-explorer') as HTMLElement & {updateComplete: Promise<unknown>};
    await ex.updateComplete;
    (ex.shadowRoot!.querySelector('[part="layers-toggle"]') as HTMLButtonElement).click();
    await ex.updateComplete;
    await new Promise((r) => requestAnimationFrame(r));
  });
  return p;
}

const boxes = (p: Page) =>
  p.evaluate(() => {
    const ex = document.querySelector('mosaica-explorer')!;
    const pop = ex.shadowRoot!.querySelector('[part="layers-popover"]') as HTMLElement;
    const e = ex.getBoundingClientRect();
    const r = pop.getBoundingClientRect();
    return {top: r.top, bottom: r.bottom, boxTop: Math.max(0, e.top), boxBottom: Math.min(innerHeight, e.bottom), scrolls: pop.scrollHeight > pop.clientHeight};
  });

describe('the Layers popover', () => {
  for (const [name, width, windowHeight, height] of [
    ['compact, opening upwards', 900, 560, '100vh'],
    ['docked, in a box shorter than the window', 1280, 900, '520px'],
    ['docked, in a box taller than the window', 1280, 600, '1200px']
  ] as const) {
    it(`fits the explorer's visible box: ${name}`, async () => {
      const b = await boxes(await opened(width, windowHeight, height));
      expect(b.top).toBeGreaterThanOrEqual(b.boxTop);
      expect(b.bottom).toBeLessThanOrEqual(b.boxBottom);
    });
  }
});
