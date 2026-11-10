import {createRequire} from 'node:module';
import type {Browser, Page} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The explorer in each of its main states, and the elements it does not draw, in Chromium: what
 * axe-core finds, the contrast of text in both colour schemes, text cut off in the compact form, at
 * 320 px wide and under wider text spacing, small pointer targets too close to another, where Tab
 * stops, and which element announces.
 *
 * Each state is built from nothing in one page per window size, and everything is measured before
 * the first test, so a test's failure lists every state it fails in.
 */

type Kind = 'axe' | 'contrast' | 'dark' | 'clipped' | 'spaced' | 'targets' | 'focus';
/** What was found wrong, as `state at width: finding`, by the kind of check. */
const found: Record<Kind, string[]> = {axe: [], contrast: [], dark: [], clipped: [], spaced: [], targets: [], focus: []};

/** What each state's walk stopped at, in order. */
const walked: Record<string, string[]> = {};
/** What else was read off a state: the strips' waiting figures in each scheme's check, the strips' heights, and how many live regions hold the region's count. */
const read: Record<string, {waiting?: string[]; heights?: number[]; saying?: number}> = {};

type Stop = {name: string; faults: string[]; again: boolean};
type Checks = {
  clipped(): string[];
  spaced(on: boolean): Promise<void>;
  targets(): string[];
  startWalk(): Stop | null;
  stop(): Stop | null;
  contrast(paths: string[][]): string[];
  waiting(): string[];
  stripHeights(): number[];
  saying(text: string): number;
};
type AxeNode = {target: (string | string[])[]; failureSummary?: string};
type AxeResult = {id: string; nodes: AxeNode[]};
type Axe = {run(context: unknown, options: unknown): Promise<{violations: AxeResult[]; incomplete: AxeResult[]}>};
type Scenes = {wide: string[]; compact: string[]; narrow: string[]; loose: string[]};
type Shown = {checks: Checks; scenes: Scenes; show(state: string): Promise<void>; axe: Axe; ready?: boolean};

const AXE = createRequire(import.meta.url).resolve('axe-core/axe.min.js');
const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

let served: Awaited<ReturnType<typeof servePage>>;
let browser: Browser;

async function opened(width: number, height: number): Promise<Page> {
  const page = await browser.newPage({viewport: {width, height}, colorScheme: 'light'});
  const failed = new Promise<never>((_, reject) => page.on('pageerror', reject));
  await page.goto(served.origin);
  await Promise.race([failed, page.waitForFunction(() => (window as unknown as Shown).ready === true, undefined, {timeout: 50_000})]);
  await page.addScriptTag({path: AXE});
  return page;
}

/**
 * Run axe over the page: every rule of WCAG 2.2 A and AA, or the contrast rule alone. Returns what
 * it found apart from contrast, and the elements whose contrast it found too low or could not
 * decide, each as the selectors that reach it through shadow roots.
 */
function axe(page: Page, contrastOnly: boolean): Promise<{faults: string[]; unsure: string[][]}> {
  return page.evaluate(
    async ([tags, contrastOnly]) => {
      const runOnly = contrastOnly ? {type: 'rule', values: ['color-contrast']} : {type: 'tag', values: tags};
      const {violations, incomplete} = await (window as unknown as Shown).axe.run(document, {runOnly, resultTypes: ['violations', 'incomplete']});
      const path = (n: AxeNode) => n.target.flat();
      const colour = (r: AxeResult) => r.id === 'color-contrast';
      return {
        faults: [
          ...violations.filter((v) => !colour(v)).flatMap((v) => v.nodes.map((n) => `${v.id} at ${path(n).join(' >> ')}: ${(n.failureSummary ?? '').replace(/\s+/g, ' ')}`)),
          ...incomplete.filter((v) => !colour(v)).flatMap((v) => v.nodes.map((n) => `${v.id}, undecided, at ${path(n).join(' >> ')}`))
        ],
        unsure: [...violations, ...incomplete].filter(colour).flatMap((v) => v.nodes.map(path))
      };
    },
    [TAGS, contrastOnly] as const
  );
}

/** Press Tab from where the state left the focus, or from before the explorer, until the focus leaves or comes round again. */
async function walk(page: Page): Promise<{faults: string[]; names: string[]}> {
  const faults: string[] = [];
  const names: string[] = [];
  let stop = await page.evaluate(() => (window as unknown as Shown).checks.startWalk());
  for (let i = 0; i < 300; i++) {
    if (stop?.again) break;
    if (stop) {
      names.push(stop.name);
      faults.push(...stop.faults.map((f) => `${stop!.name} ${f}`));
    }
    await page.keyboard.press('Tab');
    stop = await page.evaluate(() => (window as unknown as Shown).checks.stop());
    if (!stop) break;
  }
  return {faults, names};
}

/** Build each state in a window of this size and measure it; with `only`, measure nothing but the text that is cut off, as it is and with wider spacing. */
async function measure(width: number, height: number, pick: (scenes: Scenes) => string[], only?: 'text'): Promise<void> {
  const page = await opened(width, height);
  const add = (kind: Kind, state: string, findings: string[]) => found[kind].push(...findings.map((f) => `${state} at ${width} px: ${f}`));
  const clipped = () => page.evaluate(() => (window as unknown as Shown).checks.clipped());
  const contrast = (paths: string[][]) => page.evaluate((p) => (window as unknown as Shown).checks.contrast(p), paths);
  const spaced = (on: boolean) => page.evaluate((o) => (window as unknown as Shown).checks.spaced(o), on);
  for (const state of pick(await page.evaluate(() => (window as unknown as Shown).scenes))) {
    await page.evaluate((s) => (window as unknown as Shown).show(s), state);
    const seen = (read[`${state} at ${width} px`] = {heights: await page.evaluate(() => (window as unknown as Shown).checks.stripHeights())} as (typeof read)[string]);
    const cut = await clipped();
    add('clipped', state, cut);
    await spaced(true);
    add('spaced', state, (await clipped()).filter((c) => !cut.includes(c)));
    await spaced(false);
    if (only === 'text') continue;
    const light = await axe(page, false);
    add('axe', state, light.faults);
    add('contrast', state, await contrast(light.unsure));
    seen.waiting = await page.evaluate(() => (window as unknown as Shown).checks.waiting());
    seen.saying = await page.evaluate(() => (window as unknown as Shown).checks.saying('158'));
    add('targets', state, await page.evaluate(() => (window as unknown as Shown).checks.targets()));
    await page.emulateMedia({colorScheme: 'dark'});
    add('dark', state, await contrast((await axe(page, true)).unsure));
    await page.emulateMedia({colorScheme: 'light'});
    const {faults, names} = await walk(page);
    add('focus', state, faults);
    walked[`${state} at ${width} px`] = names;
  }
  await page.close();
}

beforeAll(async () => {
  served = await servePage('accessibility-page.ts');
  browser = served.browser;
  // One window at a time: each page holds a WebGL context, and the suite's other files draw beside this one.
  await measure(1200, 800, (s) => [...s.wide, ...s.loose]);
  // The explorer's compact form, between 720 and 1000 px.
  await measure(900, 700, (s) => s.compact, 'text');
  await measure(390, 760, (s) => s.narrow);
  // The width of a 1280 px window at 400% zoom.
  await measure(320, 480, (s) => s.narrow, 'text');
}, 240_000);

afterAll(async () => {
  await served?.close();
});

describe('the explorer in each state, and the elements on their own', () => {
  it('pass every axe rule for WCAG 2.2 A and AA', () => {
    expect(found.axe).toEqual([]);
  });

  it('draw text at 4.5 to 1 against what is behind it, or 3 to 1 for large text, in the light scheme', () => {
    expect(found.contrast).toEqual([]);
  });

  it('draw text at the same contrast in the dark scheme', () => {
    expect(found.dark).toEqual([]);
  });

  // Above 720 px a long name cut to one line with an ellipsis is not counted, where its whole text is a title or an accessible name.
  it('cut off no text and scroll nothing sideways, in the compact form and down to 320 px wide', () => {
    expect(found.clipped).toEqual([]);
  });

  it('cut off no more text when line, letter and word spacing are widened', () => {
    expect(found.spaced).toEqual([]);
  });

  it('keep every pointer target under 24 by 24 px clear of the others', () => {
    expect(found.targets).toEqual([]);
  });

  it('stop Tab only on controls that are drawn, inside the window, uncovered and show where the focus is', () => {
    expect(found.focus).toEqual([]);
    // Every state was walked: the map's tools, the map and the status strip at the least, or the one element's controls.
    for (const [state, names] of Object.entries(walked)) expect(names.length, state).toBeGreaterThanOrEqual(state.startsWith('no-filters') ? 1 : 3);
  });
});

describe('the status strip', () => {
  it('draws the counts that wait for an answer, or stand from before a refusal, at 4.5 to 1', () => {
    // The figures were on the strip, in a cell for those that wait, when its text was measured in each scheme.
    for (const state of ['refused', 'updating', 'unreachable']) expect(read[`${state} at 1200 px`]!.waiting!.join(' '), state).toMatch(/9,120.*29,935/);
    expect([...found.contrast, ...found.dark].filter((f) => / > status > /.test(f))).toEqual([]);
  });

  it('shows every line it wraps onto in the narrow form', () => {
    // A refusal's reason does not fit one line at either width.
    for (const width of [390, 320]) {
      const [height] = read[`narrow-refused at ${width} px`]!.heights!;
      expect(height, `at ${width} px`).toBeGreaterThan(60);
    }
    expect([...found.clipped, ...found.spaced].filter((f) => / > status > /.test(f))).toEqual([]);
  });

  it('is the one element in an explorer that announces a selected region’s count', () => {
    expect(read['region-counted at 1200 px']!.saying).toBe(1);
  });
});

describe('the Fields sheet in the narrow form', () => {
  it('keeps every control Tab reaches clear of its footer', () => {
    const names = walked['sheet-fields at 390 px']!;
    // The walk went past the open card to the folded ones below it, which scroll, and on to the footer.
    expect(names.filter((n) => /button\[fold\]/.test(n)).length).toBeGreaterThanOrEqual(3);
    expect(names.some((n) => /"Clear"/.test(n))).toBe(true);
    expect(found.focus.filter((f) => f.startsWith('sheet-fields'))).toEqual([]);
  });
});
