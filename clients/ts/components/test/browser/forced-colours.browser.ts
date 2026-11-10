import type {Browser, Page} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The shared theme under forced colours in Chromium: a pressed, checked, selected or chosen control
 * takes the highlight colour as its background, and its focus ring must still show on it. The
 * ring's colour may be the page's own, so each ring is read off the screen with what is drawn on
 * either side of it.
 */

let served: Awaited<ReturnType<typeof servePage>>;
let origin = '';
let browser: Browser;

beforeAll(async () => {
  served = await servePage('forced-colours-page.ts');
  ({origin, browser} = served);
});

afterAll(async () => {
  await served?.close();
});

type Ring = {focusVisible: boolean; style: string; width: string; offset: string; ring: string; highlight: string; highlightText: string; left: number; middle: number};

async function open(): Promise<Page> {
  const page = await browser.newPage({viewport: {width: 700, height: 600}, forcedColors: 'active'});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(origin);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as {ready?: boolean}).ready === true, undefined, {timeout: 60_000})]);
  expect(await page.evaluate(() => matchMedia('(forced-colors: active)').matches)).toBe(true);
  return page;
}

/** The ring the styles give the element `selector` finds in `tag`'s shadow root, the highlight pair, and where its left edge and middle line are. */
const ringOf = (page: Page, tag: string, selector: string) =>
  page.evaluate(
    ([t, s]) => {
      const el = document.querySelector(t!)!.shadowRoot!.querySelector(s!) as HTMLElement;
      const style = getComputedStyle(el);
      const system = (colour: string) => {
        const probe = document.createElement('span');
        probe.style.color = colour;
        document.body.append(probe);
        const rgb = getComputedStyle(probe).color;
        probe.remove();
        return rgb;
      };
      const box = el.getBoundingClientRect();
      return {
        focusVisible: el.matches(':focus-visible'),
        style: style.outlineStyle,
        width: style.outlineWidth,
        offset: style.outlineOffset,
        ring: style.outlineColor,
        highlight: system('Highlight'),
        highlightText: system('HighlightText'),
        left: Math.round(box.left),
        middle: Math.round(box.top + box.height / 2)
      };
    },
    [tag, selector]
  ) as Promise<Ring>;

/** The colours drawn along the element's middle line, each `rgb(r, g, b)`, at these distances in pixels right of its left edge. */
async function drawnAt(page: Page, ring: Ring, distances: number[]): Promise<string[]> {
  const from = Math.min(...distances);
  const png = (await page.screenshot({clip: {x: ring.left + from, y: ring.middle, width: Math.max(...distances) - from + 1, height: 1}})).toString('base64');
  return page.evaluate(
    async ([data, at]) => {
      const image = new Image();
      image.src = `data:image/png;base64,${data}`;
      await image.decode();
      const ctx = new OffscreenCanvas(image.width, 1).getContext('2d')!;
      ctx.drawImage(image, 0, 0);
      const px = ctx.getImageData(0, 0, image.width, 1).data;
      return (at as number[]).map((i) => `rgb(${px[i * 4]}, ${px[i * 4 + 1]}, ${px[i * 4 + 2]})`);
    },
    [png, distances.map((d) => d - from)] as const
  );
}

/**
 * A ring set in from the control's edge: the highlight between the edge and the ring, the ring in
 * the highlight's text colour, and the highlight again inside it.
 */
async function expectRingInside(page: Page, ring: Ring, name: string): Promise<void> {
  expect([ring.style, ring.width, ring.offset], name).toEqual(['solid', '2px', '-4px']);
  expect(ring.ring, name).toBe(ring.highlightText);
  expect(ring.ring, name).not.toBe(ring.highlight);
  const [page0, outside, drawn, inside] = await drawnAt(page, ring, [-3, 1, 3, 6]);
  expect(drawn, name).toBe(ring.highlightText);
  // The control's fill on both sides of the ring, and neither the ring's colour nor the page's.
  expect(outside, name).toBe(inside);
  expect(outside, name).not.toBe(drawn);
  expect(outside, name).not.toBe(page0);
}

describe('a focus ring under forced colours', () => {
  it('shows on a checked and a pressed control with the highlight on both sides of it, and round a button in a chosen row', async () => {
    const page = await open();
    for (const id of ['checked', 'pressed']) {
      await page.keyboard.press('Tab');
      const ring = await ringOf(page, 'theme-probe', `#${id}`);
      expect(ring.focusVisible, id).toBe(true);
      await expectRingInside(page, ring, id);
    }

    // The button's ring is outside it, on the chosen row's highlight.
    await page.keyboard.press('Tab');
    const inner = await ringOf(page, 'theme-probe', '#inner');
    expect(inner.focusVisible).toBe(true);
    expect([inner.style, inner.width, inner.offset]).toEqual(['solid', '2px', '1px']);
    const [row, drawn, gap] = await drawnAt(page, inner, [-5, -2, -1]);
    expect(drawn).toBe(inner.highlightText);
    expect(row).toBe(gap);
    expect(row).not.toBe(drawn);

    // A control in none of those states keeps the highlight colour for its ring, on its own background.
    await page.keyboard.press('Tab');
    const plain = await ringOf(page, 'theme-probe', '#plain');
    expect(plain.focusVisible).toBe(true);
    expect(plain.ring).toBe(plain.highlight);
    const [ringed, within] = await drawnAt(page, plain, [1, 6]);
    expect(within).not.toBe(ringed);
  }, 120_000);

  it('shows on a selected suggestion the arrow keys reached, and on one that is not selected', async () => {
    const page = await open();
    await expectRingInside(page, await ringOf(page, 'theme-probe', '#reached'), 'reached');
    const passed = await ringOf(page, 'theme-probe', '#passed');
    expect(passed.ring).toBe(passed.highlight);
    const [ringed, within] = await drawnAt(page, passed, [1, 6]);
    expect(within).not.toBe(ringed);
  }, 120_000);

  it('shows on the map’s pressed tool', async () => {
    const page = await open();
    await page.evaluate(() => (document.querySelector('mosaica-map')!.shadowRoot!.querySelector('[part="controls"] button') as HTMLElement).focus());
    // A key press makes the focus one to draw.
    await page.keyboard.press('Shift');
    const tool = await ringOf(page, 'mosaica-map', '[part="controls"] button[aria-pressed="true"]');
    expect(tool.focusVisible).toBe(true);
    await expectRingInside(page, tool, 'the pressed tool');
  }, 120_000);
});
