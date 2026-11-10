import type {Browser, Page} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The shared theme under forced colours in Chromium: a pressed, checked, selected or chosen control
 * takes the highlight colour as its background, and its focus ring must still show on it.
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

type Ring = {focusVisible: boolean; style: string; width: string; offset: string; ring: string; on: string; highlight: string; highlightText: string};

async function open(): Promise<Page> {
  const page = await browser.newPage({viewport: {width: 700, height: 600}, forcedColors: 'active'});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(origin);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as {ready?: boolean}).ready === true, undefined, {timeout: 60_000})]);
  expect(await page.evaluate(() => matchMedia('(forced-colors: active)').matches)).toBe(true);
  return page;
}

/**
 * The ring drawn on the element `selector` finds in `tag`'s shadow root, and the background it lies
 * on: the element's own for a ring inside its edge, else that of the nearest highlighted ancestor.
 */
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
      const inside = parseFloat(style.outlineOffset) < 0;
      const ground = inside ? el : ((el.parentElement?.closest('.on, [aria-selected="true"], [aria-pressed="true"], [aria-checked="true"]') as HTMLElement | null) ?? el);
      return {
        focusVisible: el.matches(':focus-visible'),
        style: style.outlineStyle,
        width: style.outlineWidth,
        offset: style.outlineOffset,
        ring: style.outlineColor,
        on: getComputedStyle(ground).backgroundColor,
        highlight: system('Highlight'),
        highlightText: system('HighlightText')
      };
    },
    [tag, selector]
  ) as Promise<Ring>;

describe('a focus ring under forced colours', () => {
  it('shows on a checked, a pressed and a chosen control, inside its edge, and round a button in a chosen row', async () => {
    const page = await open();
    for (const id of ['checked', 'pressed', 'inner']) {
      await page.keyboard.press('Tab');
      const ring = await ringOf(page, 'theme-probe', `#${id}`);
      expect(ring.focusVisible, id).toBe(true);
      expect([ring.style, ring.width], id).toEqual(['solid', '2px']);
      expect(ring.on, id).toBe(ring.highlight);
      expect(ring.ring, id).toBe(ring.highlightText);
      expect(ring.ring, id).not.toBe(ring.on);
      // The checked and the pressed control are ringed inside their edges, where the highlight is.
      if (id !== 'inner') expect(ring.offset, id).toBe('-2px');
    }

    // A control in none of those states keeps the highlight colour for its ring.
    await page.keyboard.press('Tab');
    const plain = await ringOf(page, 'theme-probe', '#plain');
    expect(plain.focusVisible).toBe(true);
    expect(plain.ring).toBe(plain.highlight);
    expect(plain.ring).not.toBe(plain.on);
  }, 120_000);

  it('shows on a selected suggestion the arrow keys reached, and on one that is not selected', async () => {
    const page = await open();
    const reached = await ringOf(page, 'theme-probe', '#reached');
    expect(reached.on).toBe(reached.highlight);
    expect(reached.ring).toBe(reached.highlightText);
    const passed = await ringOf(page, 'theme-probe', '#passed');
    expect(passed.ring).toBe(passed.highlight);
    expect(passed.ring).not.toBe(passed.on);
  }, 120_000);

  it('shows on the map’s pressed tool', async () => {
    const page = await open();
    await page.evaluate(() => (document.querySelector('mosaica-map')!.shadowRoot!.querySelector('[part="controls"] button') as HTMLElement).focus());
    // A key press makes the focus one to draw.
    await page.keyboard.press('Shift');
    const tool = await ringOf(page, 'mosaica-map', '[part="controls"] button[aria-pressed="true"]');
    expect(tool.focusVisible).toBe(true);
    expect(tool.on).toBe(tool.highlight);
    expect(tool.ring).toBe(tool.highlightText);
    expect(tool.offset).toBe('-2px');
  }, 120_000);
});
