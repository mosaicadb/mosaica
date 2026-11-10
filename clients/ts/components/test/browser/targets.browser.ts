import type {Browser} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The hierarchy, the colour editor, the colour picker and the item card in Chromium's layout: how
 * large their pointer targets are, that none lies over another, that text is drawn whole or has a
 * control that shows it whole, and what only a real cascade shows: a status region that has a box
 * before it speaks, a row's buttons standing in for its counts, and the picker's place in a narrow
 * window.
 */

type Result = {
  tree: {
    small: string[];
    overlaps: string[];
    outside: string[];
    cut: string[];
    swap: string[];
    status: {rendered: boolean; said: string};
    /** Each row's height by the length of its name, without a filter and with one. */
    rowHeights: {plain: Record<string, number>; filtered: Record<string, number>};
  };
  editor: {small: string[]; overlaps: string[]; nameCut: boolean; cornerTicks: boolean};
  picker: {coversRow: string[]; inside: boolean; count: number; small: string[]; overlaps: string[]; wide: boolean; hueTarget: boolean};
  cards: {
    moreBox: string[];
    proseBefore: number;
    proseAfter: number;
    proseText: boolean;
    showAllBox: string[];
    headlineBefore: {height: number; whole: boolean};
    headlineAfter: {height: number; whole: boolean};
  };
};

let served: Awaited<ReturnType<typeof servePage>>;
let browser: Browser;
let r: Result;
let narrow: Result;

/** What the page measures in a window `width` px wide. */
async function measured(width: number): Promise<Result> {
  const page = await browser.newPage({viewport: {width, height: 900}});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(served.origin);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as {result?: unknown}).result !== undefined, undefined, {timeout: 80_000})]);
  const result = await page.evaluate(() => (window as unknown as {result: Result}).result);
  await page.close();
  return result;
}

beforeAll(async () => {
  served = await servePage('targets-page.ts');
  browser = served.browser;
  r = await measured(1200);
  narrow = await measured(800);
}, 120_000);

afterAll(async () => {
  await served?.close();
});

describe('pointer targets and whole text', () => {
  it('gives every button of a hierarchy row a 24 px target inside the row, clear of the others, and wraps a long name', () => {
    expect(r.tree.small).toEqual([]);
    expect(r.tree.overlaps).toEqual([]);
    expect(r.tree.outside).toEqual([]);
    expect(r.tree.cut).toEqual([]);
  });

  it('draws a hierarchy row’s counts at rest, its buttons in their place while focus is in the row, and the counts again after Escape', () => {
    expect(r.tree.swap).toEqual([]);
  });

  it('keeps a hierarchy row to one line at 300 px for a name of up to 23 characters, or 9 beside a filter’s two counts, and a longer one to two', () => {
    // Keyed by the name's length. The counts are 2,400,000, and 31,200 of them under the filter.
    const {plain, filtered} = r.tree.rowHeights;
    for (const length of [7, 9, 14, 16, 23]) expect(plain[length], `${length} characters`).toBe(28);
    for (const length of [7, 9]) expect(filtered[length], `${length} characters, filtered`).toBe(28);
    // A longer name takes a second line beside the counts, which do not take a line of their own.
    expect(plain[24]).toBeLessThan(48);
    for (const length of [14, 16, 23, 24]) expect(filtered[length], `${length} characters, filtered`).toBeLessThan(48);
    expect(plain[104]).toBeGreaterThan(48);
  });

  it('gives the hierarchy’s status region a box before it has anything to say', () => {
    expect(r.tree.status).toEqual({rendered: true, said: ''});
  });

  it('gives the colour editor’s checkboxes, swatches and bar buttons 24 px targets, and wraps a long name', () => {
    expect(r.editor.small).toEqual([]);
    expect(r.editor.overlaps).toEqual([]);
    expect(r.editor.nameCut).toBe(false);
    expect(r.editor.cornerTicks).toBe(true);
  });

  it('keeps every colour of a 22-colour palette, and its lighter row, at least 24 px square inside the picker', () => {
    expect(r.picker.count).toBe(44);
    expect(r.picker.small).toEqual([]);
    expect(r.picker.overlaps).toEqual([]);
    expect(r.picker.wide).toBe(false);
    expect(r.picker.hueTarget).toBe(true);
  });

  it('opens the picker clear of the row it was opened from, in a wide window and in one 800 px wide', () => {
    for (const at of [r, narrow]) {
      expect(at.picker.coversRow).toEqual([]);
      expect(at.picker.inside).toBe(true);
      expect(at.picker.small).toEqual([]);
    }
  });

  it('shows a long value whole from its own button, and a compact card’s headline whole from Show all', () => {
    expect(r.cards.moreBox).toEqual([]);
    expect(r.cards.proseAfter).toBeGreaterThan(r.cards.proseBefore);
    expect(r.cards.proseText).toBe(true);
    expect(r.cards.showAllBox).toEqual([]);
    expect(r.cards.headlineBefore.whole).toBe(false);
    expect(r.cards.headlineAfter.whole).toBe(true);
    expect(r.cards.headlineAfter.height).toBeGreaterThan(r.cards.headlineBefore.height);
  });
});
