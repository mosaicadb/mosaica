import type {Browser} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/** A layer's card in a column that scrolls, stepping into a cluster whose children take a while to arrive. */

let served: Awaited<ReturnType<typeof servePage>>;
let origin = '';
let browser: Browser;

beforeAll(async () => {
  served = await servePage('step-scroll-page.ts');
  ({origin, browser} = served);
});

afterAll(async () => {
  await served?.close();
});

describe('a step into a cluster low in a scrolled column', () => {
  it('keeps the column where it was while the children are on their way, and keeps the card in view once they land', async () => {
    const page = await browser.newPage({viewport: {width: 600, height: 600}});
    const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
    await page.goto(origin);
    await Promise.race([failed, page.waitForFunction(() => (window as unknown as {result?: unknown}).result !== undefined, undefined, {timeout: 30_000})]);
    const r = await page.evaluate(() => (window as unknown as {result: {before: number; waiting: number; after: number; landed: boolean}}).result);
    expect(r.before).toBeGreaterThan(600);
    expect(r.waiting).toBe(r.before);
    expect(r.landed).toBe(true);
    // The children start at the card's top, which the column scrolls back to; it does not go back to its own top.
    expect(r.after).toBeGreaterThan(400);
  });
});
