import type {Browser} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The look of the hierarchy, the item card, the artifact card, the colour editor and its picker
 * at rest, in Chromium's layout, in a wide panel and a compact one: the sizes their styles give
 * their rows, boxes and gaps, a long name on one line, and no text drawn outside the parts that
 * draw text. What a keyboard or a screen reader is given (a 24 px target around a smaller glyph,
 * words that are read and not drawn) changes none of these. Every number is a size a style
 * states, or a line of text at a stated size; none follows from the font's widths.
 */

type Size = [width: number, height: number];
type Result = {
  tree: {
    rows: number[];
    nameLines: number[];
    indents: number[];
    expander: Size;
    gaps: number[];
    dismiss: Size[];
    dismissGlyphs: number[];
    dismissBars: boolean[];
    title: number[];
    search: number[];
    top: number;
    bottom: number;
    more: number[];
    stray: string[];
  };
  cards: {
    full: {close: Size; column: number; sameLine: boolean[]; rowGap: number; actions: number[]; openIcons: number; stray: string[]};
    compact: {close: Size; fields: number; showAll: number[]; open: number; openIcons: number; foot: number[]; stray: string[]};
  };
  card: {title: number; headline: number; rows: number[]; nameIndent: number; nameLines: number[]; labels: number[]; labelLines: number[]; fit: number[]; stray: string[]};
  dialog: {rows: number[]; nameLines: number[]; check: Size; swatch: Size; columns: number[]; selected: number; titleLines: number; stray: string[]};
  picker: {choices: Size[]; rows: number; foot: number; top: number; titleLines: number; reset: number; stray: string[]};
};

/** A line of text at `size` px, at the theme's line height. */
const line = (size: number) => size * 1.45;

let served: Awaited<ReturnType<typeof servePage>>;
let browser: Browser;
/** What the page measured in a panel of each width. */
const at = new Map<number, Result>();

beforeAll(async () => {
  served = await servePage('hierarchy-cards-look-page.ts');
  browser = served.browser;
  for (const panel of [340, 264]) {
    const page = await browser.newPage({viewport: {width: 1200, height: 900}});
    const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
    await page.goto(`${served.origin}/?panel=${panel}`);
    await Promise.race([failed, page.waitForFunction(() => (window as unknown as {result?: unknown}).result !== undefined, undefined, {timeout: 80_000})]);
    at.set(panel, await page.evaluate(() => (window as unknown as {result: Result}).result));
    await page.close();
  }
}, 120_000);

afterAll(async () => {
  await served?.close();
});

/** Run `check` on what was measured in the wide panel and in the compact one. */
function inEachPanel(check: (result: Result, panel: string) => void): void {
  expect([...at.keys()]).toEqual([340, 264]);
  for (const [panel, result] of at) check(result, `${panel} px`);
}

/** Each of `got` within half a pixel of the same place in `want`. */
function near(got: number[], want: number[], where: string): void {
  expect(got.length, where).toBe(want.length);
  got.forEach((value, i) => expect(Math.abs(value - want[i]!), `${where}, ${i}`).toBeLessThan(0.5));
}

describe('the look at rest, in a wide panel and a compact one', () => {
  it('draws every hierarchy row 28 px high on one line, filtered or not, with a 14 px expander, 6 px gaps and 20 px ×s', () => {
    inEachPanel(({tree}, panel) => {
      expect(tree.rows, panel).toEqual([28, 28, 28, 28, 28]);
      expect(tree.nameLines, panel).toEqual([1, 1, 1, 1, 1]);
      near(tree.indents, [2, 16], `${panel} indents`);
      expect(tree.expander[0], panel).toBe(14);
      near(tree.gaps, [6, 6, 6], `${panel} gaps`);
      expect(tree.dismiss, panel).toEqual([[20, 20], [20, 20]]);
      // Each × is one glyph; the highlight's differs by a bar inside its box.
      expect(tree.dismissGlyphs, panel).toEqual([1, 1]);
      expect(tree.dismissBars, panel).toEqual([false, true]);
    });
  });

  it('sets the hierarchy’s heading, search box and tree as far apart as their styles say, with More a line of text', () => {
    inEachPanel(({tree}, panel) => {
      near(tree.title, [line(11), 10], `${panel} heading`);
      near(tree.search, [30, 8], `${panel} search`);
      // The panel's padding, the heading and its margin, the search box and its margin.
      near([tree.top, tree.bottom], [14 + line(11) + 10 + 30 + 8, 14 + 1], `${panel} tree`);
      expect(tree.more[0]!, panel).toBeLessThan(line(12) + 0.5);
    });
  });

  it('draws the item card’s fields in their grid, its buttons at their sizes, and Show all as a line of text with no icon on the compact Open', () => {
    inEachPanel(({cards}, panel) => {
      const {full, compact} = cards;
      expect([full.close, compact.close], panel).toEqual([[24, 24], [24, 24]]);
      // The label's column and the gap after it; each value on its label's line; 6 px between fields.
      near([full.column, full.rowGap], [88 + 12, 6], `${panel} grid`);
      expect(full.sameLine.every(Boolean), panel).toBe(true);
      // Under the fields: 12 px, then Open and Copy id, 28 px high, 8 px apart.
      near(full.actions, [12, 28, 28, 8], `${panel} actions`);
      expect([full.openIcons, compact.openIcons], panel).toEqual([1, 0]);
      expect(compact.fields, panel).toBe(3);
      expect(compact.showAll[0]!, panel).toBeLessThan(line(12) + 0.5);
      expect(compact.showAll[1], panel).toBe(1);
      expect(compact.open, panel).toBe(24);
      // The foot's rule 10 px under the fields, and its content 8 px under the rule.
      near(compact.foot, [10, 1 + 8], `${panel} foot`);
    });
  });

  it('draws the artifact card’s parents and children as 30 px rows with one-line names 22 px in, under labels 12 px below and 4 px above', () => {
    inEachPanel(({card}, panel) => {
      expect(card.title, panel).toBeLessThan(16.5);
      near([card.headline], [10], `${panel} name`);
      expect(card.rows, panel).toEqual([30, 30, 30]);
      near([card.nameIndent], [22], `${panel} indent`);
      expect(card.nameLines, panel).toEqual([1, 1]);
      near(card.labels, [12, 4, 12, 4], `${panel} labels`);
      near(card.labelLines, [line(11), line(11)], `${panel} label lines`);
      near(card.fit, [12, 28, 8], `${panel} buttons`);
    });
  });

  it('draws the colour editor’s rows 40 px high with one-line names, a 14 px checkbox and a 22 px swatch in their columns', () => {
    inEachPanel(({dialog}, panel) => {
      expect(dialog.rows, panel).toEqual([40, 40]);
      expect(dialog.nameLines, panel).toEqual([1, 1]);
      expect([dialog.check, dialog.swatch], panel).toEqual([[14, 14], [22, 22]]);
      // The checkbox's column and the swatch's, each with the 10 px gap after it.
      near(dialog.columns, [18 + 10, 26 + 10], `${panel} columns`);
      // The bar: Set colour's 18 px swatch inside 6 px of padding.
      near([dialog.selected], [6 + 18 + 6], `${panel} bar`);
      expect(dialog.titleLines, panel).toBe(1);
    });
  });

  it('draws a ten-colour palette in the picker as two rows of 18 px squares, with nothing under the hex field', () => {
    inEachPanel(({picker}, panel) => {
      // Ten in the picker's 234 px, 6 px apart.
      expect(picker.choices, panel).toEqual(Array.from({length: 20}, () => [(234 - 9 * 6) / 10, (234 - 9 * 6) / 10]));
      expect(picker.rows, panel).toBe(2);
      // The custom controls' padding and the picker's border.
      near([picker.foot, picker.top], [14 + 1, 0], `${panel} edges`);
      expect(picker.titleLines, panel).toBe(1);
      expect(picker.reset, panel).toBe(16);
    });
  });

  it('draws no text outside the parts that draw text', () => {
    inEachPanel(({tree, cards, card, dialog, picker}, panel) => {
      expect([tree.stray, cards.full.stray, cards.compact.stray, card.stray, dialog.stray, picker.stray], panel).toEqual([[], [], [], [], [], []]);
    });
  });
});
