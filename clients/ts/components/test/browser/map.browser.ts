import type {Browser, Page} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * A bare map and a status strip in Chromium, for what needs deck's own input handling or a real
 * layout: one zoom level for each key press after a click, the tooltip and the region's tag kept
 * inside the map, the canvas's focus ring, and a strip short of room wrapping its cells.
 */

let served: Awaited<ReturnType<typeof servePage>>;
let origin = '';
let browser: Browser;

beforeAll(async () => {
  served = await servePage('map-page.ts');
  ({origin, browser} = served);
});

afterAll(async () => {
  await served?.close();
});

type Seen = {zooms: number[]; hovered: string[]; points: [number, number][]; selectRegion(bbox: [number, number, number, number]): void; ready?: boolean};
type Box = {left: number; top: number; right: number; bottom: number; width: number; height: number};

const frames = (page: Page) => page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));

async function open(): Promise<Page> {
  const page = await browser.newPage({viewport: {width: 900, height: 700}});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(origin);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as Seen).ready === true, undefined, {timeout: 60_000})]);
  await frames(page);
  return page;
}

/** The box of the element `selector` finds in the shadow root of the page's first `tag`, or null. */
const boxOf = (page: Page, tag: string, selector: string) =>
  page.evaluate(
    ([t, s]) => {
      const el = document.querySelector(t!)?.shadowRoot?.querySelector(s!);
      if (!el) return null;
      const r = el.getBoundingClientRect();
      return {left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width, height: r.height};
    },
    [tag, selector]
  ) as Promise<Box | null>;

const within = (inner: Box, outer: Box) => inner.left >= outer.left && inner.top >= outer.top && inner.right <= outer.right && inner.bottom <= outer.bottom;

/** Move the pointer onto the page's mark `n` and wait for its tooltip. */
async function hover(page: Page, n: number, nudge = 0): Promise<void> {
  const [x, y] = (await page.evaluate(() => (window as unknown as Seen).points))[n]!;
  await page.mouse.move(x + nudge - 6, y - 6);
  await page.mouse.move(x + nudge, y, {steps: 3});
}
const tooltipShown = (page: Page) => page.waitForFunction(() => document.querySelector('mosaica-map')!.shadowRoot!.querySelector('[part="tooltip"]') !== null, undefined, {timeout: 20_000});

describe('the map’s keys in a browser', () => {
  it('zoom one level for each press after a click on the map, as after reaching it with Tab', async () => {
    const page = await open();
    const zooms = () => page.evaluate(() => (window as unknown as Seen).zooms);
    // A click on empty ground puts the focus on deck's own canvas.
    await page.mouse.click(200, 120);
    await frames(page);
    const before = (await zooms()).length;
    await page.keyboard.press('+');
    await page.keyboard.press('+');
    await page.keyboard.press('-');
    await frames(page);
    expect((await zooms()).slice(before)).toEqual([1, 2, 1]);

    // In box mode too, where the controller is set again.
    await page.evaluate(() => ((document.querySelector('mosaica-map') as unknown as {mode: string}).mode = 'box'));
    await frames(page);
    await page.evaluate(() => (document.querySelector('mosaica-map') as HTMLElement).focus());
    const boxed = (await zooms()).length;
    await page.keyboard.press('+');
    await frames(page);
    expect((await zooms()).slice(boxed)).toEqual([2]);
  }, 120_000);

  it('show a focus ring two pixels wide inside the canvas’s edge', async () => {
    const page = await open();
    for (let i = 0; i < 5; i++) await page.keyboard.press('Tab');
    const ring = await page.evaluate(() => {
      const root = document.querySelector('mosaica-map')!.shadowRoot!;
      const canvas = root.querySelector('[part="canvas"]')!;
      const style = getComputedStyle(canvas);
      return {focused: root.activeElement === canvas, visible: canvas.matches(':focus-visible'), width: style.outlineWidth, style: style.outlineStyle, offset: style.outlineOffset};
    });
    expect(ring).toEqual({focused: true, visible: true, width: '2px', style: 'solid', offset: '-2px'});
  }, 120_000);
});

describe('the map’s tooltip in a browser', () => {
  it('stays inside the map beside a mark at each of its corners', async () => {
    const page = await open();
    const map = (await boxOf(page, 'mosaica-map', '[part="canvas"]'))!;
    for (const n of [0, 1, 2, 3, 4]) {
      await hover(page, n);
      await tooltipShown(page);
      await frames(page);
      const tip = (await boxOf(page, 'mosaica-map', '[part="tooltip"]'))!;
      expect(tip.width, `mark ${n}`).toBeGreaterThan(20);
      expect(within(tip, map), `mark ${n}: ${JSON.stringify(tip)}`).toBe(true);
      // Away, so the next mark's tooltip is its own.
      await page.mouse.move(400, 120);
      await page.waitForFunction(() => document.querySelector('mosaica-map')!.shadowRoot!.querySelector('[part="tooltip"]') === null, undefined, {timeout: 20_000});
    }
  }, 120_000);

  it('goes on Escape with the focus outside the map, and comes back on another mark', async () => {
    const page = await open();
    await hover(page, 0);
    await tooltipShown(page);
    expect(await page.evaluate(() => document.activeElement === document.body)).toBe(true);
    await page.keyboard.press('Escape');
    await frames(page);
    expect(await boxOf(page, 'mosaica-map', '[part="tooltip"]')).toBeNull();
    // A move on the same mark does not bring it back.
    await hover(page, 0, 1);
    await frames(page);
    await frames(page);
    expect(await boxOf(page, 'mosaica-map', '[part="tooltip"]')).toBeNull();
    await hover(page, 1);
    await tooltipShown(page);
  }, 120_000);
});

describe('the region’s tag in a browser', () => {
  it('stays inside the map with its Clear button whole, wherever the region’s corner is', async () => {
    const page = await open();
    await page.evaluate(() => (window as unknown as Seen).selectRegion([0.45, 0.45, 0.55, 0.55]));
    await frames(page);
    const map = (await boxOf(page, 'mosaica-map', '[part="canvas"]'))!;
    // The camera looks at points that put the region's corner off each edge in turn.
    for (const [x, y] of [[0.5, 0.5], [-2, 0.5], [3, 0.5], [0.5, -2], [0.5, 3], [-2, -2], [3, 3]]) {
      await page.evaluate(([px, py]) => (document.querySelector('mosaica-map') as unknown as {lookAt(x: number, y: number): boolean}).lookAt(px!, py!), [x, y]);
      await frames(page);
      const tag = (await boxOf(page, 'mosaica-map', '[part="region-tag"]'))!;
      const clear = (await boxOf(page, 'mosaica-map', '[part="region-tag"] button'))!;
      expect(within(tag, map), `${x},${y}: ${JSON.stringify(tag)}`).toBe(true);
      expect(clear.width, `${x},${y}`).toBeGreaterThanOrEqual(24);
      expect(clear.height, `${x},${y}`).toBeGreaterThanOrEqual(24);
      // The button is what a click at its middle lands on.
      const hit = await page.evaluate(
        ([cx, cy]) => {
          const root = document.querySelector('mosaica-map')!.shadowRoot!;
          return root.elementFromPoint(cx!, cy!)?.closest('button') === root.querySelector('[part="region-tag"] button');
        },
        [clear.left + clear.width / 2, clear.top + clear.height / 2]
      );
      expect(hit, `${x},${y}`).toBe(true);
    }
  }, 120_000);
});

describe('a status strip short of room in a browser', () => {
  it('wraps its cells onto further lines, with nothing cut off', async () => {
    const page = await open();
    const strip = await page.evaluate(() => {
      const root = document.querySelector('mosaica-status')!.shadowRoot!;
      const box = (el: Element) => {
        const r = el.getBoundingClientRect();
        return {left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width, height: r.height};
      };
      const el = root.querySelector('[part="strip"]')!;
      const cells = [...root.querySelectorAll('.cell')];
      // What each cell holds, down to the figures inside the counts' own shadow roots.
      const held = cells.flatMap((cell) => [...cell.querySelectorAll('span, mosaica-count')].filter((e) => !e.closest('.sr-only')).map((e) => box(e.shadowRoot?.querySelector('[part="count"]') ?? e)));
      return {strip: box(el), cells: cells.map(box), held, scrolls: el.scrollWidth > el.clientWidth || el.scrollHeight > el.clientHeight};
    });
    expect(strip.strip.width).toBeLessThanOrEqual(150);
    expect(strip.scrolls).toBe(false);
    expect(new Set(strip.cells.map((c) => Math.round(c.top))).size).toBeGreaterThan(1);
    for (const cell of strip.cells) expect(within(cell, {...strip.strip, left: strip.strip.left - 1, top: strip.strip.top - 1}), JSON.stringify(cell)).toBe(true);
    for (const box of strip.held) expect(within(box, strip.strip), JSON.stringify(box)).toBe(true);
  }, 120_000);
});
