import type {Browser} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * A point with no value under sizing by a column, drawn by the deck layer in Chromium: a ring whose
 * inside is clear to the eye and still picks the point.
 */

let served: Awaited<ReturnType<typeof servePage>>;
let origin = '';
let browser: Browser;

beforeAll(async () => {
  served = await servePage('rings-page.ts');
  ({origin, browser} = served);
});

afterAll(async () => {
  await served?.close();
});

describe('a point with no value under sizing', () => {
  it('draws as a ring whose inside is clear, and picks across its whole disc', async () => {
    const page = await browser.newPage({viewport: {width: 400, height: 400}});
    const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
    await page.goto(origin);
    await Promise.race([failed, page.waitForFunction(() => (window as unknown as {result?: unknown}).result !== undefined, undefined, {timeout: 30_000})]);
    const r = await page.evaluate(() => (window as unknown as {result: Record<string, number | string>}).result);
    // The valued point is a filled disc; the ring is drawn at its edge, 8 px out, and clear inside.
    expect(r['valued']).toBeGreaterThan(150);
    expect(r['ringEdge']).toBeGreaterThan(100);
    expect(r['ringCentre']).toBe(0);
    expect(r['outside']).toBe(0);
    // Picking finds the point anywhere on the disc the ring bounds, and not past it.
    expect([r['pickCentre'], r['pickEdge'], r['pickOutside']]).toEqual(['22', '22', 'miss']);
  });
});
