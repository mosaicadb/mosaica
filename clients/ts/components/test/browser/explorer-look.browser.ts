import type {Browser} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The explorer at rest with a pointer in its wide and compact layouts, in Chromium: the sizes of
 * its heading, its tools and the Layers popover's rows, and the text that shows. The sizes are
 * heights and fixed widths, which no font's letter widths change; a row whose height depends on
 * where its text wraps is left out.
 */

let served: Awaited<ReturnType<typeof servePage>>;
let origin = '';
let browser: Browser;

beforeAll(async () => {
  served = await servePage('explorer-look-page.ts');
  ({origin, browser} = served);
});

afterAll(async () => {
  await served?.close();
});

type Size = {width: number; height: number};
type Menu = {width: number; entries: number[]; lefts: number[]; tick: {inside: boolean; covers: boolean} | null; text: string[]};
type Look = {
  rest: {column: number; head: number; title: number; titleLines: number; titleCut: boolean; titleHolds: string; titleTitle: string; picker: number; selectFits: boolean; tools: Size; layers: Size; text: string[]};
  open: {
    width: number;
    sections: number[];
    layerPicker: number;
    entries: number[];
    names: {lines: number; cut: boolean; title: string}[];
    toggle: Size;
    sizeBy: {height: number; title: string};
    colourBy: number;
    palette: number;
    editColours: number;
    modes: number;
    densityScale: number | null;
    densityColours: number | null;
    sliders: Record<string, number>;
    text: string[];
  };
  palette: Menu;
  colour: Menu;
};

async function look(width: number, height: number, query: string): Promise<Look & {ring: boolean}> {
  const page = await browser.newPage({viewport: {width, height}});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(`${origin}/?${query}`);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as {result?: unknown}).result !== undefined, undefined, {timeout: 50_000})]);
  const result = await page.evaluate(() => (window as unknown as {result: Look}).result);
  // Open the popover with the pointer: whether the control that takes focus in it shows a ring.
  const toggle = await page.evaluate(() => {
    const root = document.querySelector('mosaica-explorer')!.shadowRoot!;
    const button = root.querySelector('[part="layers-toggle"]') as HTMLElement;
    if (root.querySelector('[part="layers-popover"]')) button.click();
    const r = button.getBoundingClientRect();
    return {x: r.left + r.width / 2, y: r.top + r.height / 2};
  });
  await page.mouse.click(toggle.x, toggle.y);
  const ring = await page.evaluate(async () => {
    const explorer = document.querySelector('mosaica-explorer') as HTMLElement & {updateComplete: Promise<unknown>};
    await explorer.updateComplete;
    const root = explorer.shadowRoot!;
    const focused = root.activeElement;
    if (!focused || !root.querySelector('[part="layers-popover"]')?.contains(focused)) throw new Error('the popover did not take focus');
    return focused.matches(':focus-visible');
  });
  await page.close();
  return {...result, ring};
}

const LONG = 'a name long enough that no line of the panel or the popover could hold all of it without cutting it short';
const POINTS = ['Points', 'Most points', '250K', '1K', '10K', '100K', '2M', 'Size by', 'None', 'Size', 'Opacity'];
const COLOUR = ['Colour', 'Colour by', 'Archive', 'Palette', 'Tableau 10', 'Edit colours…'];
const MODES = ['Density', 'None', 'Smooth', 'Hex', 'Grid', 'Lines'];
const LAYERS = ['Layers', 'Clusters', `Districts with ${LONG}`];
const near = (got: number, wanted: number, what: string) => expect(Math.abs(got - wanted), `${what}: ${got}, not ${wanted}`).toBeLessThanOrEqual(0.1);

/** What holds in every wide and compact layout: one line for each long name, the popover's rows and the two menus. */
function common(r: Look & {ring: boolean}): void {
  // Opened with a pointer, the popover shows no focus ring.
  expect(r.ring).toBe(false);
  // The title is one line, cut, with all of it in the element and in its `title`.
  expect(r.rest.titleLines).toBe(1);
  expect(r.rest.titleCut).toBe(true);
  expect(r.rest.titleHolds).toBe(`A dataset with ${LONG}`);
  expect(r.rest.titleTitle).toBe(r.rest.titleHolds);
  expect(r.rest.text).toEqual([r.rest.titleHolds]);
  // The view's select takes one line of the heading, inside it.
  near(r.rest.picker, 18.84, 'view picker');
  expect(r.rest.selectFits).toBe(true);

  near(r.open.width, 320, 'popover width');
  near(r.open.sections[0]!, 203.92, 'Points section');
  near(r.open.sections[1]!, 145.72, 'Colour section');
  near(r.open.layerPicker, 109.94, 'layer picker');
  expect(r.open.entries).toEqual([28, 28]);
  // A layer's name is one line; the long one is cut and its row's `title` holds all of it. The
  // layer drawn with another says so in its `title`, not in the row.
  expect(r.open.names.map((n) => n.lines)).toEqual([1, 1]);
  expect(r.open.names.map((n) => n.cut)).toEqual([false, true]);
  expect(r.open.names[0]!.title).toContain('Cluster names');
  expect(r.open.names[1]!.title).toBe(`Districts with ${LONG}`);
  expect(r.open.toggle).toEqual({width: 30, height: 18});
  near(r.open.sizeBy.height, 25.39, 'Size by');
  expect(r.open.sizeBy.title).toBe('No number column to size by');
  near(r.open.colourBy, 25.39, 'Colour by');
  near(r.open.palette, 27.39, 'Palette');
  near(r.open.editColours, 22, 'Edit colours');
  near(r.open.modes, 28, 'Density choice');
  // The room each slider takes in its row.
  for (const name of ['most-points', 'point-size', 'point-opacity']) near(r.open.sliders[name]!, 16, name);

  // The Palette menu: its width, a row whose line is short, the swatches where they were, and the
  // tick on the chosen row over none of the row's swatches or words.
  near(r.palette.width, 360, 'Palette menu');
  for (const i of [0, 2, 3]) near(r.palette.entries[i]!, 52.23, `palette ${i}`);
  expect(r.palette.lefts).toEqual([10, 10, 10, 10]);
  expect(r.palette.tick).toEqual({inside: true, covers: false});
  expect(r.palette.text).toEqual(['Palette', 'Tableau 10', '10 colours', 'Okabe-Ito', '8 colours · colour-blind safe', 'Set 2', '8 colours', 'Dark 2', '8 colours']);
  near(r.colour.width, 220, 'Colour by menu');
  for (const i of [0, 1, 3]) near(r.colour.entries[i]!, 32.84, `colour ${i}`);
  expect(r.colour.lefts).toEqual([14, 14, 14, 14]);
  expect(r.colour.tick).toEqual({inside: true, covers: false});
}

describe('the explorer at rest, as it was before the keyboard and screen-reader work', () => {
  it('keeps its sizes and its words in the wide overlay layout', async () => {
    const r = await look(1280, 800, 'layout=overlay');
    common(r);
    near(r.rest.column, 340, 'panel');
    near(r.rest.head, 64.59, 'heading');
    near(r.rest.title, 21.75, 'title');
    expect(r.rest.tools).toEqual({width: 40, height: 142});
    expect(r.rest.layers).toEqual({width: 40, height: 40});
    near(r.open.sections[2]!, 78.94, 'Density section');
    expect(r.open.text).toEqual([...POINTS, ...COLOUR, ...MODES, ...LAYERS]);
  });

  it('keeps them in the wide docked layout, with density drawn', async () => {
    const r = await look(1280, 800, 'layout=docked&density=smooth');
    common(r);
    near(r.rest.column, 340, 'sidebar');
    near(r.rest.head, 66.59, 'heading');
    near(r.rest.title, 21.75, 'title');
    expect(r.rest.tools).toEqual({width: 40, height: 142});
    near(r.open.sections[2]!, 249.11, 'Density section');
    near(r.open.densityScale!, 28, 'density scale');
    near(r.open.densityColours!, 25.39, 'density colours');
    near(r.open.sliders['density-resolution']!, 34, 'density-resolution');
    near(r.open.sliders['density-strength']!, 16, 'density-strength');
    // The cell size shown depends on the window; the words around it do not.
    const words = r.open.text.filter((t) => !/^\d+$/.test(t));
    expect(words).toEqual([...POINTS, ...COLOUR, ...MODES, 'Resolution', 'Coarse', 'cells ≈', 'px', 'Fine', 'Scale', 'Linear', 'Log', 'Colours', 'Warm grey', 'Strength', '%', ...LAYERS]);
  });

  it('keeps them in the compact layout', async () => {
    const r = await look(900, 700, 'layout=docked');
    common(r);
    near(r.rest.column, 264, 'panel');
    near(r.rest.head, 59.14, 'heading');
    near(r.rest.title, 20.3, 'title');
    expect(r.rest.tools).toEqual({width: 38, height: 134});
    expect(r.rest.layers).toEqual({width: 38, height: 38});
    near(r.open.sections[2]!, 78.94, 'Density section');
    expect(r.open.text).toEqual([...POINTS, ...COLOUR, ...MODES, ...LAYERS]);
  });
});
