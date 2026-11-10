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

type Seen = {zooms: number[]; hovered: string[]; points: [number, number][]; selectRegion(bbox: [number, number, number, number]): void; regionEdge(): [number, number]; setStatus(over: object): void; ready?: boolean};
type Box = {left: number; top: number; right: number; bottom: number; width: number; height: number};

const frames = (page: Page) => page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));

/** The page with its bare map and strip, or with `?explorer` its explorer; `reducedMotion` as the viewer's system asks. */
async function open(search = '', reducedMotion: 'reduce' | 'no-preference' = 'no-preference'): Promise<Page> {
  const page = await browser.newPage({viewport: {width: 900, height: 700}, reducedMotion});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(origin + search);
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

  it('goes on Escape with the focus outside the map, leaving the press to the page, and comes back on another mark', async () => {
    const page = await open();
    // What a handler of the page's own is given for each Escape.
    await page.evaluate(() => {
      const w = window as unknown as {prevented: boolean[]};
      w.prevented = [];
      document.addEventListener('keydown', (e) => e.key === 'Escape' && w.prevented.push(e.defaultPrevented));
    });
    const prevented = () => page.evaluate(() => (window as unknown as {prevented: boolean[]}).prevented);
    await hover(page, 0);
    await tooltipShown(page);
    expect(await page.evaluate(() => document.activeElement === document.body)).toBe(true);
    await page.keyboard.press('Escape');
    await frames(page);
    expect(await boxOf(page, 'mosaica-map', '[part="tooltip"]')).toBeNull();
    expect(await prevented()).toEqual([false]);
    // A move on the same mark does not bring it back.
    await hover(page, 0, 1);
    await frames(page);
    await frames(page);
    expect(await boxOf(page, 'mosaica-map', '[part="tooltip"]')).toBeNull();
    await hover(page, 1);
    await tooltipShown(page);

    // With the focus in the map, the press that dismisses the tooltip is the map's alone.
    await page.evaluate(() => (document.querySelector('mosaica-map') as HTMLElement).focus());
    await page.keyboard.press('Escape');
    await frames(page);
    expect(await boxOf(page, 'mosaica-map', '[part="tooltip"]')).toBeNull();
    expect(await prevented()).toEqual([false, true]);
  }, 120_000);
});

describe('the map’s ground in a browser', () => {
  /**
   * How far the selected region's outline stands from the ground beside it, as a contrast ratio.
   * The outline is drawn in the ink of the ground the map last drew for: dark on a light ground
   * and light on a dark one, so it stands well clear of the ground it is on only when the map has
   * drawn for that ground.
   */
  async function outlineContrast(page: Page): Promise<number> {
    const [x, y] = await page.evaluate(() => (window as unknown as Seen).regionEdge());
    const png = (await page.screenshot({clip: {x: Math.round(x) - 10, y: Math.round(y), width: 14, height: 1}})).toString('base64');
    return page.evaluate(async (data) => {
      const image = new Image();
      image.src = `data:image/png;base64,${data}`;
      await image.decode();
      const ctx = new OffscreenCanvas(image.width, 1).getContext('2d')!;
      ctx.drawImage(image, 0, 0);
      const px = ctx.getImageData(0, 0, image.width, 1).data;
      const luminance = (at: number) => {
        const [r, g, b] = [0, 1, 2].map((c) => {
          const v = px[at * 4 + c]! / 255;
          return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
      };
      // The row's first pixel is bare ground, and the outline crosses it further along.
      const ground = luminance(0);
      let most = 1;
      for (let at = 1; at < image.width; at++) most = Math.max(most, (Math.max(luminance(at), ground) + 0.05) / (Math.min(luminance(at), ground) + 0.05));
      return most;
    }, png);
  }

  /** Wait for the map to draw the outline in the ink of the ground it is now on. */
  async function redrawn(page: Page): Promise<number> {
    let contrast = 0;
    for (let tries = 0; tries < 60; tries++) {
      await frames(page);
      contrast = await outlineContrast(page);
      if (contrast >= 4) break;
    }
    return contrast;
  }
  const select = (page: Page) => page.evaluate(() => (window as unknown as Seen).selectRegion([0.3, 0.3, 0.45, 0.7]));

  it('draws for the other ground when the page’s colour scheme changes, and when the system’s does', async () => {
    const page = await open();
    await select(page);
    const dark = () => page.evaluate(() => (document.querySelector('mosaica-map') as unknown as {drawnGround: string}).drawnGround === 'dark');
    expect(await dark()).toBe(false);
    expect(await redrawn(page)).toBeGreaterThanOrEqual(4);

    await page.evaluate(() => (document.documentElement.style.colorScheme = 'dark'));
    expect(await dark()).toBe(true);
    expect(await redrawn(page)).toBeGreaterThanOrEqual(4);
    await page.evaluate(() => (document.documentElement.style.colorScheme = 'light'));
    expect(await redrawn(page)).toBeGreaterThanOrEqual(4);

    // A page that allows both follows the system.
    await page.evaluate(() => (document.documentElement.style.colorScheme = 'light dark'));
    await page.emulateMedia({colorScheme: 'dark'});
    expect(await dark()).toBe(true);
    expect(await redrawn(page)).toBeGreaterThanOrEqual(4);
    await page.emulateMedia({colorScheme: 'light'});
    expect(await dark()).toBe(false);
    expect(await redrawn(page)).toBeGreaterThanOrEqual(4);
  }, 120_000);

  it('does so inside an explorer for a viewer who asks for reduced motion, and under a page’s own rule for the map’s transitions', async () => {
    const page = await open('?explorer', 'reduce');
    await select(page);
    expect(await page.evaluate(() => matchMedia('(prefers-reduced-motion: reduce)').matches)).toBe(true);
    expect(await redrawn(page)).toBeGreaterThanOrEqual(4);
    await page.evaluate(() => (document.documentElement.style.colorScheme = 'dark'));
    expect(await redrawn(page)).toBeGreaterThanOrEqual(4);
    await page.evaluate(() => (document.documentElement.style.colorScheme = 'light'));
    expect(await redrawn(page)).toBeGreaterThanOrEqual(4);

    const bare = await open();
    await select(bare);
    await bare.addStyleTag({content: 'mosaica-map { transition: none !important; }'});
    await bare.evaluate(() => (document.documentElement.style.colorScheme = 'dark'));
    expect(await redrawn(bare)).toBeGreaterThanOrEqual(4);
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
      // The button is what a click lands on anywhere in the 24 px square about its middle, which
      // lies inside the tag.
      const hit = await page.evaluate(
        ([cx, cy]) => {
          const root = document.querySelector('mosaica-map')!.shadowRoot!;
          const clear = root.querySelector('[part="region-tag"] button');
          return [[0, 0], [-11, -11], [11, -11], [-11, 11], [11, 11]].every(([dx, dy]) => root.elementFromPoint(cx! + dx!, cy! + dy!)?.closest('button') === clear);
        },
        [clear.left + clear.width / 2, clear.top + clear.height / 2]
      );
      expect(hit, `${x},${y}`).toBe(true);
      expect(within({...clear, left: clear.left - 2, top: clear.top - 2, right: clear.right + 2, bottom: clear.bottom + 2}, tag), `${x},${y}`).toBe(true);
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

describe('a status strip’s counts that wait for an answer, in a browser', () => {
  /** Each count and word of the strip's count cells: its contrast against the strip, its colour, and whether anything fades it. */
  const figures = (page: Page) =>
    page.evaluate(() => {
      const root = document.querySelector('mosaica-status')!.shadowRoot!;
      const channels = (colour: string) => colour.match(/[\d.]+/g)!.slice(0, 3).map(Number);
      const luminance = (colour: string) => {
        const [r, g, b] = channels(colour).map((c) => (c / 255 <= 0.04045 ? c / 255 / 12.92 : ((c / 255 + 0.055) / 1.055) ** 2.4));
        return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!;
      };
      const ground = luminance(getComputedStyle(root.querySelector('[part="strip"]')!).backgroundColor);
      const texts = [...root.querySelectorAll('.cell:not(.state)')].flatMap((cell) => [...cell.querySelectorAll('span, mosaica-count')].flatMap((e) => (e.shadowRoot ? [...e.shadowRoot.querySelectorAll('span')] : [e])));
      return texts
        .filter((e) => e.textContent!.trim() !== '')
        .map((e) => {
          const ink = luminance(getComputedStyle(e).color);
          let faded = false;
          for (let at: Element | null = e; at; at = at.parentElement ?? ((at.getRootNode() as ShadowRoot).host as Element | undefined) ?? null) if (getComputedStyle(at).opacity !== '1') faded = true;
          return {text: e.textContent!.trim(), colour: getComputedStyle(e).color, contrast: (Math.max(ink, ground) + 0.05) / (Math.min(ink, ground) + 0.05), faded};
        });
    });

  it('keeps them at 4.5 to 1 in both colour schemes, drawn fainter than the current ones and faded by nothing', async () => {
    const page = await open();
    for (const scheme of ['light', 'dark'] as const) {
      await page.emulateMedia({colorScheme: scheme});
      await page.evaluate(() => (window as unknown as Seen).setStatus({}));
      await frames(page);
      const current = await figures(page);
      expect(current.length, scheme).toBeGreaterThanOrEqual(4);
      for (const over of [{status: 'loading'}, {status: 'refused', refusal: {code: 'contract', detail: 'no', status: 422}}, {status: 'retrying'}, {unreachable: true}]) {
        await page.evaluate((o) => (window as unknown as Seen).setStatus(o), over);
        await frames(page);
        const waiting = await figures(page);
        expect(waiting.map((f) => f.text), scheme).toEqual(current.map((f) => f.text));
        for (const figure of waiting) {
          expect(figure.contrast, `${scheme} ${JSON.stringify(over)}: ${figure.text}`).toBeGreaterThanOrEqual(4.5);
          expect(figure.faded, `${scheme}: ${figure.text}`).toBe(false);
        }
        // The matched count, the darkest of the current figures, is drawn in another ink.
        expect(waiting[0]!.colour, scheme).not.toBe(current[0]!.colour);
        expect(waiting[0]!.contrast, scheme).toBeLessThan(current[0]!.contrast);
      }
    }
  }, 120_000);
});
