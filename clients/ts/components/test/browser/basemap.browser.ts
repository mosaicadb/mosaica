import type {Browser} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * A map that leaves the page and comes back draws its basemap again in its new deck.gl canvas, and
 * deck.gl raises no error over a layer it finalised.
 */

let served: Awaited<ReturnType<typeof servePage>>;
let origin = '';
let browser: Browser;

beforeAll(async () => {
  served = await servePage('basemap-page.ts');
  ({origin, browser} = served);
});

afterAll(async () => {
  await served?.close();
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
