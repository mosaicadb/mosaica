import type {Browser} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The item card's Show all, in Chromium's layout: on its own, and beside a point in the explorer,
 * where it keeps clear of the cards and tools over the map.
 */

let served: Awaited<ReturnType<typeof servePage>>;
let origin = '';
let browser: Browser;

beforeAll(async () => {
  served = await servePage('card-page.ts');
  ({origin, browser} = served);
});

afterAll(async () => {
  await served?.close();
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
