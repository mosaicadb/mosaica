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

type Seen = {opened: string[]; picked: string[]; hovered: string[]; centre?: [number, number]};

/** The page, once the map is up, and the name's centre in the page. */
async function open(search: string): Promise<{page: Page; centre: [number, number]}> {
  const page = await browser.newPage({viewport: {width: 800, height: 500}});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(origin + search);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as Seen).centre !== undefined, undefined, {timeout: 60_000})]);
  // The name's glyphs are rasterised on its first frames.
  await page.waitForTimeout(2000);
  return {page, centre: await page.evaluate(() => (window as unknown as Seen).centre!)};
}

const seen = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as Seen;
    return {opened: w.opened, picked: w.picked, hovered: w.hovered};
  });

describe('a name on the map', () => {
  it('opens its artifact on a click, where reading a pick back from the GPU is slow', async () => {
    const {page, centre: [cx, cy]} = await open('');
    const offsets = [-90, -33, 0, 29, 88];
    for (const dx of offsets) {
      await page.mouse.move(cx + dx - 20, cy + 20);
      await page.mouse.move(cx + dx, cy, {steps: 4});
      // The hover's pick, slow here, has answered before the press.
      await page.waitForTimeout(900);
      await page.mouse.down();
      await page.waitForTimeout(80);
      await page.mouse.up();
      // deck holds a click back until a second one cannot follow it as a double click.
      await page.waitForTimeout(700);
    }
    expect(await seen(page)).toEqual({opened: offsets.map(() => '7'), picked: [], hovered: []});
  }, 120_000);

  it('is under the pointer at every pixel along it, not a mark beneath it', async () => {
    const {page, centre: [cx, cy]} = await open('?fast');
    for (let dx = -140; dx <= 140; dx++) {
      await page.mouse.move(cx + dx, cy);
      // A frame for deck to pick at the pointer, and one for the answer.
      await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    }
    expect((await seen(page)).hovered).toEqual([]);
    // Past the name's end the marks are under the pointer.
    await page.mouse.move(cx + 200, cy);
    await page.waitForFunction(() => (window as unknown as Seen).hovered.length > 0, undefined, {timeout: 10_000});
  }, 120_000);
});
