import type {Browser} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The hierarchy, the colour editor, the colour picker and the item card in Chromium's layout: how
 * large their pointer targets are, that none lies over another, and that text is drawn whole or
 * has a control that shows it whole.
 */

type Result = {
  tree: {small: string[]; overlaps: string[]; outside: string[]; cut: string[]};
  editor: {small: string[]; overlaps: string[]; nameCut: boolean; cornerTicks: boolean};
  picker: {count: number; small: string[]; overlaps: string[]; wide: boolean; hueTarget: boolean};
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

beforeAll(async () => {
  served = await servePage('targets-page.ts');
  browser = served.browser;
  const page = await browser.newPage({viewport: {width: 1200, height: 900}});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(served.origin);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as {result?: unknown}).result !== undefined, undefined, {timeout: 80_000})]);
  r = await page.evaluate(() => (window as unknown as {result: Result}).result);
}, 120_000);

afterAll(async () => {
  await served?.close();
});

describe('pointer targets and whole text', () => {
  it('gives every button of a hierarchy row a 24 px target inside the row, clear of the others and of the counts, and wraps a long name', () => {
    expect(r.tree.small).toEqual([]);
    expect(r.tree.overlaps).toEqual([]);
    expect(r.tree.outside).toEqual([]);
    expect(r.tree.cut).toEqual([]);
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
