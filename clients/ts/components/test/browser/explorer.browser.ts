import type {Browser, Page} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The explorer's layout in Chromium at the sizes a small or zoomed window gives it: what stays in
 * reach in the narrow layout, where a sheet and a menu end, the order Tab takes from the map, and
 * the height a pointer has to hit.
 */

let served: Awaited<ReturnType<typeof servePage>>;
let origin = '';
let browser: Browser;

beforeAll(async () => {
  served = await servePage('explorer-page.ts');
  ({origin, browser} = served);
});

afterAll(async () => {
  await served?.close();
});

type Box = {left: number; top: number; right: number; bottom: number; width: number; height: number};

/** The page set up as `scene` in a window `width` by `height`, and what it measured. */
async function opened<T>(scene: string, width: number, height: number): Promise<{page: Page; result: T}> {
  const page = await browser.newPage({viewport: {width, height}});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(`${origin}/?case=${scene}`);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as {result?: unknown}).result !== undefined, undefined, {timeout: 50_000})]);
  return {page, result: await page.evaluate(() => (window as unknown as {result: T}).result)};
}

const within = (inner: Box, outer: Box) => inner.left >= outer.left - 0.5 && inner.top >= outer.top - 0.5 && inner.right <= outer.right + 0.5 && inner.bottom <= outer.bottom + 0.5;

describe('the narrow layout in a short window', () => {
  for (const [width, height] of [
    [640, 400],
    [320, 480]
  ] as const) {
    it(`keeps the status strip and the bar of buttons inside the explorer and on top, at ${width} by ${height}`, async () => {
      const {result: r} = await opened<{explorer: Box; strip: Box; tabs: Box; map: Box; stripOnTop: boolean; tabsOnTop: boolean[]; titleCut: boolean; titleLines: number; select: {cut: boolean; lines: number}}>('short', width, height);
      expect(r.strip.height).toBeGreaterThan(0);
      expect(r.tabs.height).toBeGreaterThan(0);
      expect(within(r.strip, r.explorer)).toBe(true);
      expect(within(r.tabs, r.explorer)).toBe(true);
      expect(r.stripOnTop).toBe(true);
      expect(r.tabsOnTop).toEqual([true, true, true]);
      // The map takes the height that is left, and ends where the strip starts.
      expect(r.map.height).toBeGreaterThanOrEqual(100);
      expect(r.map.bottom).toBeLessThanOrEqual(r.strip.top + 0.5);
      // A title too long for one line wraps, and none of it is cut.
      expect(r.titleCut).toBe(false);
      if (width === 320) expect(r.titleLines).toBeGreaterThan(1);
      // So does the name of the view chosen, in its select.
      expect(r.select.cut).toBe(false);
      if (width === 320) expect(r.select.lines).toBeGreaterThan(1);
    });
  }
});

describe('the status strip in the narrow layout', () => {
  it('wraps onto a second line at 320 px with every cell inside it, above the bar of buttons', async () => {
    const {result: r} = await opened<{explorer: Box; row: Box; strip: Box; tabs: Box; cells: Box[]; clipped: boolean; tabsOnTop: boolean[]}>('strip', 320, 480);
    expect(r.clipped).toBe(false);
    // More than one line of cells.
    expect(new Set(r.cells.map((c) => Math.round(c.top))).size).toBeGreaterThan(1);
    // A cell's rule overlaps its neighbour's by a pixel, so the first of a line starts a pixel left of the strip.
    for (const cell of r.cells) expect(within({...cell, left: Math.max(cell.left, r.strip.left)}, r.strip)).toBe(true);
    expect(within(r.strip, r.row)).toBe(true);
    expect(within(r.row, r.explorer)).toBe(true);
    expect(r.row.bottom).toBeLessThanOrEqual(r.tabs.top + 0.5);
    expect(within(r.tabs, r.explorer)).toBe(true);
    expect(r.tabsOnTop).toEqual([true, true, true]);
  });
});

describe('a sheet that reaches over the map’s tools', () => {
  it('makes the map behind it inert, so Tab does not go under it, until it closes', async () => {
    type Open = {inert: boolean; sheet: Box; tools: Box; focused: boolean};
    const {page, result: r} = await opened<{open: Open; closed: {inert: boolean; sheet: boolean}}>('cover', 640, 400);
    expect(r.open.sheet.top).toBeLessThan(r.open.tools.bottom);
    expect(r.open.inert).toBe(true);
    expect(r.open.focused).toBe(true);
    expect(r.closed).toEqual({inert: false, sheet: false});
    // Open again from the keyboard: Shift+Tab from the sheet goes past the map to the heading, not under the sheet.
    const focused = () => page.evaluate(() => (window as unknown as {focused: () => string | null}).focused());
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => (document.querySelector('mosaica-explorer')!.shadowRoot!.querySelector('mosaica-map') as HTMLElement).inert);
    const stops: string[] = [];
    for (let i = 0; i < 12; i++) {
      await page.keyboard.press('Shift+Tab');
      const at = await focused();
      if (at === null) break;
      stops.push(at);
    }
    expect(stops.filter((s) => /^(controls|canvas): /.test(s))).toEqual([]);
  });
});

describe('an open sheet in the narrow layout', () => {
  it('ends above the status strip, takes focus, and shows a picked item', async () => {
    type Sheet = {sheet: Box; stripOnTop: boolean; focused: boolean; inert: boolean};
    const {result: r} = await opened<{sheets: Record<string, Sheet>; strip: Box; tabs: Box; picked: {card: boolean; focused: boolean}}>('sheet', 390, 760);
    for (const [name, s] of Object.entries(r.sheets)) {
      expect(s.sheet.height, name).toBeGreaterThan(0);
      expect(s.sheet.bottom, name).toBeLessThanOrEqual(r.strip.top + 0.5);
      expect(s.stripOnTop, name).toBe(true);
      expect(s.focused, name).toBe(true);
    }
    // A sheet that ends below the map's tools leaves the map above it in use.
    expect(r.sheets['detail']!.inert).toBe(false);
    expect(r.strip.bottom).toBeLessThanOrEqual(r.tabs.top + 0.5);
    expect(r.picked).toEqual({card: true, focused: true});
  });
});

describe('a choice menu', () => {
  for (const [width, height] of [
    [320, 480],
    [1200, 500]
  ] as const) {
    it(`fits the window and scrolls inside it, at ${width} by ${height}`, async () => {
      type Menu = {menu: Box; scrolls: boolean; lastOnTop: boolean; last: Box};
      const {result: r} = await opened<{menus: Record<string, Menu>; window: {width: number; height: number}}>('menu', width, height);
      const window = {left: 0, top: 0, right: r.window.width, bottom: r.window.height, width: r.window.width, height: r.window.height};
      for (const [name, m] of Object.entries(r.menus)) {
        expect(within(m.menu, window), name).toBe(true);
        // Its last entry, focused, is scrolled into view inside it.
        expect(within(m.last, m.menu), name).toBe(true);
        expect(m.lastOnTop, name).toBe(true);
      }
      // Forty-two entries are taller than either window.
      expect(r.menus['colour']!.scrolls).toBe(true);
    });
  }
});

describe('Tab from the map', () => {
  it('grows the card beside a point to hold every field under Show all, where the map has room, with nothing to scroll', async () => {
    const {result: r} = await opened<{before: {height: number; scrolls: boolean}; after: {height: number; scrolls: boolean}}>('grow', 1200, 900);
    expect(r.before.scrolls).toBe(false);
    expect(r.after.height).toBeGreaterThan(r.before.height);
    expect(r.after.scrolls).toBe(false);
  });

  it('reaches the region’s Clear, the selection, the status strip and the card beside the point, each control once', async () => {
    const {page, result: r} = await opened<{callout: boolean; regionTag: boolean}>('order', 1200, 800);
    expect(r).toEqual({callout: true, regionTag: true});
    const focused = () => page.evaluate(() => (window as unknown as {focused: () => string | null}).focused());
    expect(await focused()).toMatch(/^canvas: /);
    const stops: string[] = [];
    for (let i = 0; i < 60; i++) {
      await page.keyboard.press('Tab');
      const at = await focused();
      if (at === null) break;
      stops.push(at);
    }
    const places = stops.map((s) => s.split(':')[0]!);
    // The order the layout reads in: what is drawn on the map, then the card beside the point.
    expect(places[0]).toBe('region-tag');
    expect([...new Set(places)]).toEqual(['region-tag', 'selection-card', 'strip', 'callout']);
    expect(new Set(stops).size).toBe(stops.length);
    // Focus has left the explorer. Shift+Tab walks the same stops back to the map.
    const back: string[] = [];
    for (let i = 0; i <= stops.length; i++) {
      await page.keyboard.press('Shift+Tab');
      back.push((await focused()) ?? 'outside');
    }
    expect(back).toEqual([...[...stops].reverse(), expect.stringMatching(/^canvas: /)]);
  });
});

describe('the Layers popover from the keyboard', () => {
  it('takes focus as it opens, keeps it through its lists, menus and the Edit colours dialog, and closes when focus leaves', async () => {
    const {page} = await opened<object>('layers', 1200, 800);
    const focused = () => page.evaluate(() => (window as unknown as {focused: () => string | null}).focused());
    const shown = () => page.evaluate(() => (window as unknown as {shown: () => Record<string, boolean>}).shown());
    const focus = (part: string) => page.evaluate((p) => (document.querySelector('mosaica-explorer')!.shadowRoot!.querySelector(`[part~="${p}"]`) as HTMLElement).focus(), part);
    const open = {popover: true, list: false, menu: false, editor: false};
    await page.keyboard.press('Enter');
    expect(await shown()).toEqual(open);
    expect(await focused()).toMatch(/^layers-popover: /);
    // A press on the popover's own padding leaves it open.
    const corner = await page.evaluate(() => {
      const r = document.querySelector('mosaica-explorer')!.shadowRoot!.querySelector('[part="layers-popover"]')!.getBoundingClientRect();
      return {x: r.left + 3, y: r.top + 3};
    });
    await page.mouse.click(corner.x, corner.y);
    expect(await shown()).toEqual(open);

    // The list of density colours: the arrow keys move through it and keep focus in it.
    await focus('density-colours');
    const closed = await focused();
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    const first = await focused();
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('ArrowDown');
    expect(await shown()).toEqual({...open, list: true});
    const moved = await focused();
    expect(moved).toMatch(/^layers-popover: /);
    expect(moved).not.toBe(first);
    await page.keyboard.press('Escape');
    expect(await shown()).toEqual(open);
    // The button shows the colours chosen, and has focus again.
    expect(await focused()).not.toBe(closed);
    expect(await focused()).toMatch(/^layers-popover: Colours: /);

    // A menu: Tab closes it and goes back to its button, in the popover.
    await focus('colour-by');
    const button = await focused();
    await page.keyboard.press('ArrowDown');
    expect(await shown()).toEqual({...open, menu: true});
    expect(await focused()).toMatch(/^menu: /);
    await page.keyboard.press('Tab');
    expect(await shown()).toEqual(open);
    expect(await focused()).toBe(button);
    // Escape in a menu closes the menu alone; Escape again closes the popover, with focus on its button.
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Escape');
    expect(await shown()).toEqual(open);
    expect(await focused()).toBe(button);

    // The Edit colours dialog opens over the popover, which is there when the dialog closes.
    await focus('edit-colours');
    await page.keyboard.press('Enter');
    expect(await shown()).toEqual({...open, editor: true});
    await page.keyboard.press('Escape');
    expect(await shown()).toEqual(open);
    expect(await focused()).toMatch(/^layers-popover: Edit colours/);

    // Tab past its last control closes it; focus is on a control, not lost.
    for (let i = 0; i < 40 && (await shown())['popover']; i++) await page.keyboard.press('Tab');
    expect(await shown()).toEqual({...open, popover: false});
    expect(await page.evaluate(() => document.activeElement !== document.body)).toBe(true);

    // Escape from inside gives focus back to the Layers button.
    await focus('layers-toggle');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await page.keyboard.press('Escape');
    expect(await shown()).toEqual({...open, popover: false});
    expect(await focused()).toMatch(/^layers-toggle: /);
  });
});

describe('the Fields sheet with more than it shows at once', () => {
  it('keeps every control Tab stops at clear of its footer, and its footer in reach', async () => {
    type Stop = {name: string; onTop: boolean; aboveFooter: boolean};
    type Sheet = {scrolls: boolean; footer: Box; sheet: Box; footerOnTop: boolean[]};
    const {page} = await opened<object>('footer', 390, 760);
    await page.keyboard.press('Enter');
    await page.evaluate(() => (window as unknown as {settled: () => Promise<void>}).settled());
    const sheet = () => page.evaluate(() => (window as unknown as {sheet: () => Sheet}).sheet());
    expect((await sheet()).scrolls).toBe(true);
    const stops: Stop[] = [];
    for (let i = 0; i < 80; i++) {
      await page.keyboard.press('Tab');
      const stop = await page.evaluate(() => (window as unknown as {stop: () => Stop | null}).stop());
      if (!stop) break;
      stops.push(stop);
    }
    // One fold button a card at the least, and the last of them is past the first screenful.
    expect(stops.length).toBeGreaterThanOrEqual(17);
    expect(stops.filter((s) => !s.onTop || !s.aboveFooter).map((s) => s.name)).toEqual([]);
    // The footer keeps its place at the sheet's bottom edge, whatever is scrolled to.
    const after = await sheet();
    expect(Math.abs(after.footer.bottom - after.sheet.bottom)).toBeLessThanOrEqual(1);
    expect(after.footerOnTop).toEqual([true, true]);
  });
});

describe('Escape while the map shows a tooltip', () => {
  type Over = {tooltip: boolean; card: boolean; sheet: boolean};
  /** The page with two points drawn, and what a test does with it: read what is open, hover a point, press Escape. */
  async function drawn(width: number, height: number) {
    const {page, result} = await opened<{points: [number, number][]}>('tooltip', width, height);
    const settled = () => page.evaluate(() => (window as unknown as {settled: () => Promise<void>}).settled());
    const over = () => page.evaluate(() => (window as unknown as {over: () => Over}).over());
    const shown = () => page.evaluate(() => (window as unknown as {shown: () => Record<string, boolean>}).shown());
    const hover = async ([x, y]: [number, number]) => {
      await page.mouse.move(x - 60, y - 30);
      await page.mouse.move(x, y, {steps: 5});
      await page.waitForFunction(() => (window as unknown as {over: () => Over}).over().tooltip);
    };
    const escape = async () => {
      await page.keyboard.press('Escape');
      await settled();
    };
    const focus = (part: string) => page.evaluate((p) => (document.querySelector('mosaica-explorer')!.shadowRoot!.querySelector(`[part~="${p}"]`) as HTMLElement).focus(), part);
    return {page, points: result.points, settled, over, shown, hover, escape, focus};
  }

  it('dismisses the tooltip and leaves the Layers popover, its menu, its list of colours and the card beside a point open; the next Escape closes each', async () => {
    const {page, points, over, shown, hover, escape, focus} = await drawn(1200, 800);
    const [a, b] = points as [[number, number], [number, number]];
    const open = {popover: true, list: false, menu: false, editor: false};
    // The popover.
    await page.keyboard.press('Enter');
    await hover(a);
    await escape();
    expect((await over()).tooltip).toBe(false);
    expect(await shown()).toEqual(open);
    // A menu.
    await focus('colour-by');
    await page.keyboard.press('ArrowDown');
    await hover(b);
    await escape();
    expect((await over()).tooltip).toBe(false);
    expect(await shown()).toEqual({...open, menu: true});
    await escape();
    expect(await shown()).toEqual(open);
    // The list of density colours.
    await focus('density-colours');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await hover(a);
    await escape();
    expect((await over()).tooltip).toBe(false);
    expect(await shown()).toEqual({...open, list: true});
    await escape();
    expect(await shown()).toEqual(open);
    await escape();
    expect(await shown()).toEqual({...open, popover: false});
    // The card beside a point: a press picks the point, and the pointer then rests on the other.
    await page.mouse.click(a[0], a[1]);
    await page.waitForFunction(() => (window as unknown as {over: () => Over}).over().card);
    await focus('callout');
    await hover(b);
    await escape();
    expect(await over()).toEqual({tooltip: false, card: true, sheet: false});
    await escape();
    expect(await over()).toEqual({tooltip: false, card: false, sheet: false});
  });

  it('dismisses the tooltip and leaves a sheet open in the narrow layout; the next Escape closes the sheet', async () => {
    const {page, points, over, hover, escape} = await drawn(390, 760);
    await page.keyboard.press('Enter');
    await page.evaluate(() => (window as unknown as {settled: () => Promise<void>}).settled());
    expect(await over()).toEqual({tooltip: false, card: false, sheet: true});
    await hover(points[0]!);
    await escape();
    expect(await over()).toEqual({tooltip: false, card: false, sheet: true});
    await escape();
    expect(await over()).toEqual({tooltip: false, card: false, sheet: false});
  });
});

describe('the controls of the Layers popover and the heading', () => {
  it('are 24 px high for a pointer, without one slider’s box over another’s', async () => {
    const {result: r} = await opened<{toggle: boolean[]; sliders: Record<string, number>; overlap: boolean; editColours: boolean[]; select: number}>('targets', 1200, 800);
    expect(r.toggle).toEqual([true, true, true]);
    expect(Object.keys(r.sliders).length).toBeGreaterThanOrEqual(4);
    for (const [name, height] of Object.entries(r.sliders)) expect(height, name).toBeGreaterThanOrEqual(24);
    expect(r.overlap).toBe(false);
    // The switch and Edit colours are drawn smaller, inside a target of that height.
    expect(r.editColours).toEqual([true, true, true]);
    expect(r.select).toBeGreaterThanOrEqual(24);
  });
});
