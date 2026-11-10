import {createRequire} from 'node:module';
import type {Browser, Page} from 'playwright';
import {afterAll, beforeAll, describe, expect, it} from 'vitest';
import {servePage} from './page-server.js';

/**
 * The explorer in each of its main states, and the elements it does not draw, in Chromium: what
 * axe-core finds, the contrast of text in both colour schemes, text cut off at 320 px wide and
 * under wider text spacing, small pointer targets too close to another, and where Tab stops.
 *
 * Each state is built from nothing in one page per window size, and everything is measured before
 * the first test, so a test's failure lists every state it fails in.
 */

type Kind = 'axe' | 'contrast' | 'dark' | 'clipped' | 'spaced' | 'targets' | 'focus';
/** What was found wrong, as `state at width: finding`, by the kind of check. */
const found: Record<Kind, string[]> = {axe: [], contrast: [], dark: [], clipped: [], spaced: [], targets: [], focus: []};
/** How many Tab stops each state's walk made. */
const stops: Record<string, number> = {};

/**
 * Faults the checks find in the elements as they stand. Each is left out of the test of its kind
 * and asserted on its own with `it.fails`, which fails once the fault is gone.
 */
const KNOWN = {
  // status.ts draws the cells with `.cell.dim { opacity: 0.4 }`.
  dimmedCounts: /faded by .*status > div\.cell\.dim/,
  // explorer.ts fixes the strip at 44 px high in the narrow layout, and status.ts wraps its cells.
  narrowStrip: /status > .* is cut off at the top or bottom by explorer > status > div\[strip\]/,
  // explorer.ts keeps the Fields sheet's footer at the sheet's bottom edge, over what scrolls under it.
  sheetFooter: /^sheet-.* is under explorer > button\.btn/
};
const known = (finding: string) => Object.values(KNOWN).some((k) => k.test(finding));

type Stop = {name: string; faults: string[]; again: boolean};
type Checks = {
  clipped(): string[];
  spaced(on: boolean): Promise<void>;
  targets(): string[];
  startWalk(): Stop | null;
  stop(): Stop | null;
  contrast(paths: string[][]): string[];
};
type AxeNode = {target: (string | string[])[]; failureSummary?: string};
type AxeResult = {id: string; nodes: AxeNode[]};
type Axe = {run(context: unknown, options: unknown): Promise<{violations: AxeResult[]; incomplete: AxeResult[]}>};
type Scenes = {wide: string[]; narrow: string[]; loose: string[]};
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
async function walk(page: Page): Promise<{faults: string[]; count: number}> {
  const faults: string[] = [];
  let count = 0;
  let stop = await page.evaluate(() => (window as unknown as Shown).checks.startWalk());
  for (let i = 0; i < 300; i++) {
    if (stop?.again) break;
    if (stop) {
      count += 1;
      faults.push(...stop.faults.map((f) => `${stop!.name} ${f}`));
    }
    await page.keyboard.press('Tab');
    stop = await page.evaluate(() => (window as unknown as Shown).checks.stop());
    if (!stop) break;
  }
  return {faults, count};
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
    const cut = await clipped();
    add('clipped', state, cut);
    await spaced(true);
    add('spaced', state, (await clipped()).filter((c) => !cut.includes(c)));
    await spaced(false);
    if (only === 'text') continue;
    const light = await axe(page, false);
    add('axe', state, light.faults);
    add('contrast', state, await contrast(light.unsure));
    add('targets', state, await page.evaluate(() => (window as unknown as Shown).checks.targets()));
    await page.emulateMedia({colorScheme: 'dark'});
    add('dark', state, await contrast((await axe(page, true)).unsure));
    await page.emulateMedia({colorScheme: 'light'});
    const walked = await walk(page);
    add('focus', state, walked.faults);
    stops[`${state} at ${width} px`] = walked.count;
  }
  await page.close();
}

beforeAll(async () => {
  served = await servePage('accessibility-page.ts');
  browser = served.browser;
  await Promise.all([
    measure(1200, 800, (s) => [...s.wide, ...s.loose]),
    measure(390, 760, (s) => s.narrow),
    // The width of a 1280 px window at 400% zoom.
    measure(320, 480, (s) => s.narrow, 'text')
  ]);
}, 240_000);

afterAll(async () => {
  await served?.close();
});

describe('the explorer in each state, and the elements on their own', () => {
  it('pass every axe rule for WCAG 2.2 A and AA', () => {
    expect(found.axe).toEqual([]);
  });

  it('draw text at 4.5 to 1 against what is behind it, or 3 to 1 for large text, in the light scheme', () => {
    expect(found.contrast.filter((f) => !known(f))).toEqual([]);
  });

  it('draw text at the same contrast in the dark scheme', () => {
    expect(found.dark.filter((f) => !known(f))).toEqual([]);
  });

  it('cut off no text and scroll nothing sideways, down to 320 px wide', () => {
    expect(found.clipped.filter((f) => !known(f))).toEqual([]);
  });

  it('cut off no more text when line, letter and word spacing are widened', () => {
    expect(found.spaced.filter((f) => !known(f))).toEqual([]);
  });

  it('keep every pointer target under 24 by 24 px clear of the others', () => {
    expect(found.targets).toEqual([]);
  });

  it('stop Tab only on controls that are drawn, inside the window, uncovered and show where the focus is', () => {
    expect(found.focus.filter((f) => !known(f))).toEqual([]);
    // Every state was walked: the map's tools, the map and the status strip at the least.
    for (const [state, count] of Object.entries(stops)) expect(count, state).toBeGreaterThanOrEqual(3);
  });

  it.fails('draw the status strip’s counts at 4.5 to 1 while they wait for an answer or after a refusal (status.ts dims them to 40%)', () => {
    expect([...found.contrast, ...found.dark].filter((f) => KNOWN.dimmedCounts.test(f))).toEqual([]);
  });

  it.fails('show every line of the status strip in the narrow layout (explorer.ts fixes its height at 44 px)', () => {
    expect([...found.clipped, ...found.spaced].filter((f) => KNOWN.narrowStrip.test(f))).toEqual([]);
  });

  it.fails('keep a focused control of the Fields sheet clear of the sheet’s footer (explorer.ts draws the footer over it)', () => {
    expect(found.focus.filter((f) => KNOWN.sheetFooter.test(f))).toEqual([]);
  });
});
