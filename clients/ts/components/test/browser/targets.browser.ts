import type {Browser} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The hierarchy, the colour editor, the colour picker and the item card in Chromium's layout: how
 * large their pointer targets are, that none lies over another, that text is drawn whole or has a
 * control that shows it whole, and what only a real cascade shows: a status region that has a box
 * before it speaks, a row's buttons standing in for its counts, and the picker's place in a narrow
 * window. Nothing here pins a length that follows from the font: what depends on text is measured
 * in the page, and every check is made in four fonts.
 */

/** One row of the hierarchy with ordinary names, as the page measured it. */
type Row = {
  name: string;
  /** The lines the name is drawn on. */
  lines: number;
  /** The width the name's button needs to hold the name on one line, the width the row leaves it beside the expander and the counts, and the width it has. */
  needs: number;
  room: number;
  has: number;
  countsOnFirstLine: boolean;
  countsBesideName: boolean;
  /** The row's height beyond what its name's lines, or its least height, need. */
  spare: number;
};

type Result = {
  tree: {
    small: string[];
    overlaps: string[];
    outside: string[];
    cut: string[];
    swap: string[];
    covered: string[];
    status: {rendered: boolean; said: string};
    /** The rows with ordinary names, without a filter and with one. */
    rows: {plain: Row[]; filtered: Row[]};
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

/** What a real pointer's press on a row's Filter button, shown by hovering the row, set on the store. */
type Pressed = {before: string[]; after: string[]};

/** The fonts the page is drawn in, by the query that asks for each: the theme's, a serif, a monospace, and the theme's 15% larger. */
const LOOKS = {'the theme’s font': '', 'a serif': '?font=serif', 'a monospace': '?font=monospace', 'a larger font': '?scale=1.15'};

let served: Awaited<ReturnType<typeof servePage>>;
let browser: Browser;
/** What the page measured at 1200 px in each font, and at 800 px in the theme's. */
const drawn = new Map<string, Result & {pressed: Pressed}>();
let narrow: Result;

/** What the page measures in a window `width` px wide, drawn in the font `look` asks for. */
async function measured(width: number, look = ''): Promise<Result & {pressed: Pressed}> {
  const page = await browser.newPage({viewport: {width, height: 900}});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(`${served.origin}/${look}`);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as {result?: unknown}).result !== undefined, undefined, {timeout: 80_000})]);
  const result = await page.evaluate(() => (window as unknown as {result: Result}).result);
  type Places = {name: {x: number; y: number}; filter: {x: number; y: number}; pressed: string[]};
  const row = () => page.evaluate(() => (window as unknown as {plainRow(): Places}).plainRow());
  const {name, filter, pressed: before} = await row();
  await page.mouse.move(name.x, name.y);
  await page.mouse.move(filter.x, filter.y, {steps: 4});
  await page.mouse.click(filter.x, filter.y);
  const pressed = {before, after: (await row()).pressed};
  await page.close();
  return {...result, pressed};
}

/** Run `check` on what was measured in each font. */
function inEachFont(check: (at: Result & {pressed: Pressed}, font: string) => void): void {
  expect([...drawn.keys()]).toEqual(Object.keys(LOOKS));
  for (const [font, at] of drawn) check(at, font);
}

beforeAll(async () => {
  served = await servePage('targets-page.ts');
  browser = served.browser;
  for (const [font, look] of Object.entries(LOOKS)) drawn.set(font, await measured(1200, look));
  narrow = await measured(800);
}, 120_000);

afterAll(async () => {
  await served?.close();
});

describe('pointer targets and whole text, in each of four fonts', () => {
  it('gives every button of a hierarchy row a 24 px target inside the row, clear of the others, and wraps a long name', () => {
    inEachFont(({tree}, font) => {
      expect(tree.small, font).toEqual([]);
      expect(tree.overlaps, font).toEqual([]);
      expect(tree.outside, font).toEqual([]);
      expect(tree.cut, font).toEqual([]);
    });
  });

  it('draws a hierarchy row’s counts at rest, its buttons in their place while focus is in the row, and the counts again after Escape', () => {
    inEachFont(({tree}, font) => expect(tree.swap, font).toEqual([]));
  });

  it('puts a hierarchy row’s buttons above its counts while they are shown, so a pointer’s press on Filter filters', () => {
    inEachFont(({tree, pressed}, font) => {
      expect(tree.covered, font).toEqual([]);
      expect(pressed.after, font).toEqual([...pressed.before, 'filter 99, filter 2']);
    });
  });

  it('gives a hierarchy row’s name all the room beside its expander and its counts, on one line where it fits there, with the counts beside it and never on a line of their own', () => {
    inEachFont(({tree}, font) => {
      const {plain, filtered} = tree.rows;
      for (const [filter, rows] of [['no filter', plain], ['a filter', filtered]] as const) {
        expect(rows.map((row) => row.name.length), font).toEqual([7, 9, 14, 16, 23, 24, 104]);
        for (const row of rows) {
          const at = `${row.name.slice(0, 24)}, ${filter}, ${font}`;
          // The buttons, not shown, take no room: the name has all that the expander and the counts leave.
          expect(Math.abs(row.has - row.room), at).toBeLessThan(1);
          // A name that fits there is on one line; one that does not wraps.
          if (row.needs < row.room - 1) expect(row.lines, at).toBe(1);
          if (row.needs > row.room + 1) expect(row.lines, at).toBeGreaterThan(1);
          // The counts stand beside the name, on its first line where it has one line or two (past
          // that they are centred on it), and the row is as tall as the name's lines and no taller.
          expect(row.countsBesideName, at).toBe(true);
          if (row.lines <= 2) expect(row.countsOnFirstLine, at).toBe(true);
          expect(row.spare, at).toBeLessThan(1);
        }
      }
      // Neither branch is empty: a short name fits on one line without a filter, and a long one wraps.
      expect(plain[0]!.lines, font).toBe(1);
      expect(plain[0]!.needs, font).toBeLessThan(plain[0]!.room - 1);
      expect(plain.at(-1)!.lines, font).toBeGreaterThan(1);
    });
  });

  it('gives the hierarchy’s status region a box before it has anything to say', () => {
    inEachFont(({tree}, font) => expect(tree.status, font).toEqual({rendered: true, said: ''}));
  });

  it('gives the colour editor’s checkboxes, swatches and bar buttons 24 px targets, and wraps a long name', () => {
    inEachFont(({editor}, font) => {
      expect(editor.small, font).toEqual([]);
      expect(editor.overlaps, font).toEqual([]);
      expect(editor.nameCut, font).toBe(false);
      expect(editor.cornerTicks, font).toBe(true);
    });
  });

  it('keeps every colour of a 22-colour palette, and its lighter row, at least 24 px square inside the picker', () => {
    inEachFont(({picker}, font) => {
      expect(picker.count, font).toBe(44);
      expect(picker.small, font).toEqual([]);
      expect(picker.overlaps, font).toEqual([]);
      expect(picker.wide, font).toBe(false);
      expect(picker.hueTarget, font).toBe(true);
    });
  });

  it('opens the picker clear of the row it was opened from, in a wide window and in one 800 px wide', () => {
    inEachFont(({picker}, font) => {
      expect(picker.coversRow, font).toEqual([]);
      expect(picker.inside, font).toBe(true);
    });
    expect(narrow.picker.coversRow).toEqual([]);
    expect(narrow.picker.inside).toBe(true);
    expect(narrow.picker.small).toEqual([]);
  });

  it('shows a long value whole from its own button, and a compact card’s headline whole from Show all', () => {
    inEachFont(({cards}, font) => {
      expect(cards.moreBox, font).toEqual([]);
      expect(cards.proseAfter, font).toBeGreaterThan(cards.proseBefore);
      expect(cards.proseText, font).toBe(true);
      expect(cards.showAllBox, font).toEqual([]);
      expect(cards.headlineBefore.whole, font).toBe(false);
      expect(cards.headlineAfter.whole, font).toBe(true);
      expect(cards.headlineAfter.height, font).toBeGreaterThan(cards.headlineBefore.height);
    });
  });
});
