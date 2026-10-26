import type {Browser, Page} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * A cluster's name on a bare map, under the pointer in Chromium: a row of marks lies beneath it.
 * deck.gl counts a press as a click only if it is released within 250 ms of being handled.
 */

let served: Awaited<ReturnType<typeof servePage>>;
let origin = '';
let browser: Browser;

beforeAll(async () => {
  served = await servePage('label-click-page.ts');
  ({origin, browser} = served);
});

afterAll(async () => {
  await served?.close();
});

type Seen = {opened: string[]; picked: string[]; hovered: string[]; missed: number; centre?: [number, number]; markAt(points: [number, number][]): void};

/** Two animation frames: one for deck to pick at the pointer, and one for its answer. */
const frames = (page: Page) => page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));

const seen = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as Seen;
    return {opened: w.opened, picked: w.picked, hovered: w.hovered, missed: w.missed};
  });

/** The page once its name is drawn, and the name's centre in the page. */
async function open(search: string): Promise<{page: Page; centre: [number, number]}> {
  const page = await browser.newPage({viewport: {width: 800, height: 500}});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(origin + search);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as Seen).centre !== undefined, undefined, {timeout: 60_000})]);
  const [cx, cy] = await page.evaluate(() => (window as unknown as Seen).centre!);
  // The name's glyphs are rasterised on its first frames: wait for its ink on the middle row.
  const inked = async () => {
    const png = (await page.screenshot({clip: {x: cx - 20, y: cy, width: 40, height: 1}})).toString('base64');
    return page.evaluate(async (data) => {
      const image = new Image();
      image.src = `data:image/png;base64,${data}`;
      await image.decode();
      const ctx = new OffscreenCanvas(image.width, 1).getContext('2d')!;
      ctx.drawImage(image, 0, 0);
      const px = ctx.getImageData(0, 0, image.width, 1).data;
      for (let i = 0; i < px.length; i += 4) if (Math.max(px[i]!, px[i + 1]!, px[i + 2]!) < 110) return true;
      return false;
    }, png);
  };
  for (let tries = 0; !(await inked()); tries++) {
    if (tries > 300) throw new Error('the name was not drawn');
    await frames(page);
  }
  return {page, centre: [cx, cy]};
}

/** Press and release where the pointer is, then wait for deck to give up on a double click. */
async function click(page: Page, settled: (s: Awaited<ReturnType<typeof seen>>) => boolean): Promise<void> {
  await page.mouse.down();
  await page.waitForTimeout(80);
  await page.mouse.up();
  for (let tries = 0; !settled(await seen(page)); tries++) {
    if (tries > 200) return;
    await frames(page);
  }
}

describe('a name on the map', () => {
  it('opens its artifact on a click, where reading a pick back from the GPU is slow', async () => {
    const {page, centre: [cx, cy]} = await open('');
    const offsets = [-90, -33, 0, 29, 88];
    for (const [n, dx] of offsets.entries()) {
      await page.mouse.move(cx + dx - 20, cy + 20);
      await page.mouse.move(cx + dx, cy, {steps: 4});
      // The hover's pick, slow here, has answered before the press.
      await frames(page);
      await click(page, (s) => s.opened.length > n || s.picked.length > 0 || s.missed > 0);
    }
    expect(await seen(page)).toEqual({opened: offsets.map(() => '7'), picked: [], hovered: [], missed: 0});
  }, 120_000);

  it('is under the pointer at every pixel along it, not a mark beneath it', async () => {
    const {page, centre: [cx, cy]} = await open('?fast');
    for (let dx = -140; dx <= 140; dx++) {
      await page.mouse.move(cx + dx, cy);
      await frames(page);
    }
    expect((await seen(page)).hovered).toEqual([]);
    // Past the name's end the marks are under the pointer.
    await page.mouse.move(cx + 200, cy);
    await page.waitForFunction(() => (window as unknown as Seen).hovered.length > 0, undefined, {timeout: 10_000});
  }, 120_000);
});

describe('a click where the marks changed since the pointer stopped', () => {
  it('picks a mark drawn there since, and finds nothing where the mark has gone', async () => {
    const {page, centre: [cx, cy]} = await open('?fast');
    const [x, y] = [cx, cy + 80];
    await page.mouse.move(x, y);
    await frames(page);
    // Over empty ground; then a mark is drawn under the pointer, which does not move.
    await page.evaluate(([px, py]) => (window as unknown as Seen).markAt([[px!, py!]]), [x, y]);
    await frames(page);
    await click(page, (s) => s.picked.length > 0 || s.missed > 0);
    expect(await seen(page)).toMatchObject({picked: ['100'], missed: 0});

    // Over that mark; then it is taken away.
    await page.mouse.move(x + 1, y);
    await frames(page);
    await page.evaluate(() => (window as unknown as Seen).markAt([]));
    await frames(page);
    await click(page, (s) => s.picked.length > 1 || s.missed > 0);
    expect(await seen(page)).toMatchObject({picked: ['100'], missed: 1});
  }, 120_000);
});
