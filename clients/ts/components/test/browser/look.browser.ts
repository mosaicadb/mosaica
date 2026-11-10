import type {Browser} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The look of the status strip, the selection card and a bare map at rest, in Chromium, pinned to
 * what the same page measured on the elements before they were made to announce themselves and
 * to work from the keyboard: the same boxes, and no text drawn that was not drawn then. The
 * numbers are sizes a font's width does not change: paddings, rules, fixed heights, and lines
 * whose height is their line height.
 */

type Strip = {
  height: number;
  cells: number;
  oneLine: boolean;
  abutting: boolean;
  rules: string[];
  paddings: string[];
  stateCell: {width: number; text: string; dot: {width: number; height: number} | null};
  text: string;
};
type Card = {
  width: number;
  titleToCounts: number;
  stateLine: {height: number; text: string; title: string | null};
  countRows: number;
  countRowPitch: number;
  countsToLabel: number;
  items: {height: number; text: string; textLeft: number}[];
  actions: number[];
  text: string;
};
type Look = {
  strips: Record<'shown' | 'shownCompact' | 'updating' | 'updatingCompact', Strip>;
  cards: Record<'wide' | 'compact', Card>;
  map: {
    tag: {height: number; clear: {width: number; height: number}; clearToTagRight: number; text: string};
    clearTarget: {at11: boolean[]; at13: boolean[]};
    key: {height: number; text: string};
    tools: string[];
    overlayText: string;
    noteText: string;
    text: string[];
  };
};

let served: Awaited<ReturnType<typeof servePage>>;
let browser: Browser;
let look: Look;

beforeAll(async () => {
  served = await servePage('look-page.ts');
  browser = served.browser;
  const page = await browser.newPage({viewport: {width: 900, height: 1400}});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(served.origin);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as {look?: unknown}).look !== undefined, undefined, {timeout: 60_000})]);
  look = await page.evaluate(() => (window as unknown as {look: Look}).look);
});

afterAll(async () => {
  await served?.close();
});

/** Text with its white space taken out, which a template's line breaks change and the eye does not see. */
const words = (text: string) => text.replace(/\s+/g, '');

describe('the status strip at rest', () => {
  it('is one line of cells as tall as it was, up to date and updating alike', () => {
    for (const name of ['shown', 'updating'] as const) {
      const strip = look.strips[name];
      // 12 px text on a line 1.45 high, 7 px above and below, and a 1 px edge each side.
      expect(strip.height, name).toBeCloseTo(33.4, 0);
      expect([strip.cells, strip.oneLine, strip.abutting], name).toEqual([3, true, true]);
      expect(strip.rules, name).toEqual(['0px', '1px', '1px']);
      expect(strip.paddings, name).toEqual(['12px', '12px', '12px']);
    }
    for (const name of ['shownCompact', 'updatingCompact'] as const) {
      const strip = look.strips[name];
      expect(strip.height, name).toBeCloseTo(33.4, 0);
      expect([strip.cells, strip.oneLine, strip.abutting], name).toEqual([2, true, true]);
      expect(strip.rules, name).toEqual(['0px', '0px']);
      expect(strip.paddings, name).toEqual(['12px', '0px']);
    }
  });

  it('draws its state as a dot alone, in a cell no wider than the dot and its padding', () => {
    for (const [name, width] of [['shown', 38], ['updating', 38], ['shownCompact', 33], ['updatingCompact', 33]] as const) {
      const cell = look.strips[name].stateCell;
      expect(cell.text, name).toBe('');
      expect(cell.width, name).toBe(width);
      expect([cell.dot?.width, cell.dot?.height], name).toEqual([7, 7]);
    }
  });

  it('draws the figures and words it drew, and no others', () => {
    for (const name of ['shown', 'updating'] as const) expect(words(look.strips[name].text), name).toBe('16,822,190of21,406,522match5,390shown');
    for (const name of ['shownCompact', 'updatingCompact'] as const) expect(words(look.strips[name].text), name).toBe('16.8Mof21.4Mmatch');
  });
});

describe('the selection card at rest, as wide as the panel and as the compact one', () => {
  it('has no state line once the selection is counted: the verdict is the line’s tooltip', () => {
    for (const name of ['wide', 'compact'] as const) {
      const card = look.cards[name];
      expect(card.width, name).toBe(name === 'wide' ? 340 : 264);
      expect(card.stateLine.height, name).toBe(0);
      expect(card.stateLine.text, name).toBe('');
      expect(card.stateLine.title?.trim(), name).not.toBe('');
      // The counts start one heading's margin under the heading.
      expect(card.titleToCounts, name).toBe(10);
    }
  });

  it('keeps its counts, its rows and its buttons the sizes they were', () => {
    for (const name of ['wide', 'compact'] as const) {
      const card = look.cards[name];
      expect(card.countRows, name).toBe(3);
      // 13 px text on a line 1.45 high, and a 4 px gap.
      expect(card.countRowPitch, name).toBeCloseTo(22.85, 0);
      expect(card.countsToLabel, name).toBe(10);
      expect(card.items.map((row) => row.height), name).toEqual([30, 30, 30]);
      expect(card.items.map((row) => row.textLeft), name).toEqual([6, 6, 6]);
      expect(card.actions, name).toEqual([28, 28, 28, 28]);
    }
  });

  it('draws the words it drew, and no others', () => {
    for (const name of ['wide', 'compact'] as const) {
      expect(words(look.cards[name].text), name).toBe('SelectionboxShowninside3Matchedinside≈800Visibleinside≈900Shownitems3172804748677032335morenotlistedClearOutsideExportSaveasartifact');
      expect(look.cards[name].items.map((row) => row.text), name).toEqual(['31728047486770', '32', '33']);
    }
  });
});

describe('the map at rest', () => {
  it('draws the region’s tag the size it was, with a 24 px target round its Clear button that the tag does not grow for', () => {
    const {tag, clearTarget} = look.map;
    expect(tag.height).toBe(26);
    expect(tag.clear).toEqual({width: 20, height: 20});
    expect(tag.clearToTagRight).toBe(4);
    expect(words(tag.text)).toBe('≈800inside');
    // 11 px from the button's middle is on it, each way; 13 px is not.
    expect(clearTarget.at11).toEqual([true, true, true, true]);
    expect(clearTarget.at13).toEqual([false, false, false]);
  });

  it('draws its tools and its density key the sizes they were, and no words but the tag’s and the key’s', () => {
    expect(look.map.tools).toEqual(['32x32', '32x32', '32x32', '32x32']);
    // Two lines of 11 px text 1.45 high, an 8 px ramp, two 3 px gaps, 7 px above and below, and a 1 px edge each side.
    expect(look.map.key.height).toBeCloseTo(61.9, 0);
    expect(words(look.map.key.text)).toBe('DensityFewerMoreitems');
    expect([look.map.overlayText, look.map.noteText]).toEqual(['', '']);
    expect(look.map.text.map(words)).toEqual(['≈800inside', 'DensityFewerMoreitems']);
  });
});
